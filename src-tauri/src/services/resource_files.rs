//! Disk inventory shared by migration and deduplication. Never follows directory links.
use std::collections::{HashMap, HashSet};
use std::fs::{self, File, OpenOptions};
use std::io::{Read, Seek, Write};
use std::path::{Path, PathBuf};
use std::time::SystemTime;

use serde::{Deserialize, Serialize};
#[cfg(not(windows))]
use sha2::{Digest, Sha256};

pub const REFERENCED_DIR: &str = "依赖旧版本";
static MAINTENANCE_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());
pub fn maintenance_lock() -> Result<std::sync::MutexGuard<'static, ()>, String> {
    MAINTENANCE_LOCK.lock().map_err(|e| e.to_string())
}

/// Analysis can be cancelled while another maintenance operation owns the disk.
pub(crate) fn maintenance_lock_cancellable(cancel: &std::sync::atomic::AtomicBool) -> Result<std::sync::MutexGuard<'static, ()>, String> {
    loop {
        super::dedup_cache::check(cancel)?;
        match MAINTENANCE_LOCK.try_lock() {
            Ok(guard) => return Ok(guard),
            Err(std::sync::TryLockError::Poisoned(e)) => return Err(e.to_string()),
            Err(std::sync::TryLockError::WouldBlock) => std::thread::sleep(std::time::Duration::from_millis(25)),
        }
    }
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
    let identity = super::package_identity::from_path(path)?;
    let (family, version) = identity.id.rsplit_once('.')?;
    // Installation and deduplication need an actual numeric version, even
    // though browsing can read packages with nonstandard version text.
    if !family.contains('.') || !version.bytes().all(|c| c.is_ascii_digit()) {
        return None;
    }
    Some(PackageName {
        id: identity.id.into(),
        family: family.into(),
        creator: identity.creator.into(),
        version: version.parse().ok()?,
    })
}

pub fn path_key(path: &str) -> String {
    path.trim_start_matches(r"\\?\")
        .replace('\\', "/")
        .to_lowercase()
}

/// Resolve the same logical and physical records shown in the package list.
/// Canonical paths keep missing files and links outside this instance excluded.
pub fn resolve_indexed_package_file(
    db: &crate::db::Database,
    package_id: &str,
    allowed_roots: &[PathBuf],
) -> Result<Option<PathBuf>, String> {
    let paths = indexed_package_paths(db, &[package_id.to_string()])?;
    Ok(resolve_package_candidates(paths.get(package_id).map(Vec::as_slice).unwrap_or(&[]), allowed_roots))
}

/// Query one page at a time, reusing numbered parameters in both UNION branches.
/// Every candidate list retains logical-record precedence over physical copies.
pub fn indexed_package_paths(
    db: &crate::db::Database,
    package_ids: &[String],
) -> Result<HashMap<String, Vec<String>>, String> {
    let mut seen = HashSet::new();
    let ids: Vec<_> = package_ids.iter().filter(|id| seen.insert(id.as_str())).collect();
    if ids.is_empty() { return Ok(HashMap::new()); }
    db.with_conn_timed("thumbnails.db_wait", |conn| {
        let _timer = crate::services::perf::CommandTimer::new("thumbnails.paths_query");
        let tx = conn.unchecked_transaction()?;
        let mut paths: HashMap<String, Vec<String>> = HashMap::new();
        for chunk in ids.chunks(400) {
            let placeholders = (1..=chunk.len()).map(|i| format!("?{i}")).collect::<Vec<_>>().join(",");
            let mut stmt = tx.prepare(&format!(
                "SELECT id,file_path FROM packages WHERE id IN ({placeholders})
                 UNION ALL
                 SELECT package_id,file_path FROM physical_packages WHERE package_id IN ({placeholders})"
            ))?;
            let rows = stmt.query_map(rusqlite::params_from_iter(chunk.iter().copied()), |r| {
                Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?))
            })?;
            for row in rows {
                let (id, path) = row?;
                paths.entry(id).or_default().push(path);
            }
        }
        Ok(paths)
    }).map_err(|e| e.to_string())
}

pub fn resolve_package_candidates(paths: &[String], allowed_roots: &[PathBuf]) -> Option<PathBuf> {
    paths.iter().filter_map(|path| fs::canonicalize(path).ok()).find(|path| {
        path.is_file() && allowed_roots.iter().any(|root| path.starts_with(root))
    })
}

