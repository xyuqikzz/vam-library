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
    use std::{ffi::c_void, mem::{size_of, size_of_val}, os::windows::io::AsRawHandle};
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
        fn DeviceIoControl(
            handle: *mut c_void,
            control: u32,
            input: *const c_void,
            input_size: u32,
            output: *mut c_void,
            output_size: u32,
            returned: *mut u32,
            overlapped: *mut c_void,
        ) -> i32;
    }
    let mut basic = BasicInfo::default();
    let mut identity = FileIdInfo::default();
    let versions = [2_u16, 3_u16]; // READ_FILE_USN_DATA: accepted record versions.
    let mut record = [0_u8; 1024];
    let mut returned = 0_u32;
    // SAFETY: the handle stays open; buffers have the documented Win32 sizes
    // and layouts. The synchronous call receives no OVERLAPPED pointer.
    let valid = unsafe {
        GetFileInformationByHandleEx(
            file.as_raw_handle(), 0, &mut basic as *mut _ as *mut c_void,
            size_of::<BasicInfo>() as u32,
        ) != 0
            && GetFileInformationByHandleEx(
                file.as_raw_handle(), 18, &mut identity as *mut _ as *mut c_void,
                size_of::<FileIdInfo>() as u32,
            ) != 0
            && DeviceIoControl(
                file.as_raw_handle(), 0x0009_00eb, // FSCTL_READ_FILE_USN_DATA
                versions.as_ptr() as *const c_void, size_of_val(&versions) as u32,
                record.as_mut_ptr() as *mut c_void, record.len() as u32,
                &mut returned, std::ptr::null_mut(),
            ) != 0
    };
    if !valid || basic.changed <= 0 || identity.id == [0; 16] {
        return Ok(None);
    }
    let Some(usn) = record.get(..returned as usize).and_then(parse_file_usn) else {
        return Ok(None);
    };
    // Time alone can repeat for rapid writes, even after restoring the mtime.
    // Include the file's journal sequence as well as identity and metadata.
    // Unsupported/disabled journals fall back to content verification.
    // https://learn.microsoft.com/windows/win32/api/winioctl/ni-winioctl-fsctl_read_file_usn_data
    Ok(Some(format!(
        "windows-usn:{}:{}:{}:{}:{}:{}",
        identity.volume, hex::encode(identity.id), basic.created,
        basic.written, basic.changed, usn,
    )))
}

#[cfg(any(windows, test))]
fn parse_file_usn(record: &[u8]) -> Option<i64> {
    let length = u32::from_le_bytes(record.get(..4)?.try_into().ok()?) as usize;
    let record = record.get(..length)?;
    let version = u16::from_le_bytes(record.get(4..6)?.try_into().ok()?);
    let (offset, minimum) = match version {
        2 => (24, 60), // two 64-bit file identifiers before the USN.
        3 => (40, 76), // two 128-bit file identifiers before the USN.
        _ => return None,
    };
    if record.len() < minimum { return None; }
    let usn = i64::from_le_bytes(record.get(offset..offset + 8)?.try_into().ok()?);
    (usn > 0).then_some(usn)
}

#[cfg(not(any(unix, windows)))]
fn platform_stamp(_: &fs::File) -> Result<Option<String>, String> {
    Ok(None)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn record(version: u16, length: usize, offset: usize, usn: i64) -> Vec<u8> {
        let mut bytes = vec![0; length];
        bytes[..4].copy_from_slice(&(length as u32).to_le_bytes());
        bytes[4..6].copy_from_slice(&version.to_le_bytes());
        bytes[offset..offset + 8].copy_from_slice(&usn.to_le_bytes());
        bytes
    }

    #[test]
    fn usn_records_support_both_file_identifier_sizes() {
        assert_eq!(parse_file_usn(&record(2, 64, 24, 123)), Some(123));
        assert_eq!(parse_file_usn(&record(3, 80, 40, 456)), Some(456));
    }

    #[test]
    fn unavailable_or_malformed_usn_records_require_content_verification() {
        assert_eq!(parse_file_usn(&record(2, 64, 24, 0)), None);
        assert_eq!(parse_file_usn(&record(2, 64, 24, -1)), None);
        assert_eq!(parse_file_usn(&record(4, 80, 40, 123)), None);
        assert_eq!(parse_file_usn(&record(2, 32, 24, 123)), None);
        assert_eq!(parse_file_usn(&record(3, 48, 40, 123)), None);
        assert_eq!(parse_file_usn(&record(2, 64, 24, 123)[..63]), None);
        for length in 0..8 { assert_eq!(parse_file_usn(&vec![0; length]), None); }
    }

    #[cfg(windows)]
    #[test]
    fn rapid_writes_with_restored_mtime_never_reuse_an_unchanged_stamp() {
        let dir = super::super::resource_files::tests::TestDir::new();
        let path = dir.write("rapid.var", b"aaaa");
        for _ in 0..200 {
            let before = change_stamp(&path).unwrap();
            let mtime = fs::metadata(&path).unwrap().modified().unwrap();
            fs::write(&path, b"bbbb").unwrap();
            fs::File::options().write(true).open(&path).unwrap()
                .set_times(fs::FileTimes::new().set_modified(mtime)).unwrap();
            let after = change_stamp(&path).unwrap();
            // No journal means no reusable proof; content checks remain mandatory.
            if let (Some(before), Some(after)) = (before, after) {
                assert_ne!(before, after, "a changed file must invalidate the cached proof");
            }
            fs::write(&path, b"aaaa").unwrap();
        }
    }
}
