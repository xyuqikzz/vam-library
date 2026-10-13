//! Persistent, per-instance reference results. Disk reads never hold SQLite's mutex.
use super::resource_files::{self as files, DiskFile};
use crate::{db::Database, errors::AppError};
use rusqlite::params;
use serde::{Deserialize, Serialize};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::Path,
};

// Version 3 separates possible references from package dependency evidence.
// Older rows must be rescanned, since their dotted strings have lost context.
const FORMAT_VERSION: i64 = 3;
#[cfg(test)]
thread_local! { pub(super) static FORCE_SIGNATURE_CHECK: std::cell::Cell<bool> = const { std::cell::Cell::new(false) }; }

pub(super) fn init_schema(conn: &rusqlite::Connection) -> Result<(), AppError> {
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS cleanup_reference_cache (
        library_key TEXT NOT NULL, path_key TEXT NOT NULL, version INTEGER NOT NULL,
        payload TEXT NOT NULL, PRIMARY KEY(library_key, path_key));",
    )?;
    Ok(())
}

#[derive(Clone, Serialize, Deserialize)]
pub(super) struct Entry {
    pub file: DiskFile,
    pub change_stamp: Option<String>,
    // Used when the filesystem cannot supply a reliable change stamp.
    pub content_signature: String,
    pub refs: BTreeSet<String>,
    pub package_refs: BTreeSet<String>,
    pub contents: usize,
    #[serde(default)]
    pub diagnostics: Vec<String>,
    #[serde(default)]
    pub invalid_archive: bool,
    /// Why the archive cannot be used at all: `invalid_download` (a saved web
    /// page), `wrong_format` (another archive type renamed to .var) or
    /// `damaged` (the ZIP directory cannot be read). Older entries only carry
    /// `invalid_archive`, which always meant a download error page.
    #[serde(default)]
    pub problem: Option<String>,
    /// `false` when only meta.json was read (quick check). Such entries never
    /// satisfy a full check; a full entry always satisfies a quick one, since
    /// its references are a superset of the declared dependencies.
    #[serde(default = "full_by_default")]
    pub full: bool,
}
fn full_by_default() -> bool {
    true
}
impl Entry {
    /// The reason this archive is unusable, if it is.
    pub fn problem(&self) -> Option<&str> {
        self.problem
            .as_deref()
            .or(self.invalid_archive.then_some("invalid_download"))
    }
}

pub(super) struct ReferenceCache {
    pub entries: BTreeMap<String, Entry>,
    library_key: String,
    pending: BTreeMap<String, Entry>,
}

impl ReferenceCache {
    pub fn load(db: &Database, library_key: &str) -> Result<Self, String> {
        let rows: Vec<(String, String)> = db.with_conn(|conn| {
            let mut stmt = conn.prepare("SELECT path_key,payload FROM cleanup_reference_cache WHERE library_key=?1 AND version=?2")?;
            let rows = stmt.query_map(params![library_key, FORMAT_VERSION], |row| Ok((row.get(0)?, row.get(1)?)))?
                .collect::<rusqlite::Result<Vec<_>>>()?;
            Ok(rows)
        }).map_err(|e| e.to_string())?;
        // An unreadable cache row is a miss, never evidence of no references.
        // Rows are parsed in parallel; a large library has tens of thousands.
        use rayon::prelude::*;
        let entries = rows
            .into_par_iter()
            .filter_map(|(key, json)| {
                let entry: Entry = serde_json::from_str(&json).ok()?;
                (files::path_key(&entry.file.path) == key).then_some((key, entry))
            })
            .collect();
        Ok(Self {
            entries,
            library_key: library_key.into(),
            pending: BTreeMap::new(),
        })
    }

    /// A cached result that is still valid for `file` and usable for a full
    /// check, or, with `quick`, for a meta.json-only check.
    pub fn get(&self, file: &DiskFile, stamp: &Option<String>, quick: bool) -> Option<&Entry> {
        self.entries
            .get(&files::path_key(&file.path))
            .filter(|entry| {
                entry.file == *file
                    && stamp.is_some()
                    && entry.change_stamp == *stamp
                    && (quick || entry.full)
            })
    }

    pub fn remember(&mut self, entry: Entry) {
        let key = files::path_key(&entry.file.path);
        self.entries.insert(key.clone(), entry.clone());
        self.pending.insert(key, entry);
    }

