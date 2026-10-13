//! Incremental evidence. Cleanup must validate the digest stamp under a write-blocking pin.
use super::{
    cleanup_reference_cache::change_stamp,
    resource_files::{self as files, DiskFile},
};
use crate::db::Database;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    collections::{BTreeMap, HashMap, HashSet},
    fs::File,
    io::Read,
    path::Path,
    sync::atomic::{AtomicBool, Ordering},
};

type Result<T> = std::result::Result<T, String>;
const VERSION: i64 = 1;
#[cfg(test)]
thread_local! { pub(super) static CONTENT_HASHED_BYTES: std::cell::Cell<u64> = const { std::cell::Cell::new(0) }; }
const MAX_ENTRIES: usize = 200_000;
const MAX_EXPANDED: u64 = 256 * 1024 * 1024 * 1024;

pub fn check(cancel: &AtomicBool) -> Result<()> {
    if cancel.load(Ordering::Relaxed) {
        Err("DEDUP_CANCELLED".into())
    } else {
        Ok(())
    }
}

#[derive(Clone, Default, Serialize, Deserialize)]
struct Entry {
    size: u64,
    modified: String,
    stamp: Option<String>,
    raw: Option<String>,
    layout: Option<std::result::Result<String, String>>,
    content: Option<String>,
    dependencies: Option<std::result::Result<Vec<String>, String>>,
}

pub struct Cache<'a> {
    db: Option<&'a Database>,
    root: String,
    entries: HashMap<String, Entry>,
    dirty: HashSet<String>,
    pub hits: usize,
    pub bytes: u64,
}
impl<'a> Cache<'a> {
    pub fn load(db: Option<&'a Database>, root: &Path) -> Result<Self> {
        let root = files::path_key(&root.to_string_lossy());
        let entries = if let Some(db) = db {
            db.with_conn(|c| {
                c.execute_batch("CREATE TABLE IF NOT EXISTS dedup_file_cache(root TEXT NOT NULL, path TEXT NOT NULL, version INTEGER NOT NULL, payload TEXT NOT NULL, PRIMARY KEY(root,path))")?;
                let mut stmt = c.prepare("SELECT path,payload FROM dedup_file_cache WHERE root=?1 AND version=?2")?;
                let rows = stmt.query_map(rusqlite::params![root, VERSION], |r| Ok((r.get::<_,String>(0)?,r.get::<_,String>(1)?)))?
                    .collect::<rusqlite::Result<Vec<_>>>()?;
                Ok(rows.into_iter().filter_map(|(p,v)| serde_json::from_str(&v).ok().map(|v|(p,v))).collect())
            }).map_err(|e|e.to_string())?
        } else {
            HashMap::new()
        };
        Ok(Self {
            db,
            root,
            entries,
            dirty: HashSet::new(),
            hits: 0,
            bytes: 0,
        })
    }
    fn entry(&mut self, file: &DiskFile) -> Result<(String, Option<String>)> {
        files::verify(file)?;
        let stamp = change_stamp(Path::new(&file.path))?;
        let key = files::path_key(&file.path);
        let valid = self.entries.get(&key).is_some_and(|e| {
            stamp.is_some()
                && e.stamp == stamp
                && e.size == file.size
                && e.modified == file.modified
        });
        if !valid {
            self.entries.insert(
                key.clone(),
                Entry {
                    size: file.size,
                    modified: file.modified.clone(),
                    stamp: stamp.clone(),
                    ..Entry::default()
                },
            );
        }
        Ok((key, stamp))
    }
    fn completed(&mut self, file: &DiskFile, key: String, before: Option<String>) -> Result<()> {
        files::verify(file)?;
        if change_stamp(Path::new(&file.path))? != before {
            self.entries.remove(&key);
            return Err(format!("文件已变化，请重新分析: {}", file.path));
        }
        self.dirty.insert(key);
        if self.dirty.len() >= 64 {
            self.flush()?;
        }
        Ok(())
    }
    /// Return the stamp captured with this exact digest, never a newly sampled stamp.
    /// A later read may have invalidated the entry or replaced its proof.
    pub fn verification_stamp(&self, file: &DiskFile, raw: Option<&String>, content: Option<&String>) -> Option<String> {
        let entry = self.entries.get(&files::path_key(&file.path))?;
        let matches = match raw {
            Some(hash) => entry.raw.as_ref() == Some(hash),
            None => content.is_some() && entry.content.as_ref() == content,
        };
        (matches && entry.size == file.size && entry.modified == file.modified)
            .then(|| entry.stamp.clone()).flatten()
    }