pub fn open_var_archive(path: &Path) -> Result<zip::ZipArchive<File>, String> {
    let mut file = File::open(path).map_err(|e| format!("打开资源包失败: {e}"))?;
    let mut signature = [0; 6];
    file.read_exact(&mut signature).map_err(|e| format!("无法读取资源包: {e}"))?;
    if signature == [0x37, 0x7a, 0xbc, 0xaf, 0x27, 0x1c] {
        return Err("此文件实际是 7z 压缩包，无法作为 VAR 读取。VAR 必须是 ZIP 格式，仅修改扩展名无效。".into());
    }
    let text_signature = signature.to_ascii_lowercase();
    if text_signature == *b"<!doct" || text_signature.starts_with(b"<html") || text_signature == *b"<?xml " {
        return Err("此文件包含网页或 XML 文本，不是 VAR 压缩包。请检查下载来源并重新获取原始 VAR 文件。".into());
    }
    file.rewind().map_err(|e| format!("无法读取资源包: {e}"))?;
    zip::ZipArchive::new(file).map_err(|e| format!("无法读取资源包: {e}"))
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
    disk_file(path, &m)
}

/// Build a fingerprint from metadata the caller already holds (for example a
/// directory walk). Produces exactly the same value as [`fingerprint`].
pub fn disk_file(path: &Path, m: &fs::Metadata) -> Result<DiskFile, String> {
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
    hash_file_with_progress(file, &mut |_| Ok(()))
}

pub fn hash_file_with_progress(file: &DiskFile, progress: &mut impl FnMut(u64) -> Result<(), String>) -> Result<String, String> {
    verify(file)?;
    progress(0)?;
    let mut input = File::open(&file.path).map_err(|e| e.to_string())?;
    #[cfg(windows)]
    let mut hash = native_sha256::Hasher::new()?;
    #[cfg(not(windows))]
    let mut hash = Sha256::new();
    let mut buffer = vec![0u8; 1024 * 1024];
    let mut current = 0u64;
    loop {
        let n = input.read(&mut buffer).map_err(|e| e.to_string())?;
        if n == 0 { break; }
        #[cfg(windows)]
        hash.update(&buffer[..n])?;
        #[cfg(not(windows))]
        hash.update(&buffer[..n]);
        current += n as u64;
        #[cfg(test)]
        HASHED_BYTES.with(|bytes| bytes.set(bytes.get() + n as u64));
        progress(current)?;
    }
    verify(file)?;
    #[cfg(windows)]
    return hash.finish().map(hex::encode);
    #[cfg(not(windows))]
    Ok(hex::encode(hash.finalize()))
}

/// A successful Windows handle prevents writes until verification and move
/// finish. Other platforms retain checksum verification without this fast path.
pub(crate) fn pin_readonly(path: &Path) -> Option<File> {
    #[cfg(windows)]
    {
        use std::os::windows::fs::OpenOptionsExt;
        // DELETE participates in Windows sharing checks; attributes-only access
        // does not block a writer. No data-read access is needed for this pin.
        fs::OpenOptions::new().access_mode(0x00010000 | 0x0080).share_mode(0x00000001 | 0x00000004).open(path).ok()
    }
    #[cfg(not(windows))]
    { let _ = path; None }
}