    pub fn flush(&mut self, db: &Database) -> Result<(), String> {
        if self.pending.is_empty() {
            return Ok(());
        }
        let rows = self
            .pending
            .iter()
            .map(|(key, entry)| {
                Ok((
                    key.clone(),
                    serde_json::to_string(entry).map_err(|e| e.to_string())?,
                ))
            })
            .collect::<Result<Vec<_>, String>>()?;
        db.with_conn(|conn| {
            let tx = conn.unchecked_transaction()?;
            {
                let mut stmt = tx.prepare_cached("INSERT INTO cleanup_reference_cache(library_key,path_key,version,payload) VALUES(?1,?2,?3,?4)
                    ON CONFLICT(library_key,path_key) DO UPDATE SET version=excluded.version,payload=excluded.payload")?;
                for (key, payload) in &rows { stmt.execute(params![self.library_key, key, FORMAT_VERSION, payload])?; }
            }
            tx.commit()?;
            Ok(())
        }).map_err(|e| e.to_string())?;
        self.pending.clear();
        Ok(())
    }

    pub fn flush_if_full(&mut self, db: &Database) -> Result<(), String> {
        if self.pending.len() >= 64 {
            self.flush(db)?;
        }
        Ok(())
    }

    pub fn prune(&self, db: &Database, active: &BTreeSet<String>) -> Result<(), String> {
        let stale: Vec<_> = self
            .entries
            .keys()
            .filter(|key| !active.contains(*key))
            .collect();
        if stale.is_empty() {
            return Ok(());
        }
        db.with_conn(|conn| {
            let tx = conn.unchecked_transaction()?;
            {
                let mut stmt = tx.prepare_cached(
                    "DELETE FROM cleanup_reference_cache WHERE library_key=?1 AND path_key=?2",
                )?;
                for key in &stale {
                    stmt.execute(params![self.library_key, key])?;
                }
            }
            tx.commit()?;
            Ok(())
        })
        .map_err(|e| e.to_string())
    }
}

/// Includes changes that are hidden by restoring the file's size and mtime.
/// Unsupported filesystems fall back to ZIP-directory CRCs or a local-file hash.
pub(super) fn change_stamp(path: &Path) -> Result<Option<String>, String> {
    let file = open_metadata_only(path).map_err(|e| e.to_string())?;
    #[cfg(test)]
    if FORCE_SIGNATURE_CHECK.with(|forced| forced.get()) {
        return Ok(None);
    }
    match platform_stamp(&file)? {
        Some(stamp) => Ok(Some(stamp)),
        // A filesystem that refuses metadata queries on an attribute-only
        // handle must not silently turn every cache lookup into a miss.
        None if cfg!(windows) => platform_stamp(&fs::File::open(path).map_err(|e| e.to_string())?),
        None => Ok(None),
    }
}

pub(super) fn pinned_change_stamp(file: &fs::File) -> Result<Option<String>, String> {
    #[cfg(test)]
    if FORCE_SIGNATURE_CHECK.with(|forced| forced.get()) { return Ok(None); }
    platform_stamp(file)
}

/// Opens a handle that can query metadata but not read file data.
/// On Windows, requesting only FILE_READ_ATTRIBUTES avoids on-access antivirus
/// scans of every archive (a data-read open of a .var triggers one) and is
/// cheaper for the filesystem; GetFileInformationByHandleEx needs nothing more.
#[cfg(windows)]
fn open_metadata_only(path: &Path) -> std::io::Result<fs::File> {
    use std::os::windows::fs::OpenOptionsExt;
    const FILE_READ_ATTRIBUTES: u32 = 0x0080;
    fs::OpenOptions::new()
        .access_mode(FILE_READ_ATTRIBUTES)
        .open(path)
}
#[cfg(not(windows))]
fn open_metadata_only(path: &Path) -> std::io::Result<fs::File> {
    fs::File::open(path)
}

#[cfg(unix)]
fn platform_stamp(file: &fs::File) -> Result<Option<String>, String> {
    use std::os::unix::fs::MetadataExt;
    let meta = file.metadata().map_err(|e| e.to_string())?;
    Ok(Some(format!(
        "unix:{}:{}:{}:{}",
        meta.dev(),
        meta.ino(),
        meta.ctime(),
        meta.ctime_nsec()
    )))
}

#[cfg(windows)]
fn platform_stamp(file: &fs::File) -> Result<Option<String>, String> {
    use std::{ffi::c_void, mem::size_of, os::windows::io::AsRawHandle};
    #[repr(C)]
    #[derive(Default)]
    struct BasicInfo {
        created: i64,
        accessed: i64,
        written: i64,
        changed: i64,
        attributes: u32,
    }
    #[repr(C)]
    #[derive(Default)]
    struct FileIdInfo {
        volume: u64,
        id: [u8; 16],
    }
    #[link(name = "kernel32")]
    extern "system" {
        fn GetFileInformationByHandleEx(
            handle: *mut c_void,
            class: i32,
            info: *mut c_void,
            size: u32,
        ) -> i32;
    }
    let mut basic = BasicInfo::default();
    let mut identity = FileIdInfo::default();
    // SAFETY: the handle stays open, both buffers match the documented Win32
    // FILE_BASIC_INFO / FILE_ID_INFO ABI and their exact sizes are passed.
    let valid = unsafe {
        GetFileInformationByHandleEx(
            file.as_raw_handle(),
            0,
            &mut basic as *mut _ as *mut c_void,
            size_of::<BasicInfo>() as u32,
        ) != 0
            && GetFileInformationByHandleEx(
                file.as_raw_handle(),
                18,
                &mut identity as *mut _ as *mut c_void,
                size_of::<FileIdInfo>() as u32,
            ) != 0
    };
    if !valid || basic.changed <= 0 || identity.id == [0; 16] {
        return Ok(None);
    }
    Ok(Some(format!(
        "windows:{}:{}:{}:{}:{}",
        identity.volume,
        hex::encode(identity.id),
        basic.created,
        basic.written,
        basic.changed
    )))
}

#[cfg(not(any(unix, windows)))]
fn platform_stamp(_: &fs::File) -> Result<Option<String>, String> {
    Ok(None)
}