    pub fn flush(&mut self) -> Result<()> {
        if let Some(db) = self.db {
            let values: Vec<_> = self
                .dirty
                .iter()
                .filter_map(|key| self.entries.get(key).map(|e| (key, e)))
                .map(|(key, e)| {
                    Ok((
                        key.clone(),
                        serde_json::to_string(e).map_err(|e| e.to_string())?,
                    ))
                })
                .collect::<Result<_>>()?;
            db.with_conn(|c| {
                let tx = c.unchecked_transaction()?;
                for (path,payload) in values {
                    tx.execute("INSERT INTO dedup_file_cache(root,path,version,payload) VALUES(?1,?2,?3,?4) ON CONFLICT(root,path) DO UPDATE SET version=excluded.version,payload=excluded.payload", rusqlite::params![self.root,path,VERSION,payload])?;
                }
                tx.commit()?; Ok(())
            }).map_err(|e|e.to_string())?;
        }
        self.dirty.clear();
        Ok(())
    }
    pub fn prune(&mut self, files: &[DiskFile]) -> Result<()> {
        let active: HashSet<_> = files.iter().map(|f| files::path_key(&f.path)).collect();
        if let Some(db) = self.db {
            db.with_conn(|c| {
                let tx = c.unchecked_transaction()?;
                for key in self.entries.keys().filter(|key| !active.contains(*key)) {
                    tx.execute(
                        "DELETE FROM dedup_file_cache WHERE root=?1 AND path=?2",
                        rusqlite::params![self.root, key],
                    )?;
                }
                tx.commit()?;
                Ok(())
            })
            .map_err(|e| e.to_string())?;
        }
        self.entries.retain(|key, _| active.contains(key));
        Ok(())
    }
    pub fn dependencies(
        &mut self,
        file: &DiskFile,
        read: impl FnOnce() -> Result<Vec<String>>,
    ) -> Result<Result<Vec<String>>> {
        let (key, stamp) = self.entry(file)?;
        if let Some(value) = self.entries[&key].dependencies.clone() {
            self.hits += 1;
            return Ok(value);
        }
        let value = read();
        if value.is_ok() {
            self.entries.get_mut(&key).unwrap().dependencies = Some(value.clone());
        }
        self.completed(file, key, stamp)?;
        Ok(value)
    }
    pub fn raw(
        &mut self,
        file: &DiskFile,
        cancel: &AtomicBool,
        progress: &mut impl FnMut(u64),
    ) -> Result<String> {
        check(cancel)?;
        let (key, stamp) = self.entry(file)?;
        if let Some(value) = self.entries[&key].raw.clone() {
            self.hits += 1;
            return Ok(value);
        }
        let mut last = 0;
        let value = files::hash_file_with_progress(file, &mut |n| {
            check(cancel)?;
            self.bytes += n - last;
            last = n;
            progress(n);
            Ok(())
        })?;
        self.entries.get_mut(&key).unwrap().raw = Some(value.clone());
        self.completed(file, key, stamp)?;
        Ok(value)
    }
    pub fn layout(&mut self, file: &DiskFile, cancel: &AtomicBool) -> Result<Result<String>> {
        check(cancel)?;
        let (key, stamp) = self.entry(file)?;
        if let Some(value) = self.entries[&key].layout.clone() {
            self.hits += 1;
            return Ok(value);
        }
        let value = layout_digest(file, cancel);
        check(cancel)?;
        if value.is_ok() {
            self.entries.get_mut(&key).unwrap().layout = Some(value.clone());
        }
        self.completed(file, key, stamp)?;
        Ok(value)
    }
    pub fn content(
        &mut self,
        file: &DiskFile,
        cancel: &AtomicBool,
        progress: &mut impl FnMut(u64),
    ) -> Result<String> {
        check(cancel)?;
        let (key, stamp) = self.entry(file)?;
        if let Some(value) = self.entries[&key].content.clone() {
            self.hits += 1;
            return Ok(value);
        }
        let value = content_digest(file, cancel, &mut |n| {
            self.bytes += n;
            progress(n);
        })?;
        self.entries.get_mut(&key).unwrap().content = Some(value.clone());
        self.completed(file, key, stamp)?;
        Ok(value)
    }
}