#[cfg(windows)]
mod native_sha256 {
    use windows_sys::Win32::Security::Cryptography::*;
    pub(super) struct Hasher { algorithm: BCRYPT_ALG_HANDLE, hash: BCRYPT_HASH_HANDLE }
    fn checked(status: i32) -> Result<(), String> {
        if status >= 0 { Ok(()) } else { Err(format!("无法校验文件内容：Windows SHA-256 状态 {status:#x}")) }
    }
    impl Hasher {
        pub(super) fn new() -> Result<Self, String> {
            let mut hasher = Self { algorithm: std::ptr::null_mut(), hash: std::ptr::null_mut() };
            // CNG owns the hash buffer (supported since Windows 7); handles
            // remain alive through FinishHash and are released on every exit.
            unsafe {
                checked(BCryptOpenAlgorithmProvider(&mut hasher.algorithm, BCRYPT_SHA256_ALGORITHM, std::ptr::null(), 0))?;
                checked(BCryptCreateHash(hasher.algorithm, &mut hasher.hash, std::ptr::null_mut(), 0, std::ptr::null(), 0, 0))?;
            }
            Ok(hasher)
        }
        pub(super) fn update(&mut self, bytes: &[u8]) -> Result<(), String> {
            unsafe { checked(BCryptHashData(self.hash, bytes.as_ptr(), bytes.len() as u32, 0)) }
        }
        pub(super) fn finish(self) -> Result<[u8; 32], String> {
            let mut output = [0u8; 32];
            unsafe { checked(BCryptFinishHash(self.hash, output.as_mut_ptr(), output.len() as u32, 0))?; }
            Ok(output)
        }
    }
    impl Drop for Hasher {
        fn drop(&mut self) {
            unsafe {
                if !self.hash.is_null() { BCryptDestroyHash(self.hash); }
                if !self.algorithm.is_null() { BCryptCloseAlgorithmProvider(self.algorithm, 0); }
            }
        }
    }
}

