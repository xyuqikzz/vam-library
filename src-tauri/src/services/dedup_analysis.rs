use super::{
    dedup_cache::{check, Cache},
    resource_dedup::{self as dedup, DedupSnapshot, DuplicateInstance},
    resource_files::{self as files, DiskFile},
};
use crate::db::Database;
use serde::{Deserialize, Serialize};
use std::{
    collections::{BTreeMap, HashMap, HashSet},
    path::Path,
    sync::atomic::AtomicBool,
    time::Instant,
};

#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Mode {
    #[default]
    Standard,
    Deep,
}

#[derive(Default)]
pub struct Protection {
    pub complete: bool,
    pub required: HashMap<String, Vec<String>>,
    pub manual: HashSet<String>,
}
fn id(file: &DiskFile) -> String {
    Path::new(&file.path)
        .file_stem()
        .unwrap_or_default()
        .to_string_lossy()
        .into_owned()
}
fn is_var(file: &DiskFile) -> bool {
    Path::new(&file.path)
        .extension()
        .is_some_and(|e| e.eq_ignore_ascii_case("var"))
}

pub fn analyze(
    root: &Path,
    excluded: HashSet<String>,
    mode: Mode,
    db: Option<&Database>,
    protection: &Protection,
    cancel: &AtomicBool,
    progress: &mut impl FnMut(&str, usize, usize, u64),
) -> Result<DedupSnapshot, String> {
    let start = Instant::now();
    let root = files::canonical_dir(root)?;
    let mut cache = Cache::load(db, &root)?;
    let result = (|| {
        let mut snapshot = dedup::scan_cached(&root, excluded, &mut cache, cancel, progress)?;
        cache.prune(&snapshot.files)?;
        let vars: Vec<_> = snapshot
            .files
            .iter()
            .filter(|f| is_var(f))
            .cloned()
            .collect();
        let mut sizes = BTreeMap::<u64, Vec<&DiskFile>>::new();
        for file in &vars {
            sizes.entry(file.size).or_default().push(file);
        }
        let total: usize = sizes.values().filter(|v| v.len() > 1).map(Vec::len).sum();
        let mut done = 0;
        for candidates in sizes.values().filter(|v| v.len() > 1) {
            for file in candidates {
                check(cancel)?;
                progress("hashing", done, total, cache.bytes);
                match cache.raw(file, cancel, &mut |n| progress("hashing", done, total, n)) {
                    Ok(hash) => {
                        snapshot.hashes.insert(file.path.clone(), hash);
                    }
                    Err(e) => {
                        check(cancel)?;
                        snapshot
                            .summary
                            .warnings
                            .push(format!("没能比较文件内容，已保留: {} ({e})", file.path));
                    }
                }
                done += 1;
            }
        }
        // One representative per byte-identical group, including unparseable names.
        let mut representatives = BTreeMap::<String, Vec<DiskFile>>::new();
        for file in &vars {
            let key = snapshot
                .hashes
                .get(&file.path)
                .map(|h| format!("hash:{h}"))
                .unwrap_or_else(|| format!("path:{}", file.path));
            representatives.entry(key).or_default().push(file.clone());
        }
        if mode == Mode::Deep {
            let mut layouts = BTreeMap::<String, Vec<String>>::new();
            for (i, (key, copies)) in representatives.iter().enumerate() {
                check(cancel)?;
                progress("directory", i, representatives.len(), cache.bytes);
                match cache.layout(&copies[0], cancel)? {
                    Ok(layout) => layouts.entry(layout).or_default().push(key.clone()),
                    Err(e) => snapshot.summary.warnings.push(format!(
                        "没能深度比较资源包，仍保留整包比较结果: {} ({e})",
                        copies[0].path
                    )),
                }
            }
            let count: usize = layouts.values().filter(|v| v.len() > 1).map(Vec::len).sum();
            let mut done = 0;
            for candidates in layouts.values().filter(|v| v.len() > 1) {
                for key in candidates {
                    let copies = &representatives[key];
                    progress("content", done, count, cache.bytes);
                    match cache.content(&copies[0], cancel, &mut |n| {
                        progress("content", done, count, n)
                    }) {
                        Ok(hash) => {
                            for file in copies {
                                snapshot
                                    .content_hashes
                                    .insert(file.path.clone(), hash.clone());
                            }
                        }
                        Err(e) => {
                            check(cancel)?;
                            snapshot.summary.warnings.push(format!(
                                "没能完整读取包内文件，已跳过深度比较: {} ({e})",
                                copies[0].path
                            ));
                        }
                    }
                    done += 1;
                }
            }
        }
        snapshot.verification_stamps = snapshot.files.iter().filter_map(|file| {
            cache.verification_stamp(file, snapshot.hashes.get(&file.path), snapshot.content_hashes.get(&file.path))
                .map(|stamp| (file.path.clone(), stamp))
        }).collect();
        merge(&mut snapshot, protection)?;
        snapshot.summary.mode = if mode == Mode::Deep {
            "deep"
        } else {
            "standard"
        }
        .into();
        snapshot.summary.cache_hits = cache.hits;
        snapshot.summary.bytes_read = cache.bytes;
        snapshot.summary.elapsed_ms = start.elapsed().as_millis() as u64;
        progress(
            "complete",
            snapshot.files.len(),
            snapshot.files.len(),
            cache.bytes,
        );
        check(cancel)?;
        Ok(snapshot)
    })();
    // Completed records survive cancellation; partial digests never enter the cache.
    cache.flush()?;
    result
}