fn archive_entries(
    archive: &mut zip::ZipArchive<File>,
    cancel: &AtomicBool,
) -> Result<BTreeMap<String, (usize, u64, u32)>> {
    if archive.len() > MAX_ENTRIES {
        return Err("包内文件过多，已跳过深度比较".into());
    }
    let mut entries = BTreeMap::new();
    let mut normalized_paths = HashSet::new();
    let mut expanded = 0_u64;
    for i in 0..archive.len() {
        check(cancel)?;
        let entry = archive.by_index(i).map_err(|e| e.to_string())?;
        if entry.is_dir() {
            continue;
        }
        // Keep path case. Ambiguous/duplicate paths cannot prove equivalent loading.
        if entry
            .unix_mode()
            .is_some_and(|mode| !matches!(mode & 0o170000, 0 | 0o100000))
        {
            return Err("包内存在特殊文件，已跳过深度比较".into());
        }
        let original_name = entry.name().to_string();
        let name = original_name.replace('\\', "/");
        if !normalized_paths.insert(name.clone()) {
            return Err("包内有重复路径，已跳过深度比较".into());
        }
        if name.starts_with('/')
            || name
                .split('/')
                .any(|p| p.is_empty() || p == ".." || p == ".")
            || name.contains(':')
        {
            return Err("包内路径异常，已跳过深度比较".into());
        }
        expanded = expanded.checked_add(entry.size()).ok_or("包内文件过大")?;
        if expanded > MAX_EXPANDED {
            return Err("包内文件过大，已跳过深度比较".into());
        }
        if entries
            .insert(original_name, (i, entry.size(), entry.crc32()))
            .is_some()
        {
            return Err("包内有重复路径，已跳过深度比较".into());
        }
    }
    if entries.is_empty() {
        return Err("包内没有可比较的文件".into());
    }
    Ok(entries)
}
fn field(hash: &mut Sha256, bytes: &[u8]) {
    hash.update((bytes.len() as u64).to_le_bytes());
    hash.update(bytes);
}
fn layout_digest(file: &DiskFile, cancel: &AtomicBool) -> Result<String> {
    let mut archive = zip::ZipArchive::new(File::open(&file.path).map_err(|e| e.to_string())?)
        .map_err(|e| e.to_string())?;
    let mut hash = Sha256::new();
    for (name, (_, size, crc)) in archive_entries(&mut archive, cancel)? {
        field(&mut hash, name.as_bytes());
        hash.update(size.to_le_bytes());
        hash.update(crc.to_le_bytes());
    }
    Ok(hex::encode(hash.finalize()))
}

/// Order/compression/time independent; every file, including meta.json, participates.
/// Reads to EOF so the ZIP reader also verifies each entry's CRC.
pub fn content_digest(
    file: &DiskFile,
    cancel: &AtomicBool,
    progress: &mut impl FnMut(u64),
) -> Result<String> {
    files::verify(file)?;
    let before = change_stamp(Path::new(&file.path))?;
    let mut archive = zip::ZipArchive::new(File::open(&file.path).map_err(|e| e.to_string())?)
        .map_err(|e| e.to_string())?;
    let entries = archive_entries(&mut archive, cancel)?;
    let mut total = Sha256::new();
    let mut buffer = vec![0; 1024 * 1024];
    for (name, (index, size, _)) in entries {
        check(cancel)?;
        let mut entry = archive.by_index(index).map_err(|e| e.to_string())?;
        let mut hash = Sha256::new();
        let mut read = 0_u64;
        loop {
            check(cancel)?;
            let n = entry.read(&mut buffer).map_err(|e| e.to_string())?;
            if n == 0 {
                break;
            }
            read = read.checked_add(n as u64).ok_or("包内文件过大")?;
            if read > size {
                return Err("包内文件大小不一致".into());
            }
            hash.update(&buffer[..n]);
            #[cfg(test)]
            CONTENT_HASHED_BYTES.with(|bytes| bytes.set(bytes.get() + n as u64));
            progress(n as u64);
        }
        if read != size {
            return Err("包内文件不完整".into());
        }
        field(&mut total, name.as_bytes());
        total.update(size.to_le_bytes());
        total.update(hash.finalize());
    }
    files::verify(file)?;
    if before != change_stamp(Path::new(&file.path))? {
        return Err("文件已变化，请重新分析".into());
    }
    Ok(hex::encode(total.finalize()))
}
