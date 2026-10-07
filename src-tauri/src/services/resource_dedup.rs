use super::resource_files::{self as files, DiskFile, REFERENCED_DIR};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashMap, HashSet};
use std::io::Read;
use std::path::Path;

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
}

#[derive(Debug, Clone)]
pub struct DedupSnapshot {
    pub summary: DedupSummary,
    pub groups: Vec<DuplicateGroup>,
    pub files: Vec<DiskFile>,
    pub excluded: HashSet<String>,
    pub hashes: HashMap<String, String>,
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

pub fn scan(root: &Path, excluded: HashSet<String>) -> Result<DedupSnapshot, String> {
    let root = files::canonical_dir(root)?;
    let (disk_files, mut warnings) = files::inventory(&root, &excluded)?;
    let mut referenced: HashMap<String, Vec<String>> = HashMap::new();
    let mut invalid = HashSet::new();
    let mut families: BTreeMap<String, Vec<&DiskFile>> = BTreeMap::new();
    let mut ordinary: BTreeMap<(String, u64), Vec<&DiskFile>> = BTreeMap::new();
    for file in &disk_files {
        let path = Path::new(&file.path);
        if path
            .extension()
            .map(|e| e.eq_ignore_ascii_case("var"))
            .unwrap_or(false)
        {
            let owner = files::package_name(path)
                .map(|p| p.id)
                .unwrap_or_else(|| path.file_name().unwrap().to_string_lossy().into_owned());
            match read_dependencies(path) {
                Ok(deps) => {
                    for dep in deps {
                        referenced.entry(dep).or_default().push(owner.clone());
                    }
                }
                Err(e) => {
                    invalid.insert(file.path.clone());
                    warnings.push(format!(
                        "无法读取依赖，保留该文件，请检查: {} ({})",
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
        for file in members {
            let package = files::package_name(Path::new(&file.path)).unwrap();
            let refs = referenced
                .get(&package.id.to_lowercase())
                .cloned()
                .unwrap_or_default();
            let first_version = seen_versions.insert(package.version);
            let keep = invalid_family
                || (first_version
                    && (package.version == highest || !refs.is_empty() || !invalid.is_empty()));
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
            let hash = files::hash_file(file)?;
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
    };
    Ok(DedupSnapshot {
        summary,
        groups,
        files: disk_files,
        excluded,
        hashes,
    })
}

fn modified_number(file: &DiskFile) -> u128 {
    file.modified.parse().unwrap_or_default()
}
fn instance(
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
    }
}
fn group(
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
pub fn validate_selection(
    snapshot: &DedupSnapshot,
    selected: &[String],
) -> Result<Vec<DuplicateInstance>, String> {
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
    for (path, expected) in &snapshot.hashes {
        let file = snapshot
            .files
            .iter()
            .find(|f| &f.path == path)
            .ok_or("扫描快照不完整")?;
        if files::hash_file(file)? != *expected {
            return Err(format!("文件内容已变化: {}", path));
        }
    }
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    use files::tests::TestDir;
    use std::io::Write;
    use std::path::PathBuf;
    fn var(dir: &TestDir, name: &str, dependencies: serde_json::Value, seconds: u64) -> PathBuf {
        let path = dir.0.join(name);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        let mut zip = zip::ZipWriter::new(std::fs::File::create(&path).unwrap());
        zip.start_file("meta.json", zip::write::SimpleFileOptions::default())
            .unwrap();
        zip.write_all(
            serde_json::json!({"dependencies": dependencies})
                .to_string()
                .as_bytes(),
        )
        .unwrap();
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