fn evidence(snapshot: &DedupSnapshot, path: &str) -> String {
    snapshot
        .content_hashes
        .get(path)
        .map(|h| format!("content:{h}"))
        .or_else(|| snapshot.hashes.get(path).map(|h| format!("raw:{h}")))
        .unwrap_or_else(|| format!("unique:{path}"))
}
fn find(parent: &mut [usize], i: usize) -> usize {
    if parent[i] != i {
        parent[i] = find(parent, parent[i]);
    }
    parent[i]
}
fn union(parent: &mut [usize], a: usize, b: usize) {
    let a = find(parent, a);
    let b = find(parent, b);
    parent[b] = a;
}
fn merge(snapshot: &mut DedupSnapshot, p: &Protection) -> Result<(), String> {
    let mut instances = HashMap::<String, DuplicateInstance>::new();
    let indices: HashMap<_, _> = snapshot
        .files
        .iter()
        .enumerate()
        .map(|(i, f)| (f.path.clone(), i))
        .collect();
    let mut parent: Vec<_> = (0..snapshot.files.len()).collect();
    for group in &snapshot.groups {
        for item in &group.instances {
            union(
                &mut parent,
                indices[&group.instances[0].file_path],
                indices[&item.file_path],
            );
            instances.insert(item.file_path.clone(), item.clone());
        }
    }
    let mut equivalent = BTreeMap::<String, Vec<DiskFile>>::new();
    let mut identities = BTreeMap::<String, Vec<DiskFile>>::new();
    for file in snapshot.files.iter().filter(|f| is_var(f)) {
        let package = id(file);
        let key = package.to_lowercase();
        identities
            .entry(key.clone())
            .or_default()
            .push(file.clone());
        equivalent
            .entry(evidence(snapshot, &file.path))
            .or_default()
            .push(file.clone());
        let item = instances.entry(file.path.clone()).or_insert_with(|| {
            dedup::instance(
                file,
                package,
                files::package_name(Path::new(&file.path)).map(|n| n.version),
                true,
                vec![],
                None,
            )
        });
        // Ordinary-file grouping used the extension as part of the identity.
        item.package_id = id(file);
        if let Some(refs) = p.required.get(&key) {
            item.referenced_by = refs.clone();
        }
    }
    // A shared package identity is not proof that the archive bytes are equal.
    for copies in identities.values().filter(|v| v.len() > 1) {
        let variants: HashSet<_> = copies.iter().map(|f| evidence(snapshot, &f.path)).collect();
        if variants.len() > 1 {
            snapshot.summary.warnings.push(format!(
                "同名资源包的内容不同，已保留不同内容: {}",
                id(&copies[0])
            ));
            for file in copies {
                let i = instances.get_mut(&file.path).unwrap();
                i.is_recommended_keep = true;
                i.archive_destination = None;
                i.protection_reason = Some("conflict".into());
            }
        }
    }
    for copies in equivalent.values_mut().filter(|v| v.len() > 1) {
        copies.sort_by_key(|f| {
            let key = id(f).to_lowercase();
            let name = files::package_name(Path::new(&f.path));
            (
                std::cmp::Reverse(p.manual.contains(&key)),
                std::cmp::Reverse(p.required.contains_key(&key)),
                std::cmp::Reverse(name.is_some()),
                std::cmp::Reverse(name.map(|n| n.version).unwrap_or(0)),
                std::cmp::Reverse(f.modified.parse::<u128>().unwrap_or(0)),
                f.path.clone(),
            )
        });
        let mut kept = HashSet::from([copies[0].path.clone()]);
        let mut kept_ids = HashSet::from([id(&copies[0]).to_lowercase()]);
        for file in copies.iter() {
            let key = id(file).to_lowercase();
            if p.manual.contains(&key)
                || ((!p.complete || p.required.contains_key(&key)) && kept_ids.insert(key))
            {
                kept.insert(file.path.clone());
            }
        }
        for file in copies.iter() {
            union(&mut parent, indices[&copies[0].path], indices[&file.path]);
            let item = instances.get_mut(&file.path).unwrap();
            item.is_recommended_keep = kept.contains(&file.path);
            if !item.is_recommended_keep {
                item.archive_destination = None;
                item.protection_reason = None;
                // Prefer a survivor with the same identity when there is one.
                let witness = copies
                    .iter()
                    .filter(|f| kept.contains(&f.path))
                    .find(|f| id(f).eq_ignore_ascii_case(&id(file)))
                    .unwrap_or(&copies[0]);
                snapshot
                    .witnesses
                    .insert(file.path.clone(), witness.path.clone());
            }
        }
    }
    for item in instances.values_mut() {
        if !Path::new(&item.file_path)
            .extension()
            .is_some_and(|e| e.eq_ignore_ascii_case("var"))
        {
            continue;
        }
        let key = item.package_id.to_lowercase();
        let same_identity = snapshot.witnesses.get(&item.file_path).is_some_and(|keep| {
            Path::new(keep).file_stem() == Path::new(&item.file_path).file_stem()
        });
        if p.manual.contains(&key)
            || (!same_identity && (!p.complete || p.required.contains_key(&key)))
        {
            item.is_recommended_keep = true;
            item.protection_reason = Some(
                if p.manual.contains(&key) {
                    "manual"
                } else if !p.complete {
                    "incomplete"
                } else {
                    "referenced"
                }
                .into(),
            );
            if p.manual.contains(&key) || !p.complete {
                item.archive_destination = None;
            }
            snapshot.witnesses.remove(&item.file_path);
        }
    }
    let mut components = BTreeMap::<usize, Vec<DuplicateInstance>>::new();
    for (path, item) in instances {
        components
            .entry(find(&mut parent, indices[&path]))
            .or_default()
            .push(item);
    }
    snapshot.groups.clear();
    for (_, mut items) in components {
        if items.len() < 2 {
            continue;
        }
        items.sort_by_key(|i| {
            (
                std::cmp::Reverse(i.is_recommended_keep),
                i.package_id.clone(),
                i.file_path.clone(),
            )
        });
        let kinds: HashSet<_> = items
            .iter()
            .map(|i| evidence(snapshot, &i.file_path))
            .collect();
        let strategy = if kinds.len() == 1 {
            if kinds.iter().next().unwrap().starts_with("content:") {
                "deep"
            } else {
                "exact"
            }
        } else {
            "version"
        };
        let name = items[0].package_id.clone();
        snapshot.groups.push(dedup::group(
            format!("analysis:{}", items[0].file_path),
            strategy,
            name,
            items,
        ));
    }
    snapshot.summary.duplicate_groups = snapshot.groups.len();
    snapshot.summary.total_wasted_bytes =
        snapshot.groups.iter().map(|g| g.total_wasted_bytes).sum();
    snapshot.summary.safe_to_clean_count = snapshot
        .groups
        .iter()
        .flat_map(|g| &g.instances)
        .filter(|i| !i.is_recommended_keep)
        .count();
    snapshot.summary.archive_count = snapshot
        .groups
        .iter()
        .flat_map(|g| &g.instances)
        .filter(|i| i.archive_destination.is_some())
        .count();
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use files::tests::TestDir;
    use std::{io::Write, path::PathBuf};
    fn package(dir: &TestDir, name: &str, method: zip::CompressionMethod, meta: &[u8]) -> PathBuf {
        let path = dir.0.join(name);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        let mut zip = zip::ZipWriter::new(std::fs::File::create(&path).unwrap());
        let options = zip::write::SimpleFileOptions::default().compression_method(method);
        zip.start_file("meta.json", options).unwrap();
        zip.write_all(meta).unwrap();
        zip.start_file("Custom/test.txt", options).unwrap();
        zip.write_all(&vec![b'x'; 16384]).unwrap();
        zip.finish().unwrap();
        path
    }
    fn run(dir: &TestDir, mode: Mode, db: Option<&Database>, p: &Protection) -> DedupSnapshot {
        analyze(
            &dir.0,
            HashSet::new(),
            mode,
            db,
            p,
            &AtomicBool::new(false),
            &mut |_, _, _, _| {},
        )
        .unwrap()
    }
    fn clear() -> Protection {
        Protection {
            complete: true,
            ..Default::default()
        }
    }
    fn removals(s: &DedupSnapshot) -> Vec<String> {
        s.groups
            .iter()
            .flat_map(|g| &g.instances)
            .filter(|i| !i.is_recommended_keep)
            .map(|i| i.file_path.clone())
            .collect()
    }
    fn reset_validation_reads() {
        files::HASHED_BYTES.with(|v| v.set(0));
        super::super::dedup_cache::CONTENT_HASHED_BYTES.with(|v| v.set(0));
    }
    fn validation_reads() -> u64 {
        files::HASHED_BYTES.with(|v| v.get())
            + super::super::dedup_cache::CONTENT_HASHED_BYTES.with(|v| v.get())
    }

    #[test]
    fn pinned_cleanup_reuses_unchanged_proofs_in_both_modes() {
        for mode in [Mode::Standard, Mode::Deep] {
            let dir = TestDir::new();
            let data = TestDir::new();
            let db = Database::new(&data.0.join("cache.sqlite")).unwrap();
            let first = package(&dir, "A.Asset.1.var", zip::CompressionMethod::Stored, b"{}");
            if mode == Mode::Deep {
                package(&dir, "A.Asset.1_2.var", zip::CompressionMethod::Deflated, b"{}");
            } else {
                dir.write("A.Asset.1_2.var", &std::fs::read(first).unwrap());
            }
            // Cover both newly computed and persisted/reloaded evidence.
            for _ in 0..2 {
                let snapshot = run(&dir, mode, Some(&db), &clear());
                let selected = removals(&snapshot);
                assert_eq!(selected.len(), 1);
                let pins = dedup::pin_selection(&snapshot, &selected).unwrap();
                reset_validation_reads();
                assert_eq!(dedup::validate_pinned_selection(&snapshot, &selected, &pins).unwrap().len(), 1);
                if cfg!(windows) { assert_eq!(validation_reads(), 0); }
                else { assert!(validation_reads() > 0); }
                // A caller without write-blocking pins must still read every proof.
                reset_validation_reads();
                dedup::validate_selection(&snapshot, &selected).unwrap();
                assert!(validation_reads() > 0);
                #[cfg(windows)]
                for path in dedup::verification_paths(&snapshot, &selected) {
                    assert!(std::fs::OpenOptions::new().write(true).open(path).is_err());
                }
            }
        }
    }

    #[test]
    fn pinned_cleanup_falls_back_without_a_reliable_stamp() {
        struct Reset;
        impl Drop for Reset {
            fn drop(&mut self) { super::super::cleanup_reference_cache::FORCE_SIGNATURE_CHECK.with(|v| v.set(false)); }
        }
        for deep in [false, true] {
            let dir = TestDir::new();
            let first = package(&dir, "A.Asset.1.var", zip::CompressionMethod::Stored, b"{}");
            if deep { package(&dir, "A.Asset.1_2.var", zip::CompressionMethod::Deflated, b"{}"); }
            else { dir.write("A.Asset.1_2.var", &std::fs::read(first).unwrap()); }
            let snapshot = run(&dir, if deep { Mode::Deep } else { Mode::Standard }, None, &clear());
            let selected = removals(&snapshot);
            let pins = dedup::pin_selection(&snapshot, &selected).unwrap();
            let _reset = Reset;
            super::super::cleanup_reference_cache::FORCE_SIGNATURE_CHECK.with(|v| v.set(true));
            reset_validation_reads();
            dedup::validate_pinned_selection(&snapshot, &selected, &pins).unwrap();
            assert!(validation_reads() > 0);
        }
    }

    #[test]
    fn pinned_cleanup_rejects_changed_copy_and_survivor_with_restored_mtime() {
        for deep in [false, true] {
            for change_survivor in [false, true] {
                let dir = TestDir::new();
                let first = package(&dir, "A.Asset.1.var", zip::CompressionMethod::Stored, b"{}");
                if deep { package(&dir, "A.Asset.1_2.var", zip::CompressionMethod::Deflated, b"{}"); }
                else { dir.write("A.Asset.1_2.var", &std::fs::read(first).unwrap()); }
                let snapshot = run(&dir, if deep { Mode::Deep } else { Mode::Standard }, None, &clear());
                let selected = removals(&snapshot);
                let path = if change_survivor { &snapshot.witnesses[&selected[0]] } else { &selected[0] };
                let original_time = std::fs::metadata(path).unwrap().modified().unwrap();
                // Keep the size and mtime unchanged; modify a ZIP local-header byte.
                // Raw comparison fails; deep comparison either errors or detects content.
                let mut bytes = std::fs::read(path).unwrap();
                if deep { bytes[0] ^= 1; } else { bytes[40] ^= 1; }
                std::fs::write(path, bytes).unwrap();
                std::fs::File::options().write(true).open(path).unwrap()
                    .set_times(std::fs::FileTimes::new().set_modified(original_time)).unwrap();
                let pins = dedup::pin_selection(&snapshot, &selected).unwrap();
                assert!(dedup::validate_pinned_selection(&snapshot, &selected, &pins).is_err());
            }
        }
    }

    #[test]
    fn pinned_cleanup_still_accepts_identical_content_after_metadata_changes() {
        let dir = TestDir::new();
        let original = package(&dir, "A.Asset.1.var", zip::CompressionMethod::Stored, b"{}");
        dir.write("A.Asset.1_2.var", &std::fs::read(original).unwrap());
        let snapshot = run(&dir, Mode::Standard, None, &clear());
        let selected = removals(&snapshot);
        let path = &selected[0];
        let mtime = std::fs::metadata(path).unwrap().modified().unwrap();
        std::fs::write(path, std::fs::read(path).unwrap()).unwrap();
        std::fs::File::options().write(true).open(path).unwrap()
            .set_times(std::fs::FileTimes::new().set_modified(mtime)).unwrap();
        let pins = dedup::pin_selection(&snapshot, &selected).unwrap();
        reset_validation_reads();
        dedup::validate_pinned_selection(&snapshot, &selected, &pins).unwrap();
        assert!(validation_reads() > 0);
    }

    #[test]
    fn different_names_are_grouped_and_standard_filename_is_retained() {
        let dir = TestDir::new();
        let original = package(
            &dir,
            "clothing/Qing.Alet01.1.var",
            zip::CompressionMethod::Stored,
            b"{}",
        );
        let copy = dir.write(
            "other/Qing.Alet01.1_2.var",
            &std::fs::read(&original).unwrap(),
        );
        let snapshot = run(&dir, Mode::Standard, None, &clear());
        assert_eq!(snapshot.groups.len(), 1);
        assert_eq!(snapshot.groups[0].strategy, "exact");
        assert_eq!(
            removals(&snapshot)
                .iter()
                .map(|p| files::path_key(p))
                .collect::<Vec<_>>(),
            [files::path_key(&copy.to_string_lossy())]
        );
        assert_eq!(
            snapshot.summary.total_wasted_bytes,
            std::fs::metadata(copy).unwrap().len()
        );
        assert_eq!(
            snapshot
                .witnesses
                .values()
                .map(|p| files::path_key(p))
                .collect::<Vec<_>>(),
            [files::path_key(&original.to_string_lossy())]
        );
    }
    #[test]
    fn deep_compares_all_uncompressed_files_across_compression_methods() {
        let dir = TestDir::new();
        let a = package(&dir, "A.Asset.1.var", zip::CompressionMethod::Stored, b"{}");
        let b = package(
            &dir,
            "A.Asset.1_2.var",
            zip::CompressionMethod::Deflated,
            b"{}",
        );
        assert_ne!(
            std::fs::metadata(a).unwrap().len(),
            std::fs::metadata(b).unwrap().len()
        );
        assert_eq!(
            run(&dir, Mode::Standard, None, &clear())
                .summary
                .safe_to_clean_count,
            0
        );
        let deep = run(&dir, Mode::Deep, None, &clear());
        assert_eq!(deep.summary.safe_to_clean_count, 1);
        assert_eq!(deep.groups[0].strategy, "deep");
        dedup::validate_selection(&deep, &removals(&deep)).unwrap();
        package(
            &dir,
            "A.Asset.1_3.var",
            zip::CompressionMethod::Deflated,
            b"{\"dependencies\":{\"Other.Asset.1\":{}}}",
        );
        let different = run(&dir, Mode::Deep, None, &clear());
        assert_eq!(different.summary.safe_to_clean_count, 1);
        assert!(!removals(&different).iter().any(|p| p.ends_with("1_3.var")));
    }
    #[test]
    fn names_with_references_manual_protection_or_unknown_consumers_are_kept() {
        let dir = TestDir::new();
        let a = package(&dir, "A.Asset.1.var", zip::CompressionMethod::Stored, b"{}");
        dir.write("A.Asset.1_2.var", &std::fs::read(a).unwrap());
        let mut p = clear();
        p.required
            .insert("a.asset.1".into(), vec!["scene-a".into()]);
        p.required
            .insert("a.asset.1_2".into(), vec!["scene-b".into()]);
        assert!(removals(&run(&dir, Mode::Standard, None, &p)).is_empty());
        p.required.clear();
        p.manual.extend(["a.asset.1".into(), "a.asset.1_2".into()]);
        assert!(removals(&run(&dir, Mode::Standard, None, &p)).is_empty());
        assert!(removals(&run(&dir, Mode::Standard, None, &Protection::default())).is_empty());
    }
    #[test]
    fn matching_names_with_different_content_are_not_duplicates() {
        let dir = TestDir::new();
        package(
            &dir,
            "a/A.Asset.1.var",
            zip::CompressionMethod::Stored,
            b"{\"a\":1}",
        );
        package(
            &dir,
            "b/A.Asset.1.var",
            zip::CompressionMethod::Stored,
            b"{\"a\":2}",
        );
        for mode in [Mode::Standard, Mode::Deep] {
            assert!(removals(&run(&dir, mode, None, &clear())).is_empty());
        }
    }
    #[test]
    fn incremental_cache_reuses_completed_evidence_and_invalidates_preserved_mtime_changes() {
        let dir = TestDir::new();
        let data = TestDir::new();
        let db = Database::new(&data.0.join("cache.db")).unwrap();
        package(&dir, "A.Asset.1.var", zip::CompressionMethod::Stored, b"{}");
        let path = package(
            &dir,
            "A.Asset.1_2.var",
            zip::CompressionMethod::Deflated,
            b"{}",
        );
        let first = run(&dir, Mode::Deep, Some(&db), &clear());
        assert!(first.summary.bytes_read > 0);
        let second = run(&dir, Mode::Deep, Some(&db), &clear());
        assert_eq!(second.summary.bytes_read, 0);
        assert!(second.summary.cache_hits > 0);
        let before = path.metadata().unwrap().modified().unwrap();
        let mut bytes = std::fs::read(&path).unwrap();
        bytes[0] ^= 1;
        std::fs::write(&path, bytes).unwrap();
        std::fs::OpenOptions::new()
            .write(true)
            .open(&path)
            .unwrap()
            .set_times(std::fs::FileTimes::new().set_modified(before))
            .unwrap();
        let third = run(&dir, Mode::Deep, Some(&db), &clear());
        assert!(third.summary.safe_to_clean_count == 0);
    }
    #[test]
    fn execution_checks_selected_groups_but_does_not_rehash_unrelated_groups() {
        let dir = TestDir::new();
        let a = package(&dir, "A.Asset.1.var", zip::CompressionMethod::Stored, b"{}");
        dir.write("A.Asset.1_2.var", &std::fs::read(a).unwrap());
        let b = package(
            &dir,
            "B.Asset.1.var",
            zip::CompressionMethod::Stored,
            b"{\"a\":1}",
        );
        let other = dir.write("B.Asset.1_2.var", &std::fs::read(b).unwrap());
        let snapshot = run(&dir, Mode::Standard, None, &clear());
        let paths: Vec<_> = removals(&snapshot)
            .into_iter()
            .filter(|p| p.contains("A.Asset"))
            .collect();
        let before = other.metadata().unwrap().modified().unwrap();
        let mut bytes = std::fs::read(&other).unwrap();
        bytes[0] ^= 1;
        std::fs::write(&other, bytes).unwrap();
        std::fs::OpenOptions::new()
            .write(true)
            .open(&other)
            .unwrap()
            .set_times(std::fs::FileTimes::new().set_modified(before))
            .unwrap();
        assert_eq!(
            dedup::validate_selection(&snapshot, &paths).unwrap().len(),
            1
        );
        assert!(dedup::validate_selection(&snapshot, &removals(&snapshot)).is_err());
    }
    #[test]
    fn cancellation_never_publishes_a_partial_plan_and_keeps_completed_cache() {
        let dir = TestDir::new();
        let data = TestDir::new();
        let db = Database::new(&data.0.join("cache.db")).unwrap();
        let a = package(&dir, "A.Asset.1.var", zip::CompressionMethod::Stored, b"{}");
        dir.write("A.Asset.1_2.var", &std::fs::read(a).unwrap());
        let cancel = AtomicBool::new(false);
        let result = analyze(
            &dir.0,
            HashSet::new(),
            Mode::Standard,
            Some(&db),
            &clear(),
            &cancel,
            &mut |stage, done, _, _| {
                if stage == "hashing" && done == 1 {
                    cancel.store(true, std::sync::atomic::Ordering::Relaxed);
                }
            },
        );
        assert!(result.unwrap_err().contains("DEDUP_CANCELLED"));
        let next = run(&dir, Mode::Standard, Some(&db), &clear());
        assert!(next.summary.cache_hits > 0);
        assert_eq!(next.summary.safe_to_clean_count, 1);
    }
    #[test]
    fn duplicate_zip_entries_are_never_deep_equivalence_evidence() {
        let dir = TestDir::new();
        let path = dir.0.join("A.Asset.1.var");
        let mut zip = zip::ZipWriter::new(std::fs::File::create(&path).unwrap());
        zip.start_file("Custom/test.txt", zip::write::SimpleFileOptions::default())
            .unwrap();
        zip.write_all(b"a").unwrap();
        zip.start_file("Custom\\test.txt", zip::write::SimpleFileOptions::default())
            .unwrap();
        zip.write_all(b"a").unwrap();
        zip.finish().unwrap();
        assert!(super::super::dedup_cache::content_digest(
            &files::fingerprint(&path).unwrap(),
            &AtomicBool::new(false),
            &mut |_| {}
        )
        .is_err());
    }
}