#[cfg(test)]
thread_local! { pub(crate) static HASHED_BYTES: std::cell::Cell<u64> = const { std::cell::Cell::new(0) }; }

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
    if super::package_identity::from_path(path).is_none() {
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
    let meta_json = pkg.meta.as_ref().map(serde_json::to_string).transpose()?;
    let license = pkg
        .meta
        .as_ref()
        .map(|m| m.license_type.as_str())
        .unwrap_or_default();
    conn.prepare_cached("INSERT INTO packages (id,creator,name,version,file_path,size_bytes,resource_types,meta_json,license_type,scan_time,file_created_time,description,credits,instructions,promotional_link) VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,?15) ON CONFLICT(id) DO UPDATE SET creator=excluded.creator,name=excluded.name,version=excluded.version,file_created_time=excluded.file_created_time,file_path=excluded.file_path,size_bytes=excluded.size_bytes,resource_types=excluded.resource_types,meta_json=excluded.meta_json,license_type=excluded.license_type,description=excluded.description,credits=excluded.credits,instructions=excluded.instructions,promotional_link=excluded.promotional_link,updated_at=datetime('now')")?.execute(
        rusqlite::params![pkg.id,pkg.creator,pkg.name,pkg.version,pkg.file_path,pkg.size_bytes,types,meta_json,license,pkg.scan_time,pkg.created_time,pkg.meta.as_ref().and_then(|m|m.description.as_ref()),pkg.meta.as_ref().and_then(|m|m.credits.as_ref()),pkg.meta.as_ref().and_then(|m|m.instructions.as_ref()),pkg.meta.as_ref().and_then(|m|m.promotional_link.as_ref())])?;
    conn.prepare_cached("INSERT INTO physical_packages (file_path,package_id,size_bytes,modified_time,scan_time) VALUES (?1,?2,?3,?4,?5) ON CONFLICT(file_path) DO UPDATE SET package_id=excluded.package_id,size_bytes=excluded.size_bytes,modified_time=excluded.modified_time,scan_status='ok',last_error=NULL,file_md5=NULL")?.execute(rusqlite::params![pkg.file_path,pkg.id,pkg.size_bytes,pkg.modified_time,pkg.scan_time])?;
    conn.prepare_cached("DELETE FROM contents WHERE package_id=?1")?
        .execute([&pkg.id])?;
    conn.prepare_cached("DELETE FROM dependencies WHERE package_id=?1")?
        .execute([&pkg.id])?;
    for (name, size) in &pkg.contents {
        conn.prepare_cached("INSERT INTO contents (package_id,file_path,resource_type,size_bytes) VALUES (?1,?2,?3,?4)")?.execute(rusqlite::params![pkg.id,name,crate::models::resource::ResourceType::from_path(name).as_str(),size])?;
    }
    if let Some(meta) = &pkg.meta {
        for id in meta.dependencies.keys() {
            let version = id
                .split_once('.')
                .and_then(|(_, name)| name.rsplit_once('.'))
                .map(|(_, v)| v)
                .unwrap_or("latest");
            conn.prepare_cached("INSERT OR IGNORE INTO dependencies (package_id,depends_on_id,required_version) VALUES (?1,?2,?3)")?.execute(rusqlite::params![pkg.id,id,version])?;
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

    #[test]
    fn batched_paths_chunk_deduplicate_and_preserve_safe_fallbacks() {
        let dir = TestDir::new();
        let outside = TestDir::new();
        let main = dir.0.join("main.var");
        let fallback = dir.0.join("备用 资源.var");
        let forbidden = outside.0.join("outside.var");
        for path in [&main, &fallback, &forbidden] { fs::write(path, b"fixture").unwrap(); }
        let db = crate::db::Database::new(&dir.0.join("test.db")).unwrap();
        let mut ids: Vec<_> = (0..405).map(|i| format!("A.Package{i}.1")).collect();
        db.with_conn(|conn| {
            let tx = conn.unchecked_transaction()?;
            for (i, id) in ids.iter().enumerate() {
                let path = if i == 0 { main.clone() } else { dir.0.join(format!("{i}.var")) };
                tx.execute("INSERT INTO packages(id,creator,name,version,file_path,scan_time) VALUES (?1,'A',?1,1,?2,'now')",
                    rusqlite::params![id, path.to_string_lossy()])?;
            }
            for path in [&forbidden, &fallback] {
                tx.execute("INSERT INTO physical_packages(file_path,package_id,scan_time) VALUES (?1,?2,'now')",
                    rusqlite::params![path.to_string_lossy(), ids[0]])?;
            }
            tx.commit()?;
            Ok(())
        }).unwrap();
        ids.push(ids[0].clone());
        ids.push("Missing.Package.1".into());
        let paths = indexed_package_paths(&db, &ids).unwrap();
        assert_eq!(paths.len(), 405);
        let candidates = &paths[&ids[0]];
        assert_eq!(candidates.len(), 3);
        assert_eq!(candidates[0], main.to_string_lossy());
        let roots = [fs::canonicalize(&dir.0).unwrap()];
        assert_eq!(resolve_package_candidates(candidates, &roots), Some(fs::canonicalize(&main).unwrap()));
        fs::remove_file(&main).unwrap();
        assert_eq!(resolve_package_candidates(candidates, &roots), Some(fs::canonicalize(&fallback).unwrap()));
        fs::remove_file(&fallback).unwrap();
        assert!(resolve_package_candidates(candidates, &roots).is_none());
        assert!(indexed_package_paths(&db, &[]).unwrap().is_empty());
    }

    #[test]
    fn shared_indexing_preserves_import_times_tags_and_replaces_metadata() {
        let conn = rusqlite::Connection::open_in_memory().unwrap();
        crate::db::schema::create_tables(&conn).unwrap();
        let mut pkg = crate::models::var_package::VarPackage {
            id: "Author.Package.1".into(), creator: "Author".into(), name: "Package".into(), version: 1,
            file_path: "fixture.var".into(), size_bytes: 123, resource_types: vec!["scene".into()],
            contents: vec![("Saves/scene/old.json".into(), 12)], created_time: "old-created".into(),
            modified_time: "old-modified".into(), scan_time: "first-import".into(),
            meta: Some(serde_json::from_value(serde_json::json!({"description":"old", "dependencies":{"Author.Sub.Asset.12":{},"Other.Asset":{}}})).unwrap()),
        };
        index_parsed_package(&conn, &pkg).unwrap();
        assert_eq!(conn.query_row("SELECT required_version FROM dependencies WHERE depends_on_id='Author.Sub.Asset.12'", [], |row| row.get::<_, String>(0)).unwrap(), "12");
        assert_eq!(
            conn.query_row(
                "SELECT required_version FROM dependencies WHERE depends_on_id='Other.Asset'",
                [],
                |row| row.get::<_, String>(0)
            )
            .unwrap(),
            "latest"
        );
        conn.execute(
            "INSERT INTO package_tags (package_id,tag) VALUES (?1,'keep')",
            [&pkg.id],
        )
        .unwrap();
        conn.execute("UPDATE physical_packages SET file_md5='stale',scan_status='failed',last_error='old error'", []).unwrap();
        pkg.scan_time = "rescan".into();
        pkg.created_time = "new-created".into();
        pkg.contents = vec![("Saves/scene/new.json".into(), 24)];
        pkg.meta = None;
        index_parsed_package(&conn, &pkg).unwrap();
        assert_eq!(
            conn.query_row("SELECT scan_time FROM packages", [], |r| r
                .get::<_, String>(0))
                .unwrap(),
            "first-import"
        );
        assert_eq!(
            conn.query_row("SELECT scan_time FROM physical_packages", [], |r| r
                .get::<_, String>(0))
                .unwrap(),
            "first-import"
        );
        assert_eq!(
            conn.query_row("SELECT file_created_time FROM packages", [], |r| r
                .get::<_, String>(0))
                .unwrap(),
            "new-created"
        );
        assert_eq!(
            conn.query_row("SELECT tag FROM package_tags", [], |r| r
                .get::<_, String>(0))
                .unwrap(),
            "keep"
        );
        assert_eq!(
            conn.query_row("SELECT file_path FROM contents", [], |r| r
                .get::<_, String>(0))
                .unwrap(),
            "Saves/scene/new.json"
        );
        assert_eq!(
            conn.query_row("SELECT count(*) FROM dependencies", [], |r| r
                .get::<_, i64>(0))
                .unwrap(),
            0
        );
        assert!(conn
            .query_row(
                "SELECT meta_json IS NULL AND description IS NULL FROM packages",
                [],
                |r| r.get::<_, bool>(0)
            )
            .unwrap());
        assert!(conn.query_row("SELECT file_md5 IS NULL AND last_error IS NULL AND scan_status='ok' FROM physical_packages", [], |r| r.get::<_, bool>(0)).unwrap());
    }

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
    fn streamed_sha256_matches_standard_digest_and_can_cancel() {
        use sha2::{Digest, Sha256};
        let dir = TestDir::new();
        for length in [0, 1, 63, 64, 65, 1024 * 1024 + 7] {
            let payload: Vec<_> = (0..length).map(|i| (i % 251) as u8).collect();
            let path = dir.write("hash-fixture", &payload);
            let snapshot = fingerprint(&path).unwrap();
            let mut events = vec![];
            let digest = hash_file_with_progress(&snapshot, &mut |n| { events.push(n); Ok(()) }).unwrap();
            assert_eq!(digest, hex::encode(Sha256::digest(&payload)));
            assert_eq!(events.first(), Some(&0));
            assert_eq!(events.last(), Some(&(length as u64)));
        }
        let path = dir.write("cancellation-fixture", &vec![23; 3 * 1024 * 1024]);
        let snapshot = fingerprint(&path).unwrap();
        HASHED_BYTES.with(|bytes| bytes.set(0));
        let result = hash_file_with_progress(&snapshot, &mut |n| {
            if n > 0 { Err("cancelled".into()) } else { Ok(()) }
        });
        assert_eq!(result.unwrap_err(), "cancelled");
        assert_eq!(HASHED_BYTES.with(|bytes| bytes.get()), 1024 * 1024);
        assert_eq!(fs::metadata(&path).unwrap().len(), snapshot.size);
    }
    #[cfg(windows)]
    #[test]
    fn pinned_read_handle_blocks_writes_but_allows_the_verified_move() {
        let dir = TestDir::new();
        let path = dir.write("source", b"verified");
        let pin = pin_readonly(&path).unwrap();
        assert!(fs::OpenOptions::new().write(true).open(&path).is_err());
        let destination = dir.0.join("trash");
        transfer(&path, &destination, "move").unwrap();
        assert_eq!(super::super::cleanup_reference_cache::pinned_change_stamp(&pin).unwrap(), super::super::cleanup_reference_cache::change_stamp(&destination).unwrap());
        assert!(fs::OpenOptions::new().write(true).open(&destination).is_err());
        drop(pin);
        assert!(fs::OpenOptions::new().write(true).open(&destination).is_ok());
    }
    #[test]
    fn numeric_versions_and_dotted_names() {
        let p = package_name(Path::new("Author.Name.with.dots.12.VAR")).unwrap();
        assert_eq!(p.family, "Author.Name.with.dots");
        assert_eq!(p.version, 12);
        assert!(package_name(Path::new("readme.txt")).is_none());
        for filename in ["Author.Name.1_2.var", "Author.Name.latest.var", "Author.Name.var",
            "Author.Name.+1.var", "Author..1.var", "Author.123.var"] {
            assert!(package_name(Path::new(filename)).is_none(), "{filename}");
        }
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
