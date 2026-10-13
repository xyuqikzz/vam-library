use super::resource_files::{self as files, DiskFile, REFERENCED_DIR};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashMap, HashSet};
use std::io::Read;
use std::path::Path;
use std::sync::atomic::AtomicBool;
use super::dedup_cache::{Cache, check};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DuplicateInstance {
    pub package_id: String,
    pub file_path: String,
    pub size_bytes: u64,
    pub modified_time: String,
    pub version: Option<u64>,
    pub is_recommended_keep: bool,
    pub referenced_by: Vec<String>,
    pub archive_destination: Option<String>,
    pub source_type: String,
    pub link_type: Option<String>,
    pub protection_reason: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct DuplicateGroup {
    pub id: String,
    pub strategy: String,
    pub resource_path: String,
    pub total_wasted_bytes: u64,
    pub file_count: usize,
    pub instances: Vec<DuplicateInstance>,
}

#[derive(Debug, Clone, Serialize)]
pub struct DedupSummary {
    pub scan_id: String,
    pub root: String,
    pub duplicate_groups: usize,
    pub total_wasted_bytes: u64,
    pub safe_to_clean_count: usize,
    pub archive_count: usize,
    pub total_files: usize,
    pub scanned_files: usize,
    pub warnings: Vec<String>,
    pub mode: String,
    pub cache_hits: usize,
    pub bytes_read: u64,
    pub elapsed_ms: u64,
}

#[derive(Debug, Clone)]
pub struct DedupSnapshot {
    pub summary: DedupSummary,
    pub groups: Vec<DuplicateGroup>,
    pub files: Vec<DiskFile>,
    pub excluded: HashSet<String>,
    pub hashes: HashMap<String, String>,
    pub content_hashes: HashMap<String, String>,
    pub witnesses: HashMap<String, String>,
    pub verification_stamps: HashMap<String, String>,
}

fn read_dependencies(path: &Path) -> Result<Vec<String>, String> {
    let input = std::fs::File::open(path).map_err(|e| e.to_string())?;
    let mut zip = zip::ZipArchive::new(input).map_err(|e| e.to_string())?;
    let meta = zip.by_name("meta.json").map_err(|e| e.to_string())?;
    if meta.size() > 16 * 1024 * 1024 {
        return Err("meta.json 过大".into());
    }
    let value: serde_json::Value =
        serde_json::from_reader(meta.take(16 * 1024 * 1024)).map_err(|e| e.to_string())?;
    let mut deps = Vec::new();
    collect_dependencies(&value, &mut deps);
    Ok(deps)
}

fn collect_dependencies(value: &serde_json::Value, deps: &mut Vec<String>) {
    if let Some(map) = value.get("dependencies").and_then(|v| v.as_object()) {
        for (id, child) in map {
            // `latest` references are satisfied by the newest version. Numeric references
            // must keep that exact package version available to VaM.
            if id
                .rsplit_once('.')
                .and_then(|(_, v)| v.parse::<u64>().ok())
                .is_some()
            {
                deps.push(id.to_lowercase());
            }
            collect_dependencies(child, deps);
        }
    }
}

#[cfg(test)]
pub fn scan(root: &Path, excluded: HashSet<String>) -> Result<DedupSnapshot, String> {
    let mut cache = Cache::load(None, root)?;
    scan_cached(root, excluded, &mut cache, &AtomicBool::new(false), &mut |_,_,_,_| {})
}

pub(super) fn scan_cached(root: &Path, excluded: HashSet<String>, cache: &mut Cache<'_>, cancel: &AtomicBool,
    progress: &mut impl FnMut(&str, usize, usize, u64)) -> Result<DedupSnapshot, String> {
    check(cancel)?;
    progress("inventory", 0, 0, 0);
    let root = files::canonical_dir(root)?;
    let (disk_files, mut warnings) = files::inventory(&root, &excluded)?;
    let mut referenced: HashMap<String, Vec<String>> = HashMap::new();
    let mut invalid = HashSet::new();
    let mut families: BTreeMap<String, Vec<&DiskFile>> = BTreeMap::new();
    let mut ordinary: BTreeMap<(String, u64), Vec<&DiskFile>> = BTreeMap::new();
    for (index, file) in disk_files.iter().enumerate() {
        check(cancel)?;
        progress("metadata", index, disk_files.len(), cache.bytes);
        let path = Path::new(&file.path);
        if path
            .extension()
            .map(|e| e.eq_ignore_ascii_case("var"))
            .unwrap_or(false)
        {
            let owner = files::package_name(path)
                .map(|p| p.id)
                .unwrap_or_else(|| path.file_name().unwrap().to_string_lossy().into_owned());
            match cache.dependencies(file, || read_dependencies(path))? {
                Ok(deps) => {
                    for dep in deps {
                        referenced.entry(dep).or_default().push(owner.clone());
                    }
                }
                Err(e) => {
                    invalid.insert(file.path.clone());
                    warnings.push(format!(
                        "无法读取依赖，保留各版本，仅清理内容完全相同的同版本副本，请检查: {} ({})",
                        file.path, e
                    ));
                }
            }
            files::verify(file)?;
        }
        if let Some(package) = files::package_name(path) {
            families
                .entry(package.family.to_lowercase())
                .or_default()
                .push(file);
        } else {
            ordinary
                .entry((
                    path.file_name().unwrap().to_string_lossy().to_lowercase(),
                    file.size,
                ))
                .or_default()
                .push(file);
        }
    }
    if !invalid.is_empty() {
        warnings.push("部分 VAR 的依赖无法读取：暂时保留各资源的所有版本，仅清理同版本副本；已确认被依赖的旧版本仍会归档。修复异常包后可重新扫描。".into());
    }
    let mut groups = Vec::new();
    let mut hashes = HashMap::new();
    for (family, mut members) in families {
        if members.len() < 2 {
            continue;
        }
        members.sort_by_cached_key(|file| {
            (
                std::cmp::Reverse(files::package_name(Path::new(&file.path)).unwrap().version),
                std::cmp::Reverse(modified_number(file)),
                file.path.as_str(),
            )
        });
        let highest = files::package_name(Path::new(&members[0].path))
            .unwrap()
            .version;
        let mut seen_versions = HashSet::new();
        let mut instances = Vec::new();
        let invalid_family = members.iter().any(|f| invalid.contains(&f.path));
        // Unreadable metadata protects versions, not every physical copy. Matching
        // archive bytes preserve even dependencies that could not be parsed.
        let exact_duplicates = if invalid_family {
            exact_var_duplicates(&members, &mut hashes, cache, cancel, progress)?
        } else {
            HashSet::new()
        };
        for file in members {
            let package = files::package_name(Path::new(&file.path)).unwrap();
            let refs = referenced
                .get(&package.id.to_lowercase())
                .cloned()
                .unwrap_or_default();
            let first_version = seen_versions.insert(package.version);
            let keep = if invalid_family {
                !exact_duplicates.contains(&file.path)
            } else {
                first_version
                    && (package.version == highest || !refs.is_empty() || !invalid.is_empty())
            };
            let archive = if keep
                && first_version
                && package.version < highest
                && !refs.is_empty()
                && !invalid_family
            {
                let destination = root
                    .join(REFERENCED_DIR)
                    .join(Path::new(&file.path).file_name().unwrap());
                (files::path_key(&file.path) != files::path_key(&destination.to_string_lossy()))
                    .then(|| destination.to_string_lossy().into_owned())
            } else {
                None
            };
            // VAR identity comes from creator/name/version, not a hash of hundreds of GB.
            instances.push(instance(
                file,
                package.id,
                Some(package.version),
                keep,
                refs,
                archive,
            ));
        }
        if invalid_family {
            let mut kept_versions = HashSet::new();
            let mut warned_versions = HashSet::new();
            for item in instances.iter().filter(|i| i.is_recommended_keep) {
                let version = item.version.unwrap();
                if !kept_versions.insert(version) && warned_versions.insert(version) {
                    warnings.push(format!("同包同版本内容不同，已保留不同内容，请手动检查以避免游戏重复包冲突: {family}.{version}"));
                }
            }
        }
        groups.push(group(format!("var:{family}"), "version", family, instances));
    }
    for ((name, _), candidates) in ordinary {
        if candidates.len() < 2 {
            continue;
        }
        let mut by_hash: BTreeMap<String, Vec<&DiskFile>> = BTreeMap::new();
        for file in candidates {
            if invalid.contains(&file.path) {
                continue;
            }
            let hash = cache.raw(file, cancel, &mut |n| progress("hashing", 0, 0, n))?;
            hashes.insert(file.path.clone(), hash.clone());
            by_hash.entry(hash).or_default().push(file);
        }
        for (hash, mut members) in by_hash {
            if members.len() < 2 {
                continue;
            }
            members.sort_by_cached_key(|file| {
                (std::cmp::Reverse(modified_number(file)), file.path.as_str())
            });
            let instances = members
                .into_iter()
                .enumerate()
                .map(|(i, f)| {
                    instance(
                        f,
                        Path::new(&f.path)
                            .file_name()
                            .unwrap()
                            .to_string_lossy()
                            .into_owned(),
                        None,
                        i == 0,
                        vec![],
                        None,
                    )
                })
                .collect();
            groups.push(group(
                format!("file:{name}:{hash}"),
                "exact",
                name.clone(),
                instances,
            ));
        }
    }
    let summary = DedupSummary {
        scan_id: format!(
            "dedup-{}",
            chrono::Utc::now().timestamp_nanos_opt().unwrap_or_default()
        ),
        root: root.to_string_lossy().into_owned(),
        duplicate_groups: groups.len(),
        total_wasted_bytes: groups.iter().map(|g| g.total_wasted_bytes).sum(),
        safe_to_clean_count: groups
            .iter()
            .flat_map(|g| &g.instances)
            .filter(|i| !i.is_recommended_keep)
            .count(),
        archive_count: groups
            .iter()
            .flat_map(|g| &g.instances)
            .filter(|i| i.archive_destination.is_some())
            .count(),
        total_files: disk_files.len(),
        scanned_files: disk_files.len(),
        warnings,
        mode: "standard".into(), cache_hits: cache.hits, bytes_read: cache.bytes, elapsed_ms: 0,
    };
    let verification_stamps = disk_files.iter().filter_map(|file| {
        cache.verification_stamp(file, hashes.get(&file.path), None)
            .map(|stamp| (file.path.clone(), stamp))
    }).collect();
    Ok(DedupSnapshot {
        summary,
        groups,
        files: disk_files,
        excluded,
        hashes,
        content_hashes: HashMap::new(), witnesses: HashMap::new(),
        verification_stamps,
    })
}

fn exact_var_duplicates(
    members: &[&DiskFile],
    hashes: &mut HashMap<String, String>,
    cache: &mut Cache<'_>, cancel: &AtomicBool,
    progress: &mut impl FnMut(&str, usize, usize, u64),
) -> Result<HashSet<String>, String> {
    let mut candidates: BTreeMap<(u64, u64), Vec<&DiskFile>> = BTreeMap::new();
    for file in members {
        let version = files::package_name(Path::new(&file.path)).unwrap().version;
        candidates
            .entry((version, file.size))
            .or_default()
            .push(file);
    }
    let mut duplicates = HashSet::new();
    for files in candidates.values().filter(|files| files.len() > 1) {
        let mut seen_hashes = HashSet::new();
        for file in files {
            let hash = cache.raw(file, cancel, &mut |n| progress("hashing", 0, 0, n))?;
            if !seen_hashes.insert(hash.clone()) {
                duplicates.insert(file.path.clone());
            }
            hashes.insert(file.path.clone(), hash);
        }
    }
    Ok(duplicates)
}

fn modified_number(file: &DiskFile) -> u128 {
    file.modified.parse().unwrap_or_default()
}
pub(super) fn instance(
    file: &DiskFile,
    id: String,
    version: Option<u64>,
    keep: bool,
    refs: Vec<String>,
    archive: Option<String>,
) -> DuplicateInstance {
    let nanos = modified_number(file);
    let modified_time = chrono::DateTime::from_timestamp(
        (nanos / 1_000_000_000) as i64,
        (nanos % 1_000_000_000) as u32,
    )
    .map(|v| v.to_rfc3339())
    .unwrap_or_default();
    DuplicateInstance {
        package_id: id,
        file_path: file.path.clone(),
        size_bytes: file.size,
        modified_time,
        version,
        is_recommended_keep: keep,
        referenced_by: refs,
        archive_destination: archive,
        source_type: "real_file".into(),
        link_type: None,
        protection_reason: None,
    }
}
pub(super) fn group(
    id: String,
    strategy: &str,
    name: String,
    instances: Vec<DuplicateInstance>,
) -> DuplicateGroup {
    DuplicateGroup {
        id,
        strategy: strategy.into(),
        resource_path: name,
        total_wasted_bytes: instances
            .iter()
            .filter(|i| !i.is_recommended_keep)
            .map(|i| i.size_bytes)
            .sum(),
        file_count: instances.len(),
        instances,
    }
}

/// Reject stale previews and paths not belonging to the server-side plan. Recommended
/// copies include every pinned version, so deleting the final required copy is impossible.
#[cfg(test)]
pub fn validate_selection(
    snapshot: &DedupSnapshot,
    selected: &[String],
) -> Result<Vec<DuplicateInstance>, String> {
    validate_selection_inner(snapshot, selected, None, &mut |_, _, _, _| {})
}

/// Own the write-blocking handles through validation, dependency checks and moves.
/// Private storage prevents callers from declaring unpinned paths safe to reuse.
pub struct SelectionPins(HashMap<String, std::fs::File>);

pub fn pin_selection(snapshot: &DedupSnapshot, selected: &[String]) -> Result<SelectionPins, String> {
    let mut pins = HashMap::new();
    for path in verification_paths(snapshot, selected) {
        match files::pin_readonly(Path::new(&path)) {
            Some(pin) => { pins.insert(path, pin); }
            None if cfg!(windows) => return Err(format!("文件正在使用或无法锁定，请关闭后重试: {path}")),
            None => {},
        }
    }
    Ok(SelectionPins(pins))
}

#[cfg(test)]
pub fn validate_pinned_selection(
    snapshot: &DedupSnapshot,
    selected: &[String],
    pins: &SelectionPins,
) -> Result<Vec<DuplicateInstance>, String> {
    validate_pinned_selection_with_progress(snapshot, selected, pins, &mut |_, _, _, _| {})
}

pub fn validate_pinned_selection_with_progress(
    snapshot: &DedupSnapshot,
    selected: &[String],
    pins: &SelectionPins,
    progress: &mut impl FnMut(&str, usize, usize, &str),
) -> Result<Vec<DuplicateInstance>, String> {
    validate_selection_inner(snapshot, selected, Some(pins), progress)
}

fn validate_selection_inner(
    snapshot: &DedupSnapshot,
    selected: &[String],
    pins: Option<&SelectionPins>,
    progress: &mut impl FnMut(&str, usize, usize, &str),
) -> Result<Vec<DuplicateInstance>, String> {
    progress("inventory", 0, 0, "");
    let (current, _) = files::inventory(Path::new(&snapshot.summary.root), &snapshot.excluded)?;
    if current != snapshot.files {
        return Err("资源目录已变化，请重新扫描后整理".into());
    }
    let mut seen = HashSet::new();
    let mut result = Vec::new();
    for path in selected {
        if !seen.insert(path) {
            return Err("重复的清理路径".into());
        }
        let item = snapshot
            .groups
            .iter()
            .flat_map(|g| &g.instances)
            .find(|i| &i.file_path == path)
            .ok_or("清理路径不在本次扫描中")?;
        if item.is_recommended_keep {
            return Err(format!("不能清理推荐保留/依赖保护文件: {}", path));
        }
        result.push(item.clone());
    }
    let relevant = verification_paths(snapshot, selected);
    for (index, path) in relevant.iter().enumerate() {
        progress("verifying", index, relevant.len(), path);
        let expected = snapshot.hashes.get(path);
        let content = snapshot.content_hashes.get(path);
        if expected.is_none() && content.is_none() { continue; }
        let file = snapshot
            .files
            .iter()
            .find(|f| &f.path == path)
            .ok_or("扫描快照不完整")?;
        if let (Some(pin), Some(stamp)) = (
            pins.and_then(|pins| pins.0.get(path)), snapshot.verification_stamps.get(path),
        ) {
            // Check both handle and path identity: a rename must not let an old
            // handle authorize removal of the new file at the same path.
            use super::cleanup_reference_cache::{change_stamp, pinned_change_stamp};
            if pinned_change_stamp(pin)?.as_ref() == Some(stamp)
                && change_stamp(Path::new(path))?.as_ref() == Some(stamp)
            {
                continue;
            }
        }
        if let Some(expected) = expected {
            if files::hash_file_with_progress(file, &mut |_| {
                progress("verifying", index, relevant.len(), path); Ok(())
            })? != *expected { return Err(format!("文件内容已变化: {}", path)); }
        } else if let Some(expected) = content {
            if super::dedup_cache::content_digest(file, &AtomicBool::new(false), &mut |_| {
                progress("verifying", index, relevant.len(), path);
            })? != *expected {
                return Err(format!("文件内容已变化: {}", path));
            }
        }
    }
    progress("verifying", relevant.len(), relevant.len(), "");
    Ok(result)
}

/// Include every recommended survivor in affected groups, never unrelated hashes.
pub fn verification_paths(snapshot: &DedupSnapshot, selected: &[String]) -> HashSet<String> {
    let mut paths: HashSet<_> = selected.iter().cloned().collect();
    paths.extend(snapshot.groups.iter().flat_map(|g| &g.instances).filter(|i| i.archive_destination.is_some()).map(|i| i.file_path.clone()));
    for group in &snapshot.groups {
        if group.instances.iter().any(|i| paths.contains(&i.file_path)) {
            paths.extend(group.instances.iter().filter(|i| i.is_recommended_keep).map(|i| i.file_path.clone()));
        }
    }
    for path in selected {
        if let Some(keep) = snapshot.witnesses.get(path) { paths.insert(keep.clone()); }
    }
    paths
}

#[cfg(test)]
mod tests {
    use super::*;
    use files::tests::TestDir;
    use std::io::Write;
    use std::path::PathBuf;
    fn var(dir: &TestDir, name: &str, dependencies: serde_json::Value, seconds: u64) -> PathBuf {
        var_with_meta(
            dir,
            name,
            serde_json::json!({"dependencies": dependencies})
                .to_string()
                .as_bytes(),
            seconds,
        )
    }
    fn var_with_meta(dir: &TestDir, name: &str, meta: &[u8], seconds: u64) -> PathBuf {
        let path = dir.0.join(name);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        let mut zip = zip::ZipWriter::new(std::fs::File::create(&path).unwrap());
        zip.start_file("meta.json", zip::write::SimpleFileOptions::default())
            .unwrap();
        zip.write_all(meta).unwrap();
        zip.finish()
            .unwrap()
            .set_times(std::fs::FileTimes::new().set_modified(
                std::time::SystemTime::UNIX_EPOCH + std::time::Duration::from_secs(seconds),
            ))
            .unwrap();
        path
    }
    #[test]
    fn highest_version_newest_copy_and_pinned_archive() {
        let d = TestDir::new();
        var(&d, "old/A.Asset.2.var", serde_json::json!({}), 100);
        var(&d, "A.Asset.10.var", serde_json::json!({}), 100);
        let newest = var(&d, "new/A.Asset.10.var", serde_json::json!({}), 200);
        let pinned = var(&d, "A.Asset.1.var", serde_json::json!({}), 500);
        var(
            &d,
            "B.Scene.1.var",
            serde_json::json!({"A.Asset.1":{}, "A.Asset.latest":{}}),
            100,
        );
        let snapshot = scan(&d.0, HashSet::new()).unwrap();
        let group = &snapshot.groups[0];
        assert_eq!(snapshot.summary.safe_to_clean_count, 2);
        assert!(
            group
                .instances
                .iter()
                .find(|i| Path::new(&i.file_path) == newest.as_path())
                .unwrap()
                .is_recommended_keep
        );
        let p = group
            .instances
            .iter()
            .find(|i| Path::new(&i.file_path) == pinned.as_path())
            .unwrap();
        assert!(p
            .archive_destination
            .as_ref()
            .unwrap()
            .contains(REFERENCED_DIR));
        assert!(validate_selection(&snapshot, &[p.file_path.clone()]).is_err());
        let deletions: Vec<_> = group
            .instances
            .iter()
            .filter(|i| !i.is_recommended_keep)
            .map(|i| i.file_path.clone())
            .collect();
        assert_eq!(validate_selection(&snapshot, &deletions).unwrap().len(), 2);
        d.write("new.txt", b"new");
        assert!(validate_selection(&snapshot, &deletions).is_err());
    }
    #[test]
    fn ordinary_files_require_equal_names_and_content() {
        let d = TestDir::new();
        d.write("a/readme.txt", b"same");
        d.write("b/readme.txt", b"same");
        d.write("c/readme.txt", b"diff");
        d.write("d/other.txt", b"same");
        let s = scan(&d.0, HashSet::new()).unwrap();
        assert_eq!(s.summary.total_files, 4);
        assert_eq!(s.summary.safe_to_clean_count, 1);
        assert_eq!(s.groups[0].instances.len(), 2);
    }
    #[test]
    fn invalid_packages_are_not_discarded() {
        let d = TestDir::new();
        var(&d, "A.X.1.var", serde_json::json!({}), 100);
        d.write("A.X.2.var", b"broken");
        let s = scan(&d.0, HashSet::new()).unwrap();
        assert_eq!(s.summary.safe_to_clean_count, 0);
        assert!(!s.summary.warnings.is_empty());
    }

    #[test]
    fn malformed_same_version_copies_keep_one_even_when_pinned() {
        let d = TestDir::new();
        let meta = br#"{"description":"bad "quote","dependencies":{}}"#;
        let older = var_with_meta(&d, "其他/Amaimon.Bhaviour.2.var", meta, 100);
        let newest = var_with_meta(&d, "场景/Amaimon.Bhaviour.2.var", meta, 200);
        var(&d, "Amaimon.Bhaviour.1.var", serde_json::json!({}), 100);
        var(
            &d,
            "B.Scene.1.var",
            serde_json::json!({"Amaimon.Bhaviour.2":{}}),
            100,
        );

        let snapshot = scan(&d.0, HashSet::new()).unwrap();
        let group = &snapshot.groups[0];
        let same_version: Vec<_> = group
            .instances
            .iter()
            .filter(|i| i.version == Some(2))
            .collect();
        assert_eq!(
            same_version
                .iter()
                .filter(|i| i.is_recommended_keep)
                .count(),
            1
        );
        assert!(
            same_version
                .iter()
                .find(|i| Path::new(&i.file_path) == newest)
                .unwrap()
                .is_recommended_keep
        );
        assert!(same_version
            .iter()
            .all(|i| i.referenced_by == ["B.Scene.1"]));
        assert!(
            group
                .instances
                .iter()
                .find(|i| i.version == Some(1))
                .unwrap()
                .is_recommended_keep
        );
        assert_eq!(snapshot.summary.safe_to_clean_count, 1);
        assert_eq!(
            snapshot.summary.total_wasted_bytes,
            std::fs::metadata(&older).unwrap().len()
        );
        assert_eq!(snapshot.summary.archive_count, 0);
        assert_eq!(snapshot.hashes.len(), 2);
        let cleanup_path = same_version
            .iter()
            .find(|i| !i.is_recommended_keep)
            .unwrap()
            .file_path
            .clone();
        let keep_path = same_version
            .iter()
            .find(|i| i.is_recommended_keep)
            .unwrap()
            .file_path
            .clone();
        assert_eq!(
            validate_selection(&snapshot, &[cleanup_path])
                .unwrap()
                .len(),
            1
        );
        assert!(validate_selection(&snapshot, &[keep_path]).is_err());
    }

    #[test]
    fn malformed_same_version_content_conflicts_preserve_distinct_files() {
        let d = TestDir::new();
        var_with_meta(&d, "a/A.X.1.var", b"broken-a", 100);
        let newest = var_with_meta(&d, "b/A.X.1.var", b"broken-a", 200);
        let different = var_with_meta(&d, "c/A.X.1.var", b"broken-b", 300);
        let larger = var_with_meta(&d, "d/A.X.1.var", b"different length", 400);

        let snapshot = scan(&d.0, HashSet::new()).unwrap();
        let kept: Vec<_> = snapshot.groups[0]
            .instances
            .iter()
            .filter(|i| i.is_recommended_keep)
            .collect();
        assert_eq!(snapshot.summary.safe_to_clean_count, 1);
        assert_eq!(kept.len(), 3);
        for path in [&newest, &different, &larger] {
            assert!(kept.iter().any(|i| Path::new(&i.file_path) == path));
        }
        assert_eq!(snapshot.hashes.len(), 3);
        assert!(snapshot
            .summary
            .warnings
            .iter()
            .any(|w| w.contains("同包同版本内容不同")));
    }

    #[test]
    fn malformed_duplicate_content_is_rechecked_even_with_unchanged_size_and_mtime() {
        let d = TestDir::new();
        let path = var_with_meta(&d, "a/A.X.1.var", b"broken-a", 100);
        var_with_meta(&d, "b/A.X.1.var", b"broken-a", 200);
        let snapshot = scan(&d.0, HashSet::new()).unwrap();
        let selected = snapshot.groups[0]
            .instances
            .iter()
            .find(|i| !i.is_recommended_keep)
            .unwrap()
            .file_path
            .clone();
        let modified = std::fs::metadata(&path).unwrap().modified().unwrap();
        let mut bytes = std::fs::read(&path).unwrap();
        bytes[0] ^= 1;
        std::fs::write(&path, bytes).unwrap();
        std::fs::OpenOptions::new()
            .write(true)
            .open(&path)
            .unwrap()
            .set_times(std::fs::FileTimes::new().set_modified(modified))
            .unwrap();
        assert_eq!(
            files::inventory(&d.0, &HashSet::new()).unwrap().0,
            snapshot.files
        );
        assert!(validate_selection(&snapshot, &[selected])
            .unwrap_err()
            .contains("文件内容已变化"));
    }

    #[test]
    fn unreadable_dependencies_preserve_other_versions_and_nested_pins_are_archived() {
        let d = TestDir::new();
        var(&d, "A.X.1.var", serde_json::json!({}), 100);
        var(&d, "A.X.2.var", serde_json::json!({}), 100);
        var(
            &d,
            "B.Scene.1.var",
            serde_json::json!({"C.Parent.latest":{"dependencies":{"A.X.1":{}}}}),
            100,
        );
        var(&d, "Z.Other.1.var", serde_json::json!({}), 100);
        var(&d, "Z.Other.2.var", serde_json::json!({}), 100);
        d.write("broken.var", b"broken");
        let s = scan(&d.0, HashSet::new()).unwrap();
        assert_eq!(s.summary.safe_to_clean_count, 0);
        assert_eq!(s.summary.archive_count, 1);
        assert!(s
            .groups
            .iter()
            .flat_map(|g| &g.instances)
            .all(|i| i.is_recommended_keep));
    }
}
