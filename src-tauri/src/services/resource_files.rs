//! Disk inventory shared by migration and deduplication. Never follows directory links.
use std::collections::HashSet;
use std::fs::{self, File, OpenOptions};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::time::SystemTime;

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

pub const REFERENCED_DIR: &str = "依赖旧版本";
static MAINTENANCE_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());
pub fn maintenance_lock() -> Result<std::sync::MutexGuard<'static, ()>, String> {
    MAINTENANCE_LOCK.lock().map_err(|e| e.to_string())
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DiskFile {
    pub path: String,
    pub size: u64,
    pub modified: String,
}

#[derive(Debug, Clone)]
pub struct PackageName {
    pub id: String,
    pub family: String,
    pub creator: String,
    pub version: u64,
}

pub fn package_name(path: &Path) -> Option<PackageName> {
    if !path.extension()?.eq_ignore_ascii_case("var") {
        return None;
    }
    let id = path.file_stem()?.to_str()?;
    let (family, version) = id.rsplit_once('.')?;
    let (creator, name) = family.split_once('.')?;
    if creator.is_empty() || name.is_empty() {
        return None;
    }
    Some(PackageName {
        id: id.into(),
        family: family.into(),
        creator: creator.into(),
        version: version.parse().ok()?,
    })
}

pub fn path_key(path: &str) -> String {
    path.trim_start_matches(r"\\?\")
        .replace('\\', "/")
        .to_lowercase()
}

pub fn canonical_dir(path: &Path) -> Result<PathBuf, String> {
    if !path.is_dir() {
        return Err(format!("目录不存在: {}", path.display()));
    }
    ensure_no_links(path)?;
    let result = fs::canonicalize(path).map_err(|e| e.to_string())?;
    Ok(PathBuf::from(
        result.to_string_lossy().trim_start_matches(r"\\?\"),
    ))
}

pub fn is_link(metadata: &fs::Metadata) -> bool {
    if metadata.file_type().is_symlink() {
        return true;
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        if metadata.file_attributes() & 0x400 != 0 {
            return true;
        }
    }
    false
}

pub fn ensure_no_links(path: &Path) -> Result<(), String> {
    for ancestor in path.ancestors() {
        match fs::symlink_metadata(ancestor) {
            Ok(m) if is_link(&m) => {
                return Err(format!("跳过链接或重解析路径: {}", ancestor.display()))
            }
            Ok(_) => {}
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => return Err(format!("读取路径失败 {}: {}", ancestor.display(), e)),
        }
    }
    Ok(())
}

pub fn fingerprint(path: &Path) -> Result<DiskFile, String> {
    ensure_no_links(path)?;
    let m = fs::symlink_metadata(path)
        .map_err(|e| format!("读取文件失败 {}: {}", path.display(), e))?;
    if !m.is_file() {
        return Err(format!("不是普通文件: {}", path.display()));
    }
    let modified = m
        .modified()
        .map_err(|e| e.to_string())?
        .duration_since(SystemTime::UNIX_EPOCH)
        .map_err(|e| e.to_string())?
        .as_nanos()
        .to_string();
    Ok(DiskFile {
        path: path.to_string_lossy().into(),
        size: m.len(),
        modified,
    })
}

pub fn inventory(
    root: &Path,
    excluded: &HashSet<String>,
) -> Result<(Vec<DiskFile>, Vec<String>), String> {
    let root = canonical_dir(root)?;
    let mut warnings = Vec::new();
    let mut files = Vec::new();
    let mut walker = walkdir::WalkDir::new(&root).follow_links(false).into_iter();
    while let Some(entry) = walker.next() {
        let entry = entry.map_err(|e| format!("目录扫描不完整，已停止: {}", e))?;
        let m = fs::symlink_metadata(entry.path()).map_err(|e| e.to_string())?;
        let name = entry.file_name().to_string_lossy().to_lowercase();
        let internal = entry.depth() > 0
            && m.is_dir()
            && matches!(
                name.as_str(),
                ".trash" | ".downloads" | ".vamboxlibrary-downloads"
            );
        if is_link(&m) || internal || excluded.contains(&path_key(&entry.path().to_string_lossy()))
        {
            warnings.push(format!(
                "已跳过链接、托管映射或内部目录: {}",
                entry.path().display()
            ));
            if entry.file_type().is_dir() {
                walker.skip_current_dir();
            }
            continue;
        }
        if m.is_file() {
            files.push(fingerprint(entry.path())?);
        }
    }
    files.sort_by(|a, b| a.path.cmp(&b.path));
    Ok((files, warnings))
}

pub fn verify(file: &DiskFile) -> Result<(), String> {
    if fingerprint(Path::new(&file.path))? != *file {
        return Err(format!("文件已变化，请重新扫描/预览: {}", file.path));
    }
    Ok(())
}

pub fn hash_file(file: &DiskFile) -> Result<String, String> {
    verify(file)?;
    let mut input = File::open(&file.path).map_err(|e| e.to_string())?;
    let mut hash = Sha256::new();
    let mut buffer = [0u8; 128 * 1024];
    loop {
        let n = input.read(&mut buffer).map_err(|e| e.to_string())?;
        if n == 0 {
            break;
        }
        hash.update(&buffer[..n]);
    }
    verify(file)?;
    Ok(hex::encode(hash.finalize()))
}

pub fn update_index_path(
    conn: &rusqlite::Connection,
    source: &str,
    destination: &str,
) -> Result<(), crate::errors::AppError> {
    let key = path_key(source);
    // Multiple physical copies share a package ID. Only move the row whose path moved.
    conn.execute("UPDATE packages SET file_path=?1, updated_at=datetime('now') WHERE lower(replace(file_path, '\\', '/'))=?2", rusqlite::params![destination,key])?;
    conn.execute(
        "DELETE FROM physical_packages WHERE lower(replace(file_path, '\\', '/'))=?1",
        [path_key(destination)],
    )?;
    conn.execute(
        "UPDATE physical_packages SET file_path=?1 WHERE lower(replace(file_path, '\\', '/'))=?2",
        rusqlite::params![destination, key],
    )?;
    Ok(())
}

/// Index a restored/kept package without the downloader's delete-on-parse-error behavior.
pub fn index_package(
    conn: &rusqlite::Connection,
    path: &Path,
) -> Result<(), crate::errors::AppError> {
    if package_name(path).is_none() {
        return Ok(());
    }
    let pkg = super::var_parser::parse_var_file(path)?;
    index_parsed_package(conn, &pkg)
}

pub fn index_parsed_package(
    conn: &rusqlite::Connection,
    pkg: &crate::models::var_package::VarPackage,
) -> Result<(), crate::errors::AppError> {
    let types = serde_json::to_string(&pkg.resource_types)?;
    let meta_json = serde_json::to_string(&pkg.meta)?;
    let license = pkg
        .meta
        .as_ref()
        .map(|m| m.license_type.as_str())
        .unwrap_or_default();
    conn.execute("INSERT INTO packages (id,creator,name,version,file_path,size_bytes,resource_types,meta_json,license_type,scan_time,file_created_time,description,credits,instructions,promotional_link) VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,?15) ON CONFLICT(id) DO UPDATE SET file_path=excluded.file_path,size_bytes=excluded.size_bytes,resource_types=excluded.resource_types,meta_json=excluded.meta_json,license_type=excluded.license_type,description=excluded.description,credits=excluded.credits,instructions=excluded.instructions,promotional_link=excluded.promotional_link,updated_at=datetime('now')",
        rusqlite::params![pkg.id,pkg.creator,pkg.name,pkg.version,pkg.file_path,pkg.size_bytes,types,meta_json,license,pkg.scan_time,pkg.created_time,pkg.meta.as_ref().and_then(|m|m.description.as_ref()),pkg.meta.as_ref().and_then(|m|m.credits.as_ref()),pkg.meta.as_ref().and_then(|m|m.instructions.as_ref()),pkg.meta.as_ref().and_then(|m|m.promotional_link.as_ref())])?;
    conn.execute("INSERT INTO physical_packages (file_path,package_id,size_bytes,modified_time,scan_time) VALUES (?1,?2,?3,?4,?5) ON CONFLICT(file_path) DO UPDATE SET package_id=excluded.package_id,size_bytes=excluded.size_bytes,modified_time=excluded.modified_time,scan_status='ok',last_error=NULL,file_md5=NULL",rusqlite::params![pkg.file_path,pkg.id,pkg.size_bytes,pkg.modified_time,pkg.scan_time])?;
    conn.execute("DELETE FROM contents WHERE package_id=?1", [&pkg.id])?;
    conn.execute("DELETE FROM dependencies WHERE package_id=?1", [&pkg.id])?;
    for (name, size) in &pkg.contents {
        conn.execute("INSERT INTO contents (package_id,file_path,resource_type,size_bytes) VALUES (?1,?2,?3,?4)",rusqlite::params![pkg.id,name,crate::models::resource::ResourceType::from_path(name).as_str(),size])?;
    }
    if let Some(meta) = &pkg.meta {
        for id in meta.dependencies.keys() {
            let version = id.rsplit_once('.').map(|(_, v)| v).unwrap_or("latest");
            conn.execute("INSERT OR IGNORE INTO dependencies (package_id,depends_on_id,required_version) VALUES (?1,?2,?3)",rusqlite::params![pkg.id,id,version])?;
        }
    }
    Ok(())
}

/// Reserve the destination atomically; neither move nor copy may overwrite a file.
/// Cross-volume moves copy to a new file, preserve mtime, then remove the source.
pub fn transfer(source: &Path, destination: &Path, action: &str) -> Result<(), String> {
    if !matches!(action, "move" | "copy") {
        return Err("无效文件操作".into());
    }
    ensure_no_links(source)?;
    ensure_no_links(destination)?;
    let before = fingerprint(source)?;
    if fs::symlink_metadata(destination).is_ok() {
        return Err(format!("目标已存在，禁止覆盖: {}", destination.display()));
    }
    if let Some(parent) = destination.parent() {
        fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    if action == "move" && fs::hard_link(source, destination).is_ok() {
        if let Err(e) = fs::remove_file(source) {
            let _ = fs::remove_file(destination);
            return Err(e.to_string());
        }
        return Ok(());
    }
    let mut input = File::open(source).map_err(|e| e.to_string())?;
    let modified = input
        .metadata()
        .and_then(|m| m.modified())
        .map_err(|e| e.to_string())?;
    let mut output = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(destination)
        .map_err(|e| e.to_string())?;
    let result = (|| {
        std::io::copy(&mut input, &mut output).map_err(|e| e.to_string())?;
        output.flush().map_err(|e| e.to_string())?;
        output
            .set_times(fs::FileTimes::new().set_modified(modified))
            .map_err(|e| e.to_string())?;
        output.sync_all().map_err(|e| e.to_string())?;
        verify(&before)?;
        if action == "move" {
            fs::remove_file(source).map_err(|e| e.to_string())?;
        }
        Ok(())
    })();
    drop(output);
    if result.is_err() {
        let _ = fs::remove_file(destination);
    }
    result
}

/// Remove only empty descendants. Never removes root, non-empty directories, or links.
pub fn remove_empty_dirs(root: &Path) -> Result<(usize, Vec<String>), String> {
    remove_empty_dirs_excluding(root, &HashSet::new())
}

pub fn remove_empty_dirs_excluding(
    root: &Path,
    excluded: &HashSet<String>,
) -> Result<(usize, Vec<String>), String> {
    let root = canonical_dir(root)?;
    let mut dirs = Vec::new();
    let mut errors = Vec::new();
    let mut walker = walkdir::WalkDir::new(&root).follow_links(false).into_iter();
    while let Some(entry) = walker.next() {
        let entry = match entry {
            Ok(e) => e,
            Err(e) => {
                errors.push(e.to_string());
                continue;
            }
        };
        let m = match fs::symlink_metadata(entry.path()) {
            Ok(m) => m,
            Err(e) => {
                errors.push(e.to_string());
                continue;
            }
        };
        let internal = matches!(
            entry.file_name().to_string_lossy().to_lowercase().as_str(),
            ".trash" | ".downloads" | ".vamboxlibrary-downloads"
        );
        if is_link(&m) || internal || excluded.contains(&path_key(&entry.path().to_string_lossy()))
        {
            if entry.file_type().is_dir() {
                walker.skip_current_dir();
            }
            continue;
        }
        if entry.depth() > 0 && m.is_dir() {
            dirs.push(entry.path().to_path_buf());
        }
    }
    dirs.sort_by_key(|p| std::cmp::Reverse(p.components().count()));
    let mut count = 0;
    for dir in dirs {
        if !dir.starts_with(&root) || dir == root {
            continue;
        }
        if let Err(e) = ensure_no_links(&dir) {
            errors.push(e);
            continue;
        }
        match fs::read_dir(&dir) {
            Ok(mut entries) => {
                if entries.next().is_none() {
                    match fs::remove_dir(&dir) {
                        Ok(()) => count += 1,
                        Err(e) => errors.push(format!("清理空目录失败 {}: {}", dir.display(), e)),
                    }
                }
            }
            Err(e) => errors.push(e.to_string()),
        }
    }
    Ok((count, errors))
}

#[cfg(test)]
pub mod tests {
    use super::*;
    pub struct TestDir(pub PathBuf);
    impl TestDir {
        pub fn new() -> Self {
            static SEQ: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
            let path = std::env::temp_dir().join(format!(
                "vam-files-{}-{}-{}",
                std::process::id(),
                chrono::Utc::now().timestamp_nanos_opt().unwrap(),
                SEQ.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
            ));
            fs::create_dir_all(&path).unwrap();
            // Windows CI may expose TEMP through an 8.3 alias (RUNNER~1).
            // Seed fixtures and DB rows with the same canonical root used by
            // maintenance plans, so identity checks compare the intended paths.
            Self(canonical_dir(&path).unwrap())
        }
        pub fn write(&self, name: &str, content: &[u8]) -> PathBuf {
            let path = self.0.join(name);
            fs::create_dir_all(path.parent().unwrap()).unwrap();
            fs::write(&path, content).unwrap();
            path
        }
    }
    impl Drop for TestDir {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }
    #[test]
    fn numeric_versions_and_dotted_names() {
        let p = package_name(Path::new("Author.Name.with.dots.12.VAR")).unwrap();
        assert_eq!(p.family, "Author.Name.with.dots");
        assert_eq!(p.version, 12);
        assert!(package_name(Path::new("readme.txt")).is_none());
    }
    #[test]
    fn all_files_empty_dirs_and_no_overwrite() {
        let dir = TestDir::new();
        let a = dir.write("a/readme.txt", b"one");
        let b = dir.write("readme.txt", b"two");
        assert_eq!(inventory(&dir.0, &HashSet::new()).unwrap().0.len(), 2);
        assert!(transfer(&a, &b, "move").is_err());
        assert_eq!(fs::read(&b).unwrap(), b"two");
        fs::create_dir_all(dir.0.join("empty/nested")).unwrap();
        assert_eq!(remove_empty_dirs(&dir.0).unwrap().0, 2);
        assert!(a.exists());
        assert!(dir.0.exists());
    }
    #[test]
    fn copy_preserves_mtime_and_changed_files_are_rejected() {
        let dir = TestDir::new();
        let a = dir.write("a", b"one");
        let before = fingerprint(&a).unwrap();
        transfer(&a, &dir.0.join("b"), "copy").unwrap();
        assert_eq!(
            before.modified,
            fingerprint(&dir.0.join("b")).unwrap().modified
        );
        fs::write(&a, b"changed").unwrap();
        assert!(verify(&before).is_err());
    }

    #[test]
    fn excluded_and_internal_directories_are_never_cleaned() {
        let d = TestDir::new();
        fs::create_dir_all(d.0.join("protected/empty")).unwrap();
        fs::create_dir_all(d.0.join(".downloads/empty")).unwrap();
        d.write("protected/asset.txt", b"keep");
        d.write("visible.txt", b"scan");
        let excluded = HashSet::from([path_key(&d.0.join("protected").to_string_lossy())]);
        assert_eq!(inventory(&d.0, &excluded).unwrap().0.len(), 1);
        assert_eq!(remove_empty_dirs_excluding(&d.0, &excluded).unwrap().0, 0);
        assert!(d.0.join("protected/empty").exists());
        assert!(d.0.join(".downloads/empty").exists());
    }
}
