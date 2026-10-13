//! Preview-bound, instance-scoped package cleanup. Filesystem changes are
//! journaled separately from SQLite: a database transaction cannot undo a move.
use super::{
    cleanup_json_refs::{self as compatible_refs, References},
    cleanup_reference_cache::{change_stamp, Entry as ReferenceEntry, ReferenceCache},
    install_context::InstallContext,
    resource_files as files,
};
use crate::{db::Database, errors::AppError};
use rusqlite::params;
use serde::de::{self, DeserializeSeed};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    collections::{BTreeMap, BTreeSet, HashSet},
    fs,
    io::{Read, Write},
    path::{Path, PathBuf},
    sync::atomic::{AtomicBool, Ordering},
};

type Result<T> = std::result::Result<T, String>;
const JSON_LIMIT: u64 = 64 * 1024 * 1024;
// The initial journal contains every source, destination and hash. Recovery
// checks actual file locations, so moving need not rewrite it for every file.
const JOURNAL_CHUNK: usize = 64;

pub fn init_schema(conn: &rusqlite::Connection) -> std::result::Result<(), AppError> {
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS cleanup_marks (
        library_key TEXT NOT NULL, package_id TEXT NOT NULL,
        pending INTEGER NOT NULL DEFAULT 0, protected INTEGER NOT NULL DEFAULT 0,
        PRIMARY KEY(library_key,package_id));
        CREATE TABLE IF NOT EXISTS cleanup_policy (
        id INTEGER PRIMARY KEY CHECK(id=1), retention_days INTEGER NOT NULL DEFAULT 30,
        auto_purge INTEGER NOT NULL DEFAULT 0);
        INSERT OR IGNORE INTO cleanup_policy(id) VALUES(1);",
    )?;
    let has_background_index = conn
        .prepare("PRAGMA table_info(cleanup_policy)")?
        .query_map([], |r| r.get::<_, String>(1))?
        .collect::<rusqlite::Result<Vec<_>>>()?
        .iter()
        .any(|column| column == "background_index");
    if !has_background_index {
        conn.execute_batch(
            "ALTER TABLE cleanup_policy ADD COLUMN background_index INTEGER NOT NULL DEFAULT 1;",
        )?;
    }
    super::cleanup_reference_cache::init_schema(conn)?;
    Ok(())
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct Context {
    pub library_key: String,
    pub vam_root: String,
    pub roots: Vec<String>,
    #[serde(default)]
    pub managed: bool,
    #[serde(default)]
    pub shared: bool,
}
impl Context {
    pub fn from_install(install: &InstallContext) -> Result<Self> {
        let root = files::canonical_dir(Path::new(&install.vam_root))?;
        let mut roots: Vec<String> = vec![];
        for path in [&install.real_addon_dir, &install.managed_library_dir] {
            if Path::new(path).exists() {
                let path = files::canonical_dir(Path::new(path))?
                    .to_string_lossy()
                    .into_owned();
                if !roots
                    .iter()
                    .any(|p| files::path_key(p) == files::path_key(&path))
                {
                    roots.push(path);
                }
            }
        }
        roots.sort();
        let vam_root = root.to_string_lossy().into_owned();
        Ok(Self {
            library_key: files::path_key(&vam_root),
            vam_root,
            roots,
            managed: install.is_managed,
            shared: false,
        })
    }
    fn owns(&self, path: &Path) -> bool {
        if path
            .components()
            .any(|c| matches!(c, std::path::Component::ParentDir))
        {
            return false;
        }
        let key = files::path_key(&path.to_string_lossy());
        self.roots
            .iter()
            .any(|root| key.starts_with(&format!("{}/", files::path_key(root))))
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct Mark {
    pub package_id: String,
    pub pending: bool,
    pub protected: bool,
}
pub fn marks(db: &Database, ctx: &Context) -> Result<Vec<Mark>> {
    db.with_conn(|c| {
        let mut s = c.prepare("SELECT package_id,pending,protected FROM cleanup_marks WHERE library_key=?1 ORDER BY package_id")?;
        let rows = s.query_map([&ctx.library_key], |r| Ok(Mark { package_id: r.get(0)?, pending: r.get(1)?, protected: r.get(2)? }))?.collect::<rusqlite::Result<Vec<_>>>()?;
        Ok(rows)
    }).map_err(|e| e.to_string())
}
pub fn set_marks(db: &Database, ctx: &Context, ids: &[String], action: &str) -> Result<()> {
    if !matches!(action, "pending" | "protect" | "unmark" | "unprotect") {
        return Err("无效整理操作".into());
    }
    if ids.len() > 100_000 || ids.iter().any(|id| !valid_id(id)) {
        return Err("无效资源包选择".into());
    }
    db.with_conn(|c| {
        let tx = c.unchecked_transaction()?;
        for id in ids {
            let id = id.to_lowercase();
            tx.execute("INSERT OR IGNORE INTO cleanup_marks(library_key,package_id) VALUES(?1,?2)", params![ctx.library_key,id])?;
            let sql = match action {
                "pending" => "UPDATE cleanup_marks SET pending=1 WHERE library_key=?1 AND package_id=?2 AND protected=0",
                "protect" => "UPDATE cleanup_marks SET protected=1,pending=0 WHERE library_key=?1 AND package_id=?2",
                "unmark" => "UPDATE cleanup_marks SET pending=0 WHERE library_key=?1 AND package_id=?2",
                _ => "UPDATE cleanup_marks SET protected=0 WHERE library_key=?1 AND package_id=?2",
            };
            tx.execute(sql, params![ctx.library_key,id])?;
        }
        tx.commit()?;
        Ok(())
    }).map_err(|e| e.to_string())
}
pub fn valid_id(id: &str) -> bool {
    // Cleanup also handles failed scans. Validate a single safe filename stem,
    // not VaM's numeric version: paths still come only from the disk inventory.
    !id.is_empty()
        && id != "."
        && id != ".."
        && !id.ends_with(['.', ' '])
        && !id.chars().any(|c| c.is_control() || "<>:\"/\\|?*".contains(c))
}
fn cleanup_file_id(path: &Path) -> Option<String> {
    if !has_extension(path, "var") {
        return None;
    }
    let id = path.file_stem()?.to_str()?;
    valid_id(id).then(|| id.to_string())
}
fn checked(cancel: &AtomicBool) -> Result<()> {
    if cancel.load(Ordering::Relaxed) {
        Err("操作已取消".into())
    } else {
        Ok(())
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
pub struct Reason {
    pub code: String,
    pub source: String,
}
fn reason(code: &str, source: impl Into<String>) -> Reason {
    Reason {
        code: code.into(),
        source: source.into(),
    }
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PreviewItem {
    pub package_id: String,
    pub file_count: usize,
    pub size_bytes: u64,
    pub content_count: usize,
    pub attached: bool,
    pub reasons: Vec<Reason>,
    /// Set when the package cannot be read at all (see [`archive_problem`]).
    #[serde(default)]
    pub issue: Option<String>,
    /// Filename does not follow VaM's package naming rules; contents may be intact.
    #[serde(default)]
    pub invalid_name: bool,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Preview {
    pub plan_id: String,
    pub items: Vec<PreviewItem>,
    pub blocked: Vec<PreviewItem>,
    pub retained: Vec<PreviewItem>,
    /// Files whose references could not be read this time. Any of them might
    /// use a selected package, so they keep the selection unless overridden.
    pub warnings: Vec<String>,
    /// Dependencies only the selection uses, offered while they are not included.
    #[serde(default)]
    pub suggested: Vec<PreviewItem>,
    #[serde(default)]
    pub suggested_bytes: u64,
    /// Library-wide formatting notes; logged, never shown with a selection.
    #[serde(default, skip_serializing)]
    pub diagnostics: Vec<String>,
    /// Library-wide unusable files; reported through the package health instead.
    #[serde(default, skip_serializing)]
    pub invalid_files: Vec<String>,
    #[serde(default)]
    pub requires_confirmation: bool,
    pub total_bytes: u64,
    pub total_files: usize,
    /// Planned with meta.json declarations only.
    #[serde(default)]
    pub quick: bool,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct StoredFile {
    pub package_id: String,
    pub original: files::DiskFile,
    pub hash: String,
    pub trash_path: String,
    pub status: String,
    /// Captured while the verified file is pinned against writes after moving.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub verified_stamp: Option<String>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Metadata {
    pub tags: Vec<String>,
    pub scan_time: Option<String>,
    pub created_at: Option<String>,
    #[serde(default)]
    pub pending: bool,
    #[serde(default)]
    pub was_indexed: Option<bool>,
    /// A malformed package still needs a recoverable index after an explicit move.
    #[serde(default)]
    pub index_snapshot: Option<crate::models::var_package::VarPackage>,
}
#[derive(Clone, Debug)]
pub struct Plan {
    pub preview: Preview,
    pub ctx: Context,
    pub requested: Vec<String>,
    pub include_dependencies: bool,
    pub revision: String,
    pub files: Vec<StoredFile>,
    pub metadata: BTreeMap<String, Metadata>,
    pub quick: bool,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Batch {
    pub id: String,
    pub context: Context,
    pub created_at: String,
    pub expires_at: String,
    pub status: String,
    pub files: Vec<StoredFile>,
    pub metadata: BTreeMap<String, Metadata>,
    pub errors: Vec<String>,
}
impl Batch {
    /// Recovery snapshots remain in the durable journal; the UI only needs
    /// file/status summaries and must not deserialize thousands of contents.
    pub fn for_display(mut self) -> Self {
        self.metadata.clear();
        self
    }
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Policy {
    pub retention_days: i64,
    pub auto_purge: bool,
    /// Keep the reference index current in the background (default on).
    #[serde(default = "default_true")]
    pub background_index: bool,
}
fn default_true() -> bool {
    true
}
pub fn policy(db: &Database) -> Result<Policy> {
    db.with_conn(|c| {
        Ok(c.query_row(
            "SELECT retention_days,auto_purge,background_index FROM cleanup_policy WHERE id=1",
            [],
            |r| {
                Ok(Policy {
                    retention_days: r.get(0)?,
                    auto_purge: r.get(1)?,
                    background_index: r.get(2)?,
                })
            },
        )?)
    })
    .map_err(|e| e.to_string())
}
pub fn save_policy(db: &Database, days: i64, auto: bool) -> Result<()> {
    let background = policy(db)?.background_index;
    save_policy_with_index(db, days, auto, background)
}
pub fn save_policy_with_index(
    db: &Database,
    days: i64,
    auto: bool,
    background_index: bool,
) -> Result<()> {
    if !(1..=365).contains(&days) {
        return Err("保留天数须在 1–365 之间".into());
    }
    db.with_conn(|c| {
        c.execute(
            "UPDATE cleanup_policy SET retention_days=?1,auto_purge=?2,background_index=?3 WHERE id=1",
            params![days, auto, background_index],
        )?;
        Ok(())
    })
    .map_err(|e| e.to_string())
}

#[derive(Clone, Debug, Default, Serialize)]
struct Package {
    id: String,
    invalid_name: bool,
    files: Vec<files::DiskFile>,
    dependencies: BTreeSet<String>,
    package_refs: BTreeSet<String>,
    content_count: usize,
}
/// A snapshot of the library's packages, references and protections.
/// Opaque outside this module; reusable across previews of one dialog.
pub struct Catalog {
    packages: BTreeMap<String, Package>,
    edges: BTreeMap<String, BTreeSet<String>>,
    /// Reverse of `edges`: package -> packages that depend on it.
    referrers: BTreeMap<String, BTreeSet<String>>,
    protection: BTreeMap<String, Vec<Reason>>,
    warnings: Vec<String>,
    diagnostics: Vec<String>,
    invalid_files: Vec<String>,
    /// Packages whose every copy is unusable (see [`archive_problem`]).
    problems: BTreeMap<String, String>,
    /// Directory/mapping safety cannot be overridden by a removal choice.
    blockers: Vec<Reason>,
    revision: String,
    /// Built from meta.json declarations only (see [`build_catalog`]).
    quick: bool,
}
// Parallel work is disabled by default in unit tests: the test hooks
// (`REFERENCE_READS`, `FORCE_SIGNATURE_CHECK`) are thread-local.
#[cfg(test)]
thread_local! { static PARALLEL: std::cell::Cell<bool> = const { std::cell::Cell::new(false) }; }
fn parallel_enabled() -> bool {
    #[cfg(test)]
    return PARALLEL.with(|p| p.get());
    #[cfg(not(test))]
    true
}
thread_local! {
    static WORKER_POOL: std::cell::RefCell<Option<std::sync::Arc<rayon::ThreadPool>>> =
        const { std::cell::RefCell::new(None) };
}
/// Run `f` with this thread's parallel file work sent to `pool` (for example
/// low-priority background workers). Sequential steps, including database
/// writes, stay on the calling thread, so its priority is unaffected.
pub fn with_worker_pool<R>(pool: std::sync::Arc<rayon::ThreadPool>, f: impl FnOnce() -> R) -> R {
    struct Restore(Option<std::sync::Arc<rayon::ThreadPool>>);
    impl Drop for Restore {
        fn drop(&mut self) {
            let previous = self.0.take();
            WORKER_POOL.with(|p| *p.borrow_mut() = previous);
        }
    }
    let _restore = Restore(WORKER_POOL.with(|p| p.borrow_mut().replace(pool)));
    f()
}
/// Order-preserving map that runs on the rayon pool outside of tests.
fn parallel_map<T: Sync, R: Send>(items: &[T], f: impl Fn(&T) -> R + Sync + Send) -> Vec<R> {
    use rayon::prelude::*;
    if !parallel_enabled() {
        return items.iter().map(f).collect();
    }
    match WORKER_POOL.with(|p| p.borrow().clone()) {
        Some(pool) => pool.install(|| items.par_iter().map(&f).collect()),
        None => items.par_iter().map(f).collect(),
    }
}
/// Files per parallel chunk; progress and cancellation are reported between chunks.
const CHUNK: usize = 256;
fn all_files(_: &Path) -> bool {
    true
}
fn has_extension(path: &Path, extension: &str) -> bool {
    path.extension()
        .is_some_and(|x| x.eq_ignore_ascii_case(extension))
}
/// Walks `root` without following links. Only files accepted by `keep` are
/// fingerprinted; the walker's own metadata is reused (free on Windows, where
/// it comes from the directory enumeration), and descendants of a canonical,
/// link-free root reached without following links need no per-ancestor check.
fn cleanup_inventory(
    root: &Path,
    cancel: &AtomicBool,
    keep: impl Fn(&Path) -> bool,
) -> Result<(Vec<files::DiskFile>, bool)> {
    let root = files::canonical_dir(root)?;
    let mut walker = walkdir::WalkDir::new(root).follow_links(false).into_iter();
    let mut result = vec![];
    let mut skipped_link = false;
    let mut seen = 0_usize;
    while let Some(entry) = walker.next() {
        seen += 1;
        if seen % 1024 == 0 {
            checked(cancel)?;
        }
        let entry = entry.map_err(|e| format!("目录检查不完整: {e}"))?;
        let metadata = entry.metadata().map_err(|e| e.to_string())?;
        let name = entry.file_name().to_string_lossy().to_lowercase();
        let internal = entry.depth() > 0
            && matches!(
                name.as_str(),
                ".trash" | ".downloads" | ".vamboxlibrary-downloads"
            );
        if internal || files::is_link(&metadata) {
            if !internal {
                skipped_link = true;
            }
            if entry.file_type().is_dir() {
                walker.skip_current_dir();
            }
            continue;
        }
        if metadata.is_file() && keep(entry.path()) {
            result.push(files::disk_file(entry.path(), &metadata)?);
        }
    }
    checked(cancel)?;
    result.sort_by(|a, b| a.path.cmp(&b.path));
    Ok((result, skipped_link))
}
fn targets(raw: &str, packages: &BTreeMap<String, Package>) -> Vec<String> {
    let raw = raw.to_lowercase();
    // A file literally named *.latest.var must not shadow the newest numeric version.
    if !raw.ends_with(".latest") && packages.contains_key(&raw) {
        return vec![raw];
    }
    let Some((family, version)) = raw.rsplit_once('.') else {
        return vec![];
    };
    let prefix = format!("{family}.");
    let candidates: Vec<_> = packages
        .range(prefix.clone()..)
        .take_while(|(id, _)| id.starts_with(&prefix))
        .filter_map(|(id, _)| {
            let (f, v) = id.rsplit_once('.')?;
            (f == family)
                .then(|| v.parse::<u64>().ok().map(|v| (v, id.clone())))
                .flatten()
        })
        .collect();
    if version == "latest" {
        candidates
            .into_iter()
            .max_by_key(|(v, _)| *v)
            .map(|(_, id)| vec![id])
            .unwrap_or_default()
    } else {
        // Preserve all fallback candidates when an explicitly requested version
        // is absent; never assume another version is interchangeable for cleanup.
        candidates
            .into_iter()
            .filter(|(v, _)| version.parse::<u64>().map_or(true, |n| *v >= n))
            .map(|(_, id)| id)
            .collect()
    }
}
fn read_metadata(
    reader: impl Read,
    refs: &mut References,
    cancel: &AtomicBool,
) -> Result<Vec<String>> {
    let mut bytes = Vec::new();
    CancellableReader { reader, cancel }
        .take(JSON_LIMIT + 1)
        .read_to_end(&mut bytes)
        .map_err(|e| e.to_string())?;
    if bytes.len() as u64 > JSON_LIMIT {
        return Err("引用文件过大，无法完整检查".into());
    }
    let body = bytes.strip_prefix(&[0xef, 0xbb, 0xbf]).unwrap_or(&bytes);
    let strict = serde_json::from_slice(body)
        .map_err(|e| e.to_string())
        .and_then(|value| {
            validate_meta(&value)?;
            collect_refs(&value, refs);
            Ok(())
        });
    match strict {
        Ok(()) => Ok(vec![]),
        Err(issue) => {
            compatible_refs::scan(body, refs, cancel, true)?;
            // Malformed metadata may encode dependencies in unexpected shapes.
            // Retain all candidates here rather than infer independence.
            refs.packages.extend(refs.candidates.iter().cloned());
            Ok(vec![format!(
                "meta.json 非标准格式，已完整检查引用：{issue}"
            )])
        }
    }
}
fn collect_refs(value: &serde_json::Value, refs: &mut References) {
    match value {
        serde_json::Value::String(s) => {
            compatible_refs::collect_value(s, refs);
        }
        serde_json::Value::Array(a) => {
            for v in a {
                collect_refs(v, refs);
            }
        }
        serde_json::Value::Object(o) => {
            for (key, v) in o {
                if key == "dependencies" {
                    if let Some(deps) = v.as_object() {
                        for key in deps.keys() {
                            compatible_refs::collect_dependency(key, refs);
                        }
                    }
                }
                collect_refs(v, refs);
            }
        }
        _ => {}
    }
}
/// `Creator.Package.Version:/path` (or `.latest`) inside a JSON string.
fn insert_reference(value: &str, refs: &mut References) {
    compatible_refs::collect_value(value, refs);
}
struct ReferenceSeed<'a> {
    refs: &'a mut References,
    dependency_keys: bool,
}
impl<'de> DeserializeSeed<'de> for ReferenceSeed<'_> {
    type Value = ();
    fn deserialize<D: de::Deserializer<'de>>(
        self,
        deserializer: D,
    ) -> std::result::Result<(), D::Error> {
        deserializer.deserialize_any(self)
    }
}
impl<'de> de::Visitor<'de> for ReferenceSeed<'_> {
    type Value = ();
    fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        f.write_str("valid JSON content")
    }
    fn visit_str<E: de::Error>(self, value: &str) -> std::result::Result<(), E> {
        insert_reference(value, self.refs);
        Ok(())
    }
    fn visit_map<A: de::MapAccess<'de>>(self, mut map: A) -> std::result::Result<(), A::Error> {
        while let Some(key) = map.next_key::<String>()? {
            if self.dependency_keys {
                compatible_refs::collect_dependency(&key, self.refs);
            }
            map.next_value_seed(ReferenceSeed {
                refs: &mut *self.refs,
                dependency_keys: key == "dependencies",
            })?;
        }
        Ok(())
    }
    fn visit_seq<A: de::SeqAccess<'de>>(self, mut seq: A) -> std::result::Result<(), A::Error> {
        while seq
            .next_element_seed(ReferenceSeed {
                refs: &mut *self.refs,
                dependency_keys: false,
            })?
            .is_some()
        {}
        Ok(())
    }
    fn visit_bool<E: de::Error>(self, _: bool) -> std::result::Result<(), E> {
        Ok(())
    }
    fn visit_i64<E: de::Error>(self, _: i64) -> std::result::Result<(), E> {
        Ok(())
    }
    fn visit_u64<E: de::Error>(self, _: u64) -> std::result::Result<(), E> {
        Ok(())
    }
    fn visit_f64<E: de::Error>(self, _: f64) -> std::result::Result<(), E> {
        Ok(())
    }
    fn visit_unit<E: de::Error>(self) -> std::result::Result<(), E> {
        Ok(())
    }
}
struct CancellableReader<'a, R> {
    reader: R,
    cancel: &'a AtomicBool,
}
impl<R: Read> Read for CancellableReader<'_, R> {
    fn read(&mut self, buffer: &mut [u8]) -> std::io::Result<usize> {
        checked(self.cancel).map_err(std::io::Error::other)?;
        self.reader.read(buffer)
    }
}
#[cfg(test)]
fn stream_references(
    reader: impl Read,
    refs: &mut References,
    cancel: &AtomicBool,
) -> Result<()> {
    let mut reader =
        std::io::BufReader::with_capacity(64 * 1024, CancellableReader { reader, cancel });
    let mut prefix = [0_u8; 3];
    let mut length = 0;
    while length < prefix.len() {
        let n = reader
            .read(&mut prefix[length..])
            .map_err(|e| e.to_string())?;
        if n == 0 {
            break;
        }
        length += n;
    }
    let bytes = if prefix == [0xef, 0xbb, 0xbf] {
        vec![]
    } else {
        prefix[..length].to_vec()
    };
    let mut deserializer =
        serde_json::Deserializer::from_reader(std::io::Cursor::new(bytes).chain(reader));
    ReferenceSeed {
        refs,
        dependency_keys: false,
    }
    .deserialize(&mut deserializer)
    .map_err(|e| e.to_string())?;
    deserializer.end().map_err(|e| e.to_string())
}
/// Content files up to this size are scanned in memory; larger ones stream.
const SCAN_IN_MEMORY: u64 = 256 * 1024 * 1024;
/// Collect package references from a scene/preset JSON.
///
/// Standard files use a fast string scan. Non-standard syntax falls back to a
/// conservative scanner matching VaM's token rules. Unreadable strings remain
/// blocking errors. Archive entries may have unclosed containers after complete
/// CRC-verified reads; local files require closed containers to avoid accepting
/// an interrupted write. The returned flag identifies recovered format issues.
fn scan_references(
    reader: impl Read,
    size: u64,
    refs: &mut References,
    cancel: &AtomicBool,
    allow_unclosed_container: bool,
) -> Result<bool> {
    if size > SCAN_IN_MEMORY {
        compatible_refs::scan(reader, refs, cancel, allow_unclosed_container)?;
        return Ok(false);
    }
    let mut bytes = Vec::with_capacity(size as usize);
    CancellableReader { reader, cancel }
        .take(SCAN_IN_MEMORY + 1)
        .read_to_end(&mut bytes)
        .map_err(|e| e.to_string())?;
    if bytes.len() as u64 > SCAN_IN_MEMORY {
        return Err("引用文件过大，无法完整检查".into());
    }
    let body = bytes.strip_prefix(&[0xef, 0xbb, 0xbf]).unwrap_or(&bytes);
    let strict = if memchr::memmem::find(body, b"\"dependencies\"").is_some() {
        let mut deserializer = serde_json::Deserializer::from_slice(body);
        ReferenceSeed {
            refs,
            dependency_keys: false,
        }
        .deserialize(&mut deserializer)
        .map_err(|e| e.to_string())
        .and_then(|_| deserializer.end().map_err(|e| e.to_string()))
    } else {
        scan_json_strings(body, refs)
    };
    match strict {
        Ok(()) => Ok(false),
        Err(_) => {
            compatible_refs::scan(body, refs, cancel, allow_unclosed_container)?;
            Ok(true)
        }
    }
}
fn scan_json_strings(body: &[u8], refs: &mut References) -> Result<()> {
    // Single pass; VaM scenes are dominated by short strings, where a plain
    // byte loop beats per-string memchr calls.
    let n = body.len();
    let mut depth = 0_usize;
    let mut stack: Vec<u8> = Vec::with_capacity(32);
    let mut root_done = false;
    let mut i = 0;
    while i < n {
        let byte = body[i];
        i += 1;
        match byte {
            b'"' => {
                if depth == 0 {
                    if root_done {
                        return Err("JSON 结尾有多余内容".into());
                    }
                    root_done = true;
                }
                let start = i;
                let mut escaped = false;
                loop {
                    let Some(&c) = body.get(i) else {
                        return Err("JSON 字符串未结束".into());
                    };
                    match c {
                        b'"' => break,
                        b'\\' => {
                            escaped = true;
                            i += 2;
                        }
                        0..=31 => return Err("JSON 字符串包含原始控制字符".into()),
                        _ => i += 1,
                    }
                }
                let end = i;
                i += 1;
                if escaped {
                    // Escapes may hide any character, including the ':' marker.
                    let value: String = serde_json::from_slice(&body[start - 1..=end])
                        .map_err(|e| e.to_string())?;
                    insert_reference(&value, refs);
                } else {
                    let value = std::str::from_utf8(&body[start..end])
                        .map_err(|_| "JSON 包含无效 UTF-8".to_string())?;
                    insert_reference(value, refs);
                }
                // Missing separators may merge literals in the game's parser.
                // Send those files through the scanner that also retains merged tokens.
                let next = body[i..].iter().copied().find(|b| !b.is_ascii_whitespace());
                if next.is_some_and(|b| !matches!(b, b':' | b',' | b'}' | b']')) {
                    return Err("JSON 字符串后缺少分隔符".into());
                }
            }
            b'{' | b'[' => {
                if depth == 0 && root_done {
                    return Err("JSON 结尾有多余内容".into());
                }
                stack.push(byte);
                depth += 1;
            }
            b'}' | b']' => {
                let open = if byte == b'}' { b'{' } else { b'[' };
                if stack.pop() != Some(open) {
                    return Err("JSON 括号不匹配".into());
                }
                depth -= 1;
                if depth == 0 {
                    root_done = true;
                }
            }
            b' ' | b'\t' | b'\n' | b'\r' => {}
            _ if depth == 0 => {
                if root_done {
                    return Err("JSON 结尾有多余内容".into());
                }
                // A bare top-level scalar (number, true, false, null).
                while i < n
                    && !matches!(
                        body[i],
                        b' ' | b'\t' | b'\n' | b'\r' | b'"' | b'{' | b'[' | b'}' | b']'
                    )
                {
                    i += 1;
                }
                root_done = true;
            }
            b':' | b',' => {}
            _ => {
                let start = i - 1;
                while i < n
                    && !matches!(
                        body[i],
                        b' ' | b'\t'
                            | b'\n'
                            | b'\r'
                            | b','
                            | b'}'
                            | b']'
                            | b'"'
                            | b':'
                            | b'{'
                            | b'['
                    )
                {
                    i += 1;
                }
                // Inspect bare tokens through the compatible scanner instead of
                // assuming they can never contain a reference.
                let token = &body[start..i];
                if !matches!(token, b"true" | b"false" | b"null")
                    && serde_json::from_slice::<serde_json::Number>(token).is_err()
                {
                    return Err("JSON 包含非标准裸值".into());
                }
                let next = body[i..].iter().copied().find(|b| !b.is_ascii_whitespace());
                if next.is_some_and(|b| !matches!(b, b',' | b'}' | b']')) {
                    return Err("JSON 裸值后缺少分隔符".into());
                }
            }
        }
    }
    if depth != 0 {
        return Err("JSON 意外结束".into());
    }
    if !root_done {
        return Err("JSON 内容为空".into());
    }
    Ok(())
}
fn validate_meta(value: &serde_json::Value) -> Result<()> {
    let object = value.as_object().ok_or("meta.json 须是 JSON 对象")?;
    if let Some(deps) = object.get("dependencies") {
        let deps = deps
            .as_object()
            .ok_or("meta.json 的 dependencies 格式无效")?;
        for (id, nested) in deps {
            let Some(normalized) = compatible_refs::canonical_id(id) else {
                return Err("meta.json 包含无法识别的依赖标识".into());
            };
            let (family, version) = normalized.rsplit_once('.').unwrap();
            if !family.contains('.')
                || family.contains(['/', '\\', ':'])
                || (!version.eq_ignore_ascii_case("latest") && version.parse::<u64>().is_err())
            {
                return Err(format!("meta.json 包含无法识别的依赖: {id}"));
            }
            if nested
                .as_object()
                .is_some_and(|o| o.contains_key("dependencies"))
            {
                validate_meta(nested)?;
            }
        }
    }
    Ok(())
}
fn is_json(path: &str) -> bool {
    path.rsplit_once('.')
        .is_some_and(|(_, ext)| matches!(ext.to_lowercase().as_str(), "json" | "vap" | "vaj"))
}
fn protect(map: &mut BTreeMap<String, Vec<Reason>>, id: String, r: Reason) {
    map.entry(id).or_default().push(r);
}
/// Hashes the JSON encoding of `value` without materializing it in memory.
fn digest(value: &impl Serialize) -> Result<String> {
    struct HashWriter(Sha256);
    impl Write for HashWriter {
        fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
            self.0.update(bytes);
            Ok(bytes.len())
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }
    let mut writer = HashWriter(Sha256::new());
    serde_json::to_writer(&mut writer, value).map_err(|e| e.to_string())?;
    Ok(hex::encode(writer.0.finalize()))
}
#[cfg(test)]
thread_local! { static REFERENCE_READS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) }; }
/// References of one package. With `quick`, only meta.json is read: the
/// result lists declared dependencies and is marked as not full.
// A failed HTTP download can leave an HTML/XML response named *.var. Such a
// file cannot act as a VaM package, so it is reported separately from an archive
// whose references are unknown. Never classify a damaged ZIP as an error page.
fn is_download_response(file: &files::DiskFile, cancel: &AtomicBool) -> Result<bool> {
    if file.size > 1024 * 1024 {
        return Ok(false);
    }
    let mut bytes = Vec::new();
    CancellableReader {
        reader: fs::File::open(&file.path).map_err(|e| e.to_string())?,
        cancel,
    }
    .read_to_end(&mut bytes)
    .map_err(|e| e.to_string())?;
    let Ok(text) = std::str::from_utf8(&bytes) else {
        return Ok(false);
    };
    let text = text.trim_start_matches('\u{feff}').trim_start().to_ascii_lowercase();
    Ok(text.starts_with("<!doctype html") || text.starts_with("<html")
        || (text.starts_with("<?xml") && text.contains("<error")))
}
/// Classify an archive that could not be opened. Only structural failures are
/// reported: neither VaM nor this app can read such a file, so it can neither
/// use other resources nor be used by them. Anything else (a locked file, an
/// unsupported ZIP feature VaM might still read) stays an unknown consumer.
fn archive_problem(file: &files::DiskFile, cancel: &AtomicBool) -> Result<Option<&'static str>> {
    if is_download_response(file, cancel)? {
        return Ok(Some("invalid_download"));
    }
    let Ok(mut handle) = fs::File::open(&file.path) else {
        return Ok(None);
    };
    let mut signature = [0_u8; 6];
    let mut read = 0;
    while read < signature.len() {
        match handle.read(&mut signature[read..]) {
            Ok(0) => break,
            Ok(n) => read += n,
            Err(_) => return Ok(None),
        }
    }
    if read < 4 {
        return Ok(Some("damaged"));
    }
    if signature == [0x37, 0x7a, 0xbc, 0xaf, 0x27, 0x1c] || signature.starts_with(b"Rar!") {
        return Ok(Some("wrong_format"));
    }
    let Ok(handle) = fs::File::open(&file.path) else {
        return Ok(None);
    };
    // Only a missing or truncated central directory (an unfinished download,
    // or not a ZIP at all). Other complaints, such as unusual entry names or
    // dates, may still be accepted by the game.
    Ok(match zip::ZipArchive::new(handle) {
        Err(zip::result::ZipError::InvalidArchive(reason))
            if matches!(
                reason,
                "Could not find EOCD"
                    | "Could not find EOCD64"
                    | "Invalid central directory size or offset"
                    | "Invalid CDFH offset in EOCD"
                    | "No CDFH found"
                    | "Invalid EOCD comment length"
            ) =>
        {
            Some("damaged")
        }
        Err(zip::result::ZipError::Io(e)) if e.kind() == std::io::ErrorKind::UnexpectedEof => {
            Some("damaged")
        }
        _ => None,
    })
}
fn archive_references(
    file: &files::DiskFile,
    stamp: &Option<String>,
    cached: Option<&ReferenceEntry>,
    quick: bool,
    cancel: &AtomicBool,
) -> Result<ReferenceEntry> {
    #[cfg(test)]
    REFERENCE_READS.with(|reads| reads.set(reads.get() + 1));
    let mut zip = match files::open_var_archive(Path::new(&file.path)) {
        Ok(zip) => zip,
        Err(error) => {
            let Some(problem) = archive_problem(file, cancel)? else {
                return Err(error);
            };
            // Only used when the filesystem has no change stamps; error pages are small.
            let signature = if problem == "invalid_download" {
                files::hash_file(file)?
            } else {
                String::new()
            };
            verify_reference_source(file, stamp)?;
            return Ok(ReferenceEntry {
                file: file.clone(), change_stamp: stamp.clone(), content_signature: signature,
                refs: BTreeSet::new(), package_refs: BTreeSet::new(), contents: 0, full: true,
                diagnostics: vec![], invalid_archive: true, problem: Some(problem.into()),
            });
        }
    };
    let mut directory = Vec::with_capacity(zip.len());
    for i in 0..zip.len() {
        checked(cancel)?;
        let entry = zip.by_index(i).map_err(|e| e.to_string())?;
        directory.push((entry.name().to_string(), entry.size(), entry.crc32()));
    }
    let signature = digest(&directory)?;
    if let Some(cached) = cached.filter(|entry| {
        entry.file == *file && entry.content_signature == signature && (quick || entry.full)
    }) {
        verify_reference_source(file, stamp)?;
        return Ok(ReferenceEntry {
            change_stamp: stamp.clone(),
            ..cached.clone()
        });
    }
    let mut refs = References::default();
    let mut diagnostics = Vec::new();
    let mut contents = 0;
    let mut has_meta = false;
    for i in 0..zip.len() {
        checked(cancel)?;
        let entry = zip.by_index(i).map_err(|e| e.to_string())?;
        if entry.is_dir() {
            continue;
        }
        let path = entry.name().to_string();
        if is_json(&path) {
            if path.eq_ignore_ascii_case("meta.json") {
                has_meta = true;
            } else {
                contents += 1;
            }
            if path.eq_ignore_ascii_case("meta.json") {
                diagnostics.extend(
                    read_metadata(entry, &mut refs, cancel).map_err(|e| format!("{path}: {e}"))?,
                );
            } else if !quick {
                let size = entry.size();
                // Archive entries are read to EOF with ZIP CRC verification.
                // VaM accepts unclosed containers if their strings are complete;
                // retain every token in those entries as well.
                if scan_references(entry, size, &mut refs, cancel, true)
                    .map_err(|e| format!("{path}: {e}"))?
                {
                    diagnostics.push(format!("{path} 非标准格式，已完整检查引用"));
                }
            }
        }
    }
    if !has_meta {
        if quick {
            return Err("缺少 meta.json，无法按声明检查依赖".into());
        }
        diagnostics.push("缺少 meta.json，已完整检查包内引用文件".into());
    }
    verify_reference_source(file, stamp)?;
    Ok(ReferenceEntry {
        file: file.clone(),
        change_stamp: stamp.clone(),
        content_signature: signature,
        refs: refs.candidates,
        package_refs: refs.packages,
        contents,
        full: !quick,
        diagnostics,
        invalid_archive: false,
        problem: None,
    })
}
fn verify_reference_source(file: &files::DiskFile, stamp: &Option<String>) -> Result<()> {
    files::verify(file)?;
    if change_stamp(Path::new(&file.path))? != *stamp {
        return Err("文件在引用检查过程中发生变化，请重新预览".into());
    }
    Ok(())
}
fn local_references(
    file: &files::DiskFile,
    stamp: &Option<String>,
    cached: Option<&ReferenceEntry>,
    cancel: &AtomicBool,
) -> Result<ReferenceEntry> {
    #[cfg(test)]
    REFERENCE_READS.with(|reads| reads.set(reads.get() + 1));
    let signature = if stamp.is_none() {
        files::hash_file(file)?
    } else {
        String::new()
    };
    if stamp.is_none() {
        if let Some(cached) =
            cached.filter(|entry| entry.file == *file && entry.content_signature == signature)
        {
            verify_reference_source(file, stamp)?;
            return Ok(cached.clone());
        }
    }
    let mut refs = References::default();
    let recovered = scan_references(
        fs::File::open(&file.path).map_err(|e| e.to_string())?,
        file.size,
        &mut refs,
        cancel,
        false,
    )?;
    verify_reference_source(file, stamp)?;
    Ok(ReferenceEntry {
        file: file.clone(),
        change_stamp: stamp.clone(),
        content_signature: signature,
        refs: refs.candidates,
        package_refs: refs.packages,
        contents: 0,
        full: true,
        invalid_archive: false,
        problem: None,
        diagnostics: if recovered {
            vec!["非标准格式，已完整检查引用".into()]
        } else {
            vec![]
        },
    })
}
fn catalog(
    db: &Database,
    ctx: &Context,
    quick: bool,
    cancel: &AtomicBool,
    progress: &mut impl FnMut(&str, usize, usize),
) -> Result<Catalog> {
    progress("loading_index", 0, 0);
    let mut cache = ReferenceCache::load(db, &ctx.library_key)?;
    let mut inventory = BTreeMap::new();
    let mut warnings = vec![];
    let mut blockers = vec![];
    if ctx.managed {
        blockers.push(reason("managed", ""));
        warnings.push("按需托管库可能存在启用映射，请先恢复为普通资源目录后执行清理".into());
    }
    if ctx.shared {
        blockers.push(reason("shared", ""));
        warnings.push(
            "资源目录被其他已配置实例共用，无法确认其本地引用；请先拆分共享目录后清理".into(),
        );
    }
    for root in &ctx.roots {
        let (list, skipped) =
            cleanup_inventory(Path::new(root), cancel, |p| has_extension(p, "var"))?;
        // A skipped link could contain an unobserved consumer. Managed instances
        // are handled conservatively until their mapping has been restored.
        if skipped {
            blockers.push(reason("unsafeDirectory", ""));
            warnings.push("资源目录包含链接或不可检查目录，请恢复为真实目录后清理".into());
        }
        for file in list {
            inventory.insert(files::path_key(&file.path), file);
        }
    }
    let mut stamps: Vec<files::DiskFile> = inventory.values().cloned().collect();
    let mut local_files = BTreeMap::new();
    let mut favorites = vec![];
    for dir in ["Custom", "Saves", "AddonPackagesFilePrefs"] {
        let root = Path::new(&ctx.vam_root).join(dir);
        if !root.exists() {
            continue;
        }
        // Only reference sources are fingerprinted; textures, audio and other
        // payload files are skipped without any per-file system call.
        let (list, skipped) = if dir == "AddonPackagesFilePrefs" {
            cleanup_inventory(&root, cancel, |p| has_extension(p, "fav"))?
        } else {
            cleanup_inventory(&root, cancel, |p| is_json(&p.to_string_lossy()))?
        };
        if skipped {
            warnings.push(format!("{dir} 中有无法完整检查的链接或目录"));
        }
        for file in list {
            let relative = Path::new(&file.path)
                .strip_prefix(&ctx.vam_root)
                .map_err(|e| e.to_string())?
                .to_string_lossy()
                .replace('\\', "/");
            if dir == "AddonPackagesFilePrefs" && relative.ends_with(".fav") {
                favorites.push(relative);
                stamps.push(file);
            } else if dir != "AddonPackagesFilePrefs" && is_json(&file.path) {
                local_files.insert(files::path_key(&file.path), relative);
                stamps.push(file.clone());
                inventory.insert(files::path_key(&file.path), file);
            }
        }
    }
    // Change stamps are metadata-only reads; run them in parallel and report
    // progress, since a large library has tens of thousands of them.
    let chunk_size = if parallel_enabled() { CHUNK } else { 1 };
    let sources: Vec<&files::DiskFile> = inventory.values().collect();
    let mut jobs = Vec::with_capacity(sources.len());
    for chunk in sources.chunks(CHUNK) {
        checked(cancel)?;
        let stamped = parallel_map(chunk, |file| change_stamp(Path::new(&file.path)));
        for (file, stamp) in chunk.iter().zip(stamped) {
            jobs.push((*file, stamp?));
        }
        progress("checking_files", jobs.len(), sources.len());
    }
    let total = jobs
        .iter()
        .filter(|(file, stamp)| cache.get(file, stamp, quick).is_none())
        .count();
    let stage = if cache.entries.is_empty() {
        "building_index"
    } else {
        "updating_index"
    };
    let mut current = 0;
    if total > 0 {
        progress(stage, 0, total);
    }
    let mut packages = BTreeMap::<String, Package>::new();
    let mut local_refs = BTreeMap::<String, BTreeSet<String>>::new();
    let mut unidentified_refs = BTreeMap::<String, BTreeSet<String>>::new();
    let mut invalid_files = vec![];
    // package key -> (unusable copies, reason of the first one)
    let mut problem_files = BTreeMap::<String, (usize, String)>::new();
    let mut reference_stamps = vec![];
    let mut diagnostics = vec![];
    for chunk in jobs.chunks(chunk_size) {
        if let Err(e) = checked(cancel) {
            cache.flush(db)?;
            return Err(e);
        }
        // Uncached archives and local files are read concurrently; the results
        // are then applied in the original order, so warnings, cache writes and
        // progress stay deterministic.
        let mut computed = parallel_map(chunk, |(file, stamp)| {
            if cache.get(file, stamp, quick).is_some() {
                return None;
            }
            let key = files::path_key(&file.path);
            let cached = cache.entries.get(&key);
            if local_files.contains_key(&key) {
                Some(local_references(file, stamp, cached, cancel))
            } else {
                // Renamed packages can still contain references to installed resources.
                Some(archive_references(file, stamp, cached, quick, cancel))
            }
        })
        .into_iter();
        if let Err(e) = checked(cancel) {
            cache.flush(db)?;
            return Err(e);
        }
        for (file, stamp) in chunk {
            let (file, stamp) = (*file, stamp.clone());
            let fresh = computed.next().flatten();
            let key = files::path_key(&file.path);
            let local = local_files.get(&key);
            let package_id = if local.is_none() {
                cleanup_file_id(Path::new(&file.path)).map(|id| {
                    let package = packages.entry(id.to_lowercase()).or_insert_with(|| Package {
                        id: id.clone(),
                        invalid_name: files::package_name(Path::new(&file.path)).is_none(),
                        ..Package::default()
                    });
                    package.files.push(file.clone());
                    id
                })
            } else {
                None
            };
            let entry = match fresh {
                None => match cache.get(file, &stamp, quick) {
                    Some(cached) => Ok(cached.clone()),
                    None => Err("引用检查结果缺失，请重新预览".to_string()),
                },
                Some(result) => {
                    if let Ok(entry) = &result {
                        cache.remember(entry.clone());
                        cache.flush_if_full(db)?;
                    }
                    current += 1;
                    progress(stage, current, total);
                    result
                }
            };
            let source = Path::new(&file.path).strip_prefix(&ctx.vam_root)
                .unwrap_or(Path::new(&file.path)).to_string_lossy().replace('\\', "/");
            match entry {
                Ok(entry) => {
                    if let Some(problem) = entry.problem() {
                        invalid_files.push(source.clone());
                        if let Some(package_id) = &package_id {
                            let slot = problem_files
                                .entry(package_id.to_lowercase())
                                .or_insert((0, problem.to_string()));
                            slot.0 += 1;
                        }
                    }
                    diagnostics.extend(
                        entry
                            .diagnostics
                            .iter()
                            .map(|issue| format!("{source}: {issue}")),
                    );
                    reference_stamps.push((key, stamp, entry.content_signature));
                    if let Some(local) = local {
                        local_refs.insert(local.clone(), entry.refs);
                    } else if let Some(package_id) = package_id {
                        let package = packages.get_mut(&package_id.to_lowercase()).unwrap();
                        package.dependencies.extend(entry.refs);
                        package.package_refs.extend(entry.package_refs);
                        package.content_count = package.content_count.max(entry.contents);
                    } else {
                        unidentified_refs.insert(source, entry.refs);
                    }
                }
                Err(e) => warnings.push(format!("{source}: {e}")),
            }
            if let Err(e) = checked(cancel) {
                cache.flush(db)?;
                return Err(e);
            }
        }
    }
    cache.flush(db)?;
    cache.prune(db, &inventory.keys().cloned().collect())?;
    progress("analyzing", 0, 0);
    let mut protection = BTreeMap::new();
    for relative in favorites {
        if let Some(id) = relative.split('/').nth(1) {
            for id in targets(id, &packages) {
                protect(&mut protection, id, reason("favorite", relative.clone()));
            }
        }
    }
    let marks = marks(db, ctx)?;
    for mark in &marks {
        if mark.protected {
            let key = mark.package_id.to_lowercase();
            let marked = if packages.contains_key(&key) { vec![key] } else { targets(&key, &packages) };
            for id in marked {
                protect(
                    &mut protection,
                    id,
                    reason("manual", mark.package_id.clone()),
                );
            }
        }
    }
    let plans: Vec<(String, String)> = db
        .with_conn(|c| {
            let mut s = c.prepare("SELECT name,package_ids FROM on_demand_plans ORDER BY id")?;
            let rows = s
                .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?
                .collect::<rusqlite::Result<Vec<_>>>()?;
            Ok(rows)
        })
        .map_err(|e| e.to_string())?;
    for (name, ids) in &plans {
        let ids: Vec<String> =
            serde_json::from_str(ids).map_err(|e| format!("启用方案无法读取: {e}"))?;
        for raw in ids {
            for id in targets(&raw, &packages) {
                protect(&mut protection, id, reason("plan", name.clone()));
            }
        }
    }
    for (source, refs) in &local_refs {
        for raw in refs {
            for id in targets(raw, &packages) {
                protect(&mut protection, id, reason("local", source.clone()));
            }
        }
    }
    for (source, refs) in &unidentified_refs {
        for raw in refs {
            for id in targets(raw, &packages) {
                protect(&mut protection, id, reason("referencedFile", source.clone()));
            }
        }
    }
    // Scenes inside a package often name the package itself; that is not a use
    // by another resource and must not be reported as one.
    let edges: BTreeMap<String, BTreeSet<String>> = packages
        .iter()
        .map(|(id, p)| {
            (
                id.clone(),
                p.dependencies
                    .iter()
                    .flat_map(|raw| targets(raw, &packages))
                    .filter(|target| target != id)
                    .collect(),
            )
        })
        .collect();
    // A package is unusable only when every copy of it is.
    let problems: BTreeMap<String, String> = problem_files
        .into_iter()
        .filter(|(id, (count, _))| packages.get(id).is_some_and(|p| p.files.len() == *count))
        .map(|(id, (_, problem))| (id, problem))
        .collect();
    let mut referrers = BTreeMap::<String, BTreeSet<String>>::new();
    for (source, deps) in &edges {
        for dep in deps {
            referrers
                .entry(dep.clone())
                .or_default()
                .insert(source.clone());
        }
    }
    warnings.sort();
    warnings.dedup();
    diagnostics.sort();
    diagnostics.dedup();
    invalid_files.sort();
    invalid_files.dedup();
    stamps.sort_by(|a, b| a.path.cmp(&b.path));
    let revision = digest(&(
        &stamps,
        &packages,
        &protection,
        &marks,
        &plans,
        &local_refs,
        &unidentified_refs,
        &invalid_files,
        &problems,
        &warnings,
        &diagnostics,
        &reference_stamps,
        quick,
    ))?;
    if !diagnostics.is_empty() {
        log::debug!("清理检查：{} 个文件不是标准格式，已按兼容方式读完", diagnostics.len());
    }
    Ok(Catalog {
        packages,
        edges,
        referrers,
        protection,
        warnings,
        diagnostics,
        invalid_files,
        problems,
        blockers,
        revision,
        quick,
    })
}
fn closure(
    roots: impl IntoIterator<Item = String>,
    edges: &BTreeMap<String, BTreeSet<String>>,
) -> BTreeSet<String> {
    let mut seen = BTreeSet::new();
    let mut queue: Vec<_> = roots.into_iter().collect();
    while let Some(id) = queue.pop() {
        if seen.insert(id.clone()) {
            if let Some(next) = edges.get(&id) {
                queue.extend(next.iter().cloned());
            }
        }
    }
    seen
}
fn item(p: &Package, attached: bool, reasons: Vec<Reason>, issue: Option<&String>) -> PreviewItem {
    PreviewItem {
        package_id: p.id.clone(),
        file_count: p.files.len(),
        size_bytes: p.files.iter().map(|f| f.size).sum(),
        content_count: p.content_count,
        attached,
        reasons,
        issue: issue.cloned(),
        invalid_name: p.invalid_name,
    }
}
pub fn make_plan(
    db: &Database,
    ctx: Context,
    id: String,
    requested: Vec<String>,
    include_dependencies: bool,
    excluded: Vec<String>,
    cancel: &AtomicBool,
    progress: &mut impl FnMut(usize, usize),
) -> Result<Plan> {
    make_plan_with_progress(
        db,
        ctx,
        id,
        requested,
        include_dependencies,
        excluded,
        cancel,
        &mut |_, n, t| progress(n, t),
    )
}
pub fn make_plan_with_progress(
    db: &Database,
    ctx: Context,
    id: String,
    requested: Vec<String>,
    include_dependencies: bool,
    excluded: Vec<String>,
    cancel: &AtomicBool,
    progress: &mut impl FnMut(&str, usize, usize),
) -> Result<Plan> {
    validate_request(&requested)?;
    let cat = catalog(db, &ctx, false, cancel, progress)?;
    make_plan_from_catalog(
        db,
        ctx,
        &cat,
        id,
        requested,
        include_dependencies,
        excluded,
        cancel,
    )
}
pub fn validate_request(requested: &[String]) -> Result<()> {
    if requested.is_empty() || requested.len() > 100_000 || requested.iter().any(|id| !valid_id(id))
    {
        return Err("请选择有效资源包".into());
    }
    Ok(())
}
/// Build the reference catalog for `ctx` (the slow part of a preview).
///
/// With `quick`, packages are checked by their meta.json declarations only:
/// references made inside scenes/presets without being declared are missed,
/// so a quick plan can remove a package that is still in use. Local files,
/// favorites, plans and manual protection are always checked in full, and a
/// quick plan is re-verified in quick mode when executed.
pub fn build_catalog(
    db: &Database,
    ctx: &Context,
    quick: bool,
    cancel: &AtomicBool,
    progress: &mut impl FnMut(&str, usize, usize),
) -> Result<Catalog> {
    catalog(db, ctx, quick, cancel, progress)
}
/// Deduplication preserves independently referenced names, not just equal bytes.
pub(crate) fn dedup_protection(cat: &Catalog) -> super::dedup_analysis::Protection {
    let mut result = super::dedup_analysis::Protection {
        complete: !cat.quick && cat.warnings.is_empty() && cat.blockers.is_empty(),
        ..Default::default()
    };
    for (id, refs) in &cat.referrers {
        let refs: Vec<_> = refs.iter().filter(|source| *source != id).cloned().collect();
        if !refs.is_empty() { result.required.insert(id.clone(), refs); }
    }
    for (id, reasons) in &cat.protection {
        for reason in reasons {
            if reason.code == "manual" { result.manual.insert(id.clone()); }
            else { result.required.entry(id.clone()).or_default().push(reason.source.clone()); }
        }
    }
    result
}

/// What the cleanup page shows about a package before anything is selected.
/// Only packages with something to report are listed.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct PackageHealth {
    pub package_id: String,
    /// See [`archive_problem`].
    #[serde(default)]
    pub issue: Option<String>,
    #[serde(default)]
    pub invalid_name: bool,
    /// Files with this package name across the resource folders.
    pub copies: usize,
    /// No other package, local scene or preset, favorite or launch set uses it.
    pub unused: bool,
    /// A complete check found no use by others and no dependency on another package.
    #[serde(default)]
    pub standalone: bool,
}
/// Derived from a full catalog only: a quick one misses undeclared uses.
pub fn package_health(cat: &Catalog) -> Vec<PackageHealth> {
    cat.packages
        .iter()
        .filter_map(|(key, p)| {
            let issue = cat.problems.get(key).cloned();
            let used = cat.referrers.get(key).is_some_and(|r| !r.is_empty())
                || cat
                    .protection
                    .get(key)
                    .is_some_and(|reasons| reasons.iter().any(|r| r.code != "manual"));
            let copies = p.files.len();
            // Broad matches still protect installed packages. Only dependency
            // evidence can prove a missing external package: arbitrary dotted
            // text such as programVersion is not an outgoing dependency.
            let standalone = !cat.quick && cat.warnings.is_empty() && cat.blockers.is_empty()
                && issue.is_none() && !p.invalid_name && !used
                && cat.edges.get(key).is_none_or(BTreeSet::is_empty)
                && p.package_refs.iter().all(|raw| {
                    raw == key || (raw.ends_with(".latest") && targets(raw, &cat.packages) == vec![key.clone()])
                });
            (issue.is_some() || p.invalid_name || copies > 1 || !used).then(|| PackageHealth {
                package_id: p.id.clone(),
                issue,
                invalid_name: p.invalid_name,
                copies,
                unused: !used,
                standalone,
            })
        })
        .collect()
}
impl Catalog {
    pub fn is_quick(&self) -> bool {
        self.quick
    }
}
/// Plan a cleanup against an existing catalog. `execute` rebuilds the catalog
/// and compares revisions, so a reused catalog can never approve stale state.
pub fn make_plan_from_catalog(
    db: &Database,
    ctx: Context,
    cat: &Catalog,
    id: String,
    requested: Vec<String>,
    include_dependencies: bool,
    excluded: Vec<String>,
    cancel: &AtomicBool,
) -> Result<Plan> {
    make_plan_with_removal_choice(db, ctx, cat, id, requested, include_dependencies, excluded, &[], cancel)
}
/// Reasons that only say "something still uses this package". The person may
/// override them for a package they selected; they never apply to a package
/// that cannot be read at all, since nothing can actually use it.
fn usage_reason(code: &str) -> bool {
    matches!(
        code,
        "favorite" | "local" | "plan" | "referenced" | "referencedFile" | "protectedDependency" | "incomplete"
    )
}
/// Which candidates move and which stay, without touching the database.
struct Classified {
    preview: Preview,
    moves: Vec<StoredFile>,
}
fn classify(
    ctx: &Context,
    cat: &Catalog,
    id: &str,
    selected: &BTreeSet<String>,
    include_dependencies: bool,
    excluded: &BTreeSet<String>,
    force: &BTreeSet<String>,
    cancel: &AtomicBool,
) -> Result<Classified> {
    let candidates = if include_dependencies {
        closure(selected.iter().cloned(), &cat.edges)
    } else {
        selected.clone()
    };
    if !excluded.is_subset(&candidates) {
        return Err("保留项不属于本次清理范围".into());
    }
    let mut roots: BTreeSet<_> = cat
        .packages
        .keys()
        .filter(|id| !candidates.contains(*id))
        .cloned()
        .collect();
    roots.extend(cat.protection.keys().cloned());
    roots.extend(excluded.iter().cloned());
    let retained = closure(roots, &cat.edges);
    let mut preview = Preview {
        plan_id: id.to_string(),
        items: vec![],
        blocked: vec![],
        retained: vec![],
        warnings: cat.warnings.clone(),
        suggested: vec![],
        suggested_bytes: 0,
        diagnostics: cat.diagnostics.clone(),
        invalid_files: cat.invalid_files.clone(),
        requires_confirmation: cat.quick,
        total_bytes: 0,
        total_files: 0,
        quick: cat.quick,
    };
    let mut moves = vec![];
    for key in &candidates {
        checked(cancel)?;
        let Some(p) = cat.packages.get(key) else {
            preview.blocked.push(PreviewItem {
                package_id: key.clone(),
                file_count: 0,
                size_bytes: 0,
                content_count: 0,
                attached: !selected.contains(key),
                reasons: vec![reason("missing", key.clone())],
                issue: None,
                invalid_name: false,
            });
            continue;
        };
        let issue = cat.problems.get(key);
        let mut reasons = cat.protection.get(key).cloned().unwrap_or_default();
        reasons.extend(cat.blockers.clone());
        if excluded.contains(key) {
            reasons.push(reason("excluded", ""));
        }
        if retained.contains(key) {
            reasons.extend(
                cat.referrers
                    .get(key)
                    .into_iter()
                    .flatten()
                    .filter(|source| retained.contains(*source))
                    .map(|source| {
                        reason(
                            "referenced",
                            cat.packages
                                .get(source)
                                .map(|p| p.id.clone())
                                .unwrap_or_else(|| source.clone()),
                        )
                    }),
            );
            if reasons.is_empty() {
                reasons.push(reason("protectedDependency", p.id.clone()));
            }
        }
        if !cat.warnings.is_empty() {
            reasons.push(reason("incomplete", ""));
        }
        if cat.quick && !selected.contains(key) {
            reasons.push(reason("quickDependency", ""));
        }
        if issue.is_some() {
            // Unreadable in the game as well: references to it are already broken.
            reasons.retain(|r| !usage_reason(&r.code) && r.code != "quickDependency");
        }
        let override_usage = force.contains(key)
            && selected.contains(key)
            && reasons.iter().all(|r| usage_reason(&r.code));
        if !reasons.is_empty() && !override_usage {
            let row = item(p, !selected.contains(key), reasons, issue);
            if selected.contains(key) {
                preview.blocked.push(row);
            } else {
                preview.retained.push(row);
            }
        } else {
            for file in &p.files {
                let root = ctx
                    .roots
                    .iter()
                    .find(|root| Path::new(&file.path).starts_with(root))
                    .ok_or("文件不属于当前实例")?;
                // Hashed when the plan is executed, not while previewing.
                moves.push(StoredFile {
                    package_id: p.id.clone(),
                    original: file.clone(),
                    hash: String::new(),
                    trash_path: Path::new(root)
                        .join(".trash")
                        .join(format!("{}_{}.var", preview.plan_id, moves.len()))
                        .to_string_lossy()
                        .into_owned(),
                    status: "planned".into(),
                    verified_stamp: None,
                });
            }
            preview.requires_confirmation |= !reasons.is_empty();
            preview.items.push(item(p, !selected.contains(key), reasons, issue));
        }
    }
    preview.total_bytes = moves.iter().map(|f| f.original.size).sum();
    preview.total_files = moves.len();
    Ok(Classified { preview, moves })
}
/// `force` lists selected packages the person chose to remove even though
/// something still uses them. It only overrides usage checks for those
/// packages; attached dependencies and manual or directory protections stay.
pub fn make_plan_with_removal_choice(
    db: &Database,
    ctx: Context,
    cat: &Catalog,
    id: String,
    requested: Vec<String>,
    include_dependencies: bool,
    excluded: Vec<String>,
    force: &[String],
    cancel: &AtomicBool,
) -> Result<Plan> {
    validate_request(&requested)?;
    let selected: BTreeSet<_> = requested.iter().map(|id| id.to_lowercase()).collect();
    let excluded: BTreeSet<_> = excluded.iter().map(|id| id.to_lowercase()).collect();
    let force: BTreeSet<_> = force.iter().map(|id| id.to_lowercase()).collect();
    if !force.is_subset(&selected) {
        return Err("只能对选中的资源包选择仍然移走".into());
    }
    let Classified { mut preview, moves } =
        classify(&ctx, cat, &id, &selected, include_dependencies, &excluded, &force, cancel)?;
    if !include_dependencies {
        // What "also clean up dependencies" would add, so the choice shows its effect.
        let with_dependencies =
            classify(&ctx, cat, &id, &selected, true, &excluded, &force, cancel)?;
        preview.suggested = with_dependencies
            .preview
            .items
            .into_iter()
            .filter(|item| item.attached)
            .collect();
        preview.suggested_bytes = preview.suggested.iter().map(|item| item.size_bytes).sum();
    }
    let metadata = item_metadata(db, &ctx, &preview.items)?;
    Ok(Plan {
        preview,
        ctx,
        requested,
        include_dependencies,
        revision: cat.revision.clone(),
        files: moves,
        metadata,
        quick: cat.quick,
    })
}
/// Capture metadata and index snapshots with bounded batched queries; avoid a
/// scan per package (`id=? COLLATE NOCASE` cannot use the primary-key index).
fn item_metadata(
    db: &Database,
    ctx: &Context,
    items: &[PreviewItem],
) -> Result<BTreeMap<String, Metadata>> {
    if items.is_empty() {
        return Ok(BTreeMap::new());
    }
    let wanted: HashSet<String> = items.iter().map(|i| i.package_id.to_lowercase()).collect();
    db.with_conn(|c| {
        // id -> (id, file_path, scan_time, created_at), first row in rowid order.
        let mut rows =
            BTreeMap::<String, Vec<(String, String, Option<String>, Option<String>)>>::new();
        let mut s = c.prepare_cached(
            "SELECT id,file_path,scan_time,created_at FROM packages ORDER BY rowid",
        )?;
        let mut q = s.query([])?;
        while let Some(r) = q.next()? {
            let id: String = r.get(0)?;
            let key = id.to_lowercase();
            if wanted.contains(&key) {
                rows.entry(key)
                    .or_default()
                    .push((id, r.get(1)?, r.get(2).ok(), r.get(3).ok()));
            }
        }
        // Match Unicode/case aliases once using Rust's normalization, then use
        // the existing primary/foreign-key indexes for the selected records only.
        let indexed_ids: Vec<_> = rows.values().flatten().map(|row| row.0.as_str()).collect();
        let snapshot_ids: Vec<_> = rows.values().filter_map(|rows| rows.first().map(|row| row.0.as_str())).collect();
        let mut tags = BTreeMap::<String, Vec<String>>::new();
        let mut snapshots = BTreeMap::new();
        for chunk in snapshot_ids.chunks(400) {
            let placeholders = vec!["?"; chunk.len()].join(",");
            let mut s = c.prepare(&format!("SELECT id,creator,name,version,file_path,size_bytes,resource_types,meta_json,COALESCE(file_created_time,created_at),scan_time FROM packages WHERE id IN ({placeholders})"))?;
            let mut q = s.query(rusqlite::params_from_iter(chunk.iter()))?;
            while let Some(r) = q.next()? {
                let id: String = r.get(0)?;
                let types: String = r.get(6)?;
                let meta: Option<String> = r.get(7)?;
                snapshots.insert(id.to_lowercase(), crate::models::var_package::VarPackage {
                    id, creator: r.get(1)?, name: r.get(2)?, version: r.get(3)?,
                    file_path: r.get(4)?, size_bytes: r.get(5)?,
                    resource_types: serde_json::from_str(&types)?,
                    meta: meta.as_deref().map(serde_json::from_str).transpose()?,
                    created_time: r.get(8)?, scan_time: r.get(9)?,
                    modified_time: String::new(), contents: vec![],
                });
            }
        }
        let mut contents = BTreeMap::<String, Vec<(i64, String, u64)>>::new();
        for chunk in indexed_ids.chunks(400) {
            let placeholders = vec!["?"; chunk.len()].join(",");
            let mut s = c.prepare(&format!("SELECT package_id,tag FROM package_tags WHERE package_id IN ({placeholders})"))?;
            let mut q = s.query(rusqlite::params_from_iter(chunk.iter()))?;
            while let Some(r) = q.next()? {
                tags.entry(r.get::<_, String>(0)?.to_lowercase()).or_default().push(r.get(1)?);
            }
            let mut s = c.prepare(&format!("SELECT package_id,id,file_path,size_bytes FROM contents WHERE package_id IN ({placeholders})"))?;
            let mut q = s.query(rusqlite::params_from_iter(chunk.iter()))?;
            while let Some(r) = q.next()? {
                contents.entry(r.get::<_, String>(0)?.to_lowercase()).or_default().push((r.get(1)?,r.get(2)?,r.get(3)?));
            }
        }
        for tags in tags.values_mut() { tags.sort(); }
        for (key, mut rows) in contents {
            rows.sort_by_key(|row| row.0);
            if let Some(snapshot) = snapshots.get_mut(&key) {
                snapshot.contents = rows.into_iter().map(|(_, path, size)| (path, size)).collect();
            }
        }
        let mut pending = BTreeMap::<String, bool>::new();
        let mut s = c.prepare_cached("SELECT package_id,pending FROM cleanup_marks WHERE library_key=?1")?;
        let mut q = s.query([&ctx.library_key])?;
        while let Some(r) = q.next()? { pending.insert(r.get(0)?, r.get(1)?); }
        let mut all = BTreeMap::new();
        for row in items {
            let key = row.package_id.to_lowercase();
            let indexed = rows.get(&key).and_then(|v| v.first());
            if indexed.is_some_and(|(_, path, _, _)| !ctx.owns(Path::new(path))) {
                return Err(AppError::Validation(
                    "资源索引来自其他实例，请先重新扫描当前实例".into(),
                ));
            }
            let times = rows
                .get(&key)
                .and_then(|v| v.iter().find(|(id, ..)| *id == row.package_id))
                .and_then(|(_, _, scan, created)| Some((scan.clone()?, created.clone()?)));
            all.insert(
                row.package_id.clone(),
                Metadata {
                    tags: tags.remove(&key).unwrap_or_default(),
                    scan_time: times.as_ref().map(|t| t.0.clone()),
                    created_at: times.map(|t| t.1),
                    pending: pending.get(&key).copied().unwrap_or(false),
                    was_indexed: Some(indexed.is_some()),
                    index_snapshot: snapshots.remove(&key),
                },
            );
        }
        Ok(all)
    })
    .map_err(|e| e.to_string())
}

fn batch_dir(data: &Path, ctx: &Context) -> PathBuf {
    data.join("cleanup-batches")
        .join(hex::encode(Sha256::digest(ctx.library_key.as_bytes())))
}
fn manifest(data: &Path, batch: &Batch) -> PathBuf {
    batch_dir(data, &batch.context).join(format!("{}.json", batch.id))
}
fn safe_batch_id(id: &str) -> bool {
    !id.is_empty() && id.len() < 100 && id.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-')
}
fn write_batch(data: &Path, batch: &Batch) -> Result<()> {
    let path = manifest(data, batch);
    files::ensure_no_links(&path)?;
    fs::create_dir_all(path.parent().ok_or("无效批次目录")?).map_err(|e| e.to_string())?;
    let temp = path.with_extension("tmp");
    files::ensure_no_links(&temp)?;
    let mut file = fs::OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .open(&temp)
        .map_err(|e| e.to_string())?;
    let bytes = serde_json::to_vec(batch).map_err(|e| e.to_string())?;
    #[cfg(test)]
    JOURNAL_IO.with(|io| { let (writes, total) = io.get(); io.set((writes + 1, total + bytes.len())); });
    file.write_all(&bytes)
        .and_then(|_| file.sync_all())
        .map_err(|e| e.to_string())?;
    drop(file);
    fs::rename(temp, path).map_err(|e| e.to_string())
}
#[cfg(test)]
thread_local! { static JOURNAL_IO: std::cell::Cell<(usize, usize)> = const { std::cell::Cell::new((0, 0)) }; }
fn validate_batch(batch: &Batch, ctx: &Context) -> Result<()> {
    if batch.context != *ctx || !safe_batch_id(&batch.id) {
        return Err("清理批次不属于当前实例或资源目录已变化".into());
    }
    for (i, file) in batch.files.iter().enumerate() {
        let source = Path::new(&file.original.path);
        let trash = Path::new(&file.trash_path);
        if !ctx.owns(source)
            || source
                .components()
                .any(|c| matches!(c, std::path::Component::ParentDir))
            || source.components().any(|c| {
                c.as_os_str()
                    .to_string_lossy()
                    .eq_ignore_ascii_case(".trash")
            })
        {
            return Err("无效清理源文件".into());
        }
        let expected = ctx
            .roots
            .iter()
            .filter(|r| source.starts_with(r))
            .map(|r| {
                Path::new(r)
                    .join(".trash")
                    .join(format!("{}_{}.var", batch.id, i))
            })
            .any(|p| p == trash);
        if !expected
            || file.hash.len() != 64
            || hex::decode(&file.hash).is_err()
            || !valid_id(&file.package_id)
            || !cleanup_file_id(source)
                .is_some_and(|id| id.eq_ignore_ascii_case(&file.package_id))
        {
            return Err("无效回收文件记录".into());
        }
        files::ensure_no_links(source)?;
        files::ensure_no_links(trash)?;
    }
    Ok(())
}
fn load_batch(data: &Path, ctx: &Context, id: &str) -> Result<Batch> {
    if !safe_batch_id(id) {
        return Err("无效批次标识".into());
    }
    let path = batch_dir(data, ctx).join(format!("{id}.json"));
    files::ensure_no_links(&path)?;
    // Recovery snapshots may be large. serde_json reads individual bytes;
    // buffering avoids one filesystem read for every byte in the journal.
    let batch: Batch = serde_json::from_reader(std::io::BufReader::new(fs::File::open(path).map_err(|e| e.to_string())?))
        .map_err(|e| e.to_string())?;
    if batch.id != id {
        return Err("批次记录标识不一致".into());
    }
    validate_batch(&batch, ctx)?;
    Ok(batch)
}
fn verify_hash(path: &Path, hash: &str) -> Result<()> {
    let snapshot = files::fingerprint(path)?;
    if files::hash_file(&snapshot)? != hash {
        return Err("文件内容已变化，请重新预览或检查回收记录".into());
    }
    Ok(())
}
fn delete_index(db: &Database, batch: &Batch) -> Result<()> {
    let removed: HashSet<_> = batch.files.iter()
        .filter(|file| file.status == "trashed" && !Path::new(&file.original.path).exists())
        .map(|file| files::path_key(&file.original.path))
        .collect();
    db.with_conn(|c| {
        let tx = c.unchecked_transaction()?;
        for file in batch
            .files
            .iter()
            .filter(|f| removed.contains(&files::path_key(&f.original.path)))
        {
            tx.execute(
                "DELETE FROM physical_packages WHERE file_path=?1",
                [&file.original.path],
            )?;
        }
        for id in batch.metadata.keys() {
            let current = tx
                .query_row("SELECT file_path FROM packages WHERE id=?1", [id], |r| {
                    r.get::<_, String>(0)
                })
                .ok();
            if current.is_some_and(|path| removed.contains(&files::path_key(&path))) {
                tx.execute("DELETE FROM packages WHERE id=?1", [id])?;
            }
            tx.execute(
                "UPDATE cleanup_marks SET pending=0 WHERE library_key=?1 AND package_id=?2",
                params![batch.context.library_key, id.to_lowercase()],
            )?;
        }
        tx.commit()?;
        Ok(())
    })
    .map_err(|e| e.to_string())
}
fn restore_index(db: &Database, batch: &Batch, rollback: bool) -> Result<()> {
    db.with_conn(|c| {
        let tx=c.unchecked_transaction()?;
        let mut already_indexed=BTreeSet::new();
        for id in batch.metadata.keys(){
            let path=tx.query_row("SELECT file_path FROM packages WHERE id=?1",[id],|r|r.get::<_,String>(0)).ok();
            if path.is_some_and(|path|batch.context.owns(Path::new(&path))){already_indexed.insert(id.clone());}
        }
        for file in batch.files.iter().filter(|file| file.status == "restored") {
            let path = Path::new(&file.original.path);
            match super::var_parser::parse_var_file(path) {
                Ok(package) => files::index_parsed_package(&tx, &package)?,
                Err(error) => {
                    let meta = batch.metadata.get(&file.package_id);
                    if let Some(snapshot) = meta.and_then(|meta| meta.index_snapshot.as_ref()) {
                        if snapshot.id != file.package_id {
                            return Err(AppError::Validation("恢复记录的资源名称不一致".into()));
                        }
                        let mut package = snapshot.clone();
                        package.file_path = file.original.path.clone();
                        package.size_bytes = file.original.size;
                        package.modified_time = chrono::DateTime::<chrono::Utc>::from(fs::metadata(path)?.modified()?).to_rfc3339();
                        if !already_indexed.contains(&file.package_id) {
                            files::index_parsed_package(&tx, &package)?;
                        }
                        // Restore the file and prior record, but never claim a corrupt
                        // archive has passed a new scan. SQL failures still abort.
                        tx.execute("INSERT INTO physical_packages(file_path,package_id,size_bytes,modified_time,scan_time,scan_status,last_error) VALUES(?1,?2,?3,?4,?5,'failed',?6) ON CONFLICT(file_path) DO UPDATE SET scan_status='failed',last_error=excluded.last_error",
                            params![package.file_path,package.id,package.size_bytes,package.modified_time,package.scan_time,error.to_string()])?;
                    } else if meta.and_then(|meta| meta.was_indexed) != Some(false) {
                        return Err(error);
                    } else {
                        // Keep failed scans visible after restoring, without inventing
                        // parsed package metadata for an unreadable or renamed file.
                        let modified = chrono::DateTime::<chrono::Utc>::from(fs::metadata(path)?.modified()?).to_rfc3339();
                        tx.execute("INSERT INTO physical_packages(file_path,package_id,size_bytes,modified_time,scan_time,scan_status,last_error) VALUES(?1,?2,?3,?4,?5,'failed',?6) ON CONFLICT(file_path) DO UPDATE SET package_id=excluded.package_id,size_bytes=excluded.size_bytes,modified_time=excluded.modified_time,scan_time=excluded.scan_time,scan_status='failed',last_error=excluded.last_error,file_md5=NULL",
                            params![file.original.path,file.package_id,file.original.size,modified,chrono::Utc::now().to_rfc3339(),error.to_string()])?;
                    }
                }
            }
        }
        for (id,meta) in &batch.metadata {
            if batch.files.iter().any(|f|f.package_id==*id && f.status=="restored") {
                if !already_indexed.contains(id) { for tag in &meta.tags {tx.execute("INSERT OR IGNORE INTO package_tags(package_id,tag) SELECT ?1,?2 WHERE EXISTS(SELECT 1 FROM packages WHERE id=?1)",params![id,tag])?;}
                tx.execute("UPDATE packages SET scan_time=COALESCE(?2,scan_time),created_at=COALESCE(?3,created_at) WHERE id=?1",params![id,meta.scan_time,meta.created_at])?;
                }
                tx.execute("UPDATE cleanup_marks SET pending=?3 WHERE library_key=?1 AND package_id=?2",params![batch.context.library_key,id.to_lowercase(),rollback && meta.pending])?;
            }
        }
        tx.commit()?;Ok(())
    }).map_err(|e|e.to_string())
}
pub fn execute(
    db: &Database,
    data: &Path,
    plan: Plan,
    cancel: &AtomicBool,
    progress: &mut impl FnMut(usize, usize),
) -> Result<Batch> {
    execute_with_progress(db, data, plan, false, cancel, &mut |stage, n, t| {
        if stage == "moving" {
            progress(n, t)
        }
    })
}
/// Prove a planned file still holds the hashed bytes right before it moves:
/// an unchanged change stamp suffices; otherwise (or without stamps) re-hash.
fn verify_unchanged(file: &StoredFile, stamp: &Option<String>) -> Result<()> {
    verify_hashed_file(&file.original, &file.hash, stamp)
}
fn verify_hashed_file(file: &files::DiskFile, hash: &str, stamp: &Option<String>) -> Result<()> {
    files::verify(file)?;
    let path = Path::new(&file.path);
    if stamp.is_some() && change_stamp(path)? == *stamp {
        return Ok(());
    }
    verify_hash(path, hash)
}
/// Execute a plan. Stages: the catalog stages while re-checking references,
/// then `verifying` (hashing the files once) and `moving`.
pub fn execute_with_progress(
    db: &Database,
    data: &Path,
    mut plan: Plan,
    confirm_risks: bool,
    cancel: &AtomicBool,
    progress: &mut impl FnMut(&str, usize, usize),
) -> Result<Batch> {
    if plan.files.is_empty() {
        return Err("没有可清理的资源，请查看保留原因".into());
    }
    if plan.preview.requires_confirmation && !confirm_risks {
        return Err("请先确认清理对其他资源的影响".into());
    }
    let _guard = files::maintenance_lock()?;
    checked(cancel)?;
    let current = catalog(db, &plan.ctx, plan.quick, cancel, progress)?;
    if current.revision != plan.revision {
        return Err("资源、引用或整理状态已变化，请重新预览".into());
    }
    // Previews no longer hash; each file is read once here. The revision check
    // above already proves the files are those that were previewed.
    let mut guards = Vec::with_capacity(plan.files.len());
    let total = plan.files.len();
    progress("verifying", 0, total);
    let total_bytes = plan.files.iter().map(|f| f.original.size).sum::<u64>();
    let mut verified_bytes = 0u64;
    let mut reported = std::time::Instant::now();
    // Streaming one archive at a time avoids disk contention on large selections.
    for file in &mut plan.files {
        checked(cancel)?;
        let before = change_stamp(Path::new(&file.original.path))?;
        let hash = files::hash_file_with_progress(&file.original, &mut |bytes| {
            checked(cancel)?;
            if bytes == file.original.size || reported.elapsed() >= std::time::Duration::from_millis(80) {
                progress("verifying_bytes", (verified_bytes + bytes) as usize, total_bytes as usize);
                reported = std::time::Instant::now();
            }
            Ok(())
        })?;
        checked(cancel)?;
        if change_stamp(Path::new(&file.original.path))? != before {
            return Err("文件在校验过程中发生变化，请重新预览".into());
        }
        file.hash = hash;
        guards.push(before);
        verified_bytes += file.original.size;
        progress("verifying", guards.len(), total);
    }
    if Path::new(&plan.ctx.vam_root).join("VaM.exe").is_file() {
        super::scene_launch::ensure_game_stopped(Path::new(&plan.ctx.vam_root))?;
    }
    let now = chrono::Utc::now();
    let days = policy(db)?.retention_days;
    let mut batch = Batch {
        id: plan.preview.plan_id,
        context: plan.ctx,
        created_at: now.to_rfc3339(),
        expires_at: (now + chrono::Duration::days(days)).to_rfc3339(),
        status: "processing".into(),
        files: plan.files,
        metadata: plan.metadata,
        errors: vec![],
    };
    validate_batch(&batch, &batch.context)?;
    if manifest(data, &batch).exists() {
        return Err("该批次已存在，请重新生成预览".into());
    }
    write_batch(data, &batch)?;
    progress("moving", 0, batch.files.len());
    let operation = (|| {
        for i in 0..batch.files.len() {
            checked(cancel)?;
            let file = &batch.files[i];
            let pinned = files::pin_readonly(Path::new(&file.original.path));
            verify_unchanged(file, &guards[i])?;
            files::transfer(
                Path::new(&file.original.path),
                Path::new(&file.trash_path),
                "move",
            )?;
            batch.files[i].status = "trashed".into();
            if let Some(source_pin) = pinned {
                let path = Path::new(&batch.files[i].trash_path);
                // Closing the last source-name handle can finalize its unlink and
                // change NTFS ChangeTime. Overlap the destination pin before closing
                // it, so the final stamp is taken after unlink without allowing writes.
                let destination_pin = files::pin_readonly(path);
                let source_stamp = super::cleanup_reference_cache::pinned_change_stamp(&source_pin)?;
                let same_file = destination_pin.as_ref()
                    .map(super::cleanup_reference_cache::pinned_change_stamp).transpose()?
                    .is_some_and(|stamp| stamp.is_some() && stamp == source_stamp);
                drop(source_pin);
                if same_file {
                    let stamp = destination_pin.as_ref()
                        .map(super::cleanup_reference_cache::pinned_change_stamp).transpose()?.flatten();
                    if stamp.is_some() && change_stamp(path)? == stamp {
                        batch.files[i].verified_stamp = stamp;
                    }
                } else {
                    // A cross-volume copy cannot inherit the source's evidence.
                    verify_hash(path, &batch.files[i].hash)?;
                }
            }
            if (i + 1) % JOURNAL_CHUNK == 0 {
                write_batch(data, &batch)?;
            }
            progress("moving", i + 1, batch.files.len());
        }
        delete_index(db, &batch)?;
        batch.status = "completed".into();
        write_batch(data, &batch)
    })();
    if let Err(e) = operation {
        batch.errors.push(e);
        for i in (0..batch.files.len()).rev() {
            if batch.files[i].status != "trashed" {
                continue;
            }
            let file = &batch.files[i];
            let result = verify_hash(Path::new(&file.trash_path), &file.hash).and_then(|_| {
                files::transfer(
                    Path::new(&file.trash_path),
                    Path::new(&file.original.path),
                    "move",
                )
            });
            match result {
                Ok(()) => batch.files[i].status = "restored".into(),
                Err(e) => batch.errors.push(format!("{}: {e}", file.package_id)),
            }
        }
        if let Err(e) = restore_index(db, &batch, true) {
            batch.errors.push(e);
        }
        batch.status =
            if batch.files.iter().any(|f| f.status == "trashed") || batch.errors.len() > 1 {
                "needs_repair"
            } else {
                "rolled_back"
            }
            .into();
        write_batch(data, &batch)?;
    }
    Ok(batch)
}
pub fn list_batches(data: &Path, ctx: &Context) -> Result<Vec<Batch>> {
    let dir = batch_dir(data, ctx);
    files::ensure_no_links(&dir)?;
    if !dir.exists() {
        return Ok(vec![]);
    }
    let mut batches = vec![];
    for entry in fs::read_dir(dir).map_err(|e| e.to_string())? {
        let path = entry.map_err(|e| e.to_string())?.path();
        if path.extension().is_some_and(|x| x == "json") {
            batches.push(load_batch(
                data,
                ctx,
                path.file_stem()
                    .and_then(|s| s.to_str())
                    .ok_or("无效批次文件")?,
            )?);
        }
    }
    batches.sort_by(|a, b| b.created_at.cmp(&a.created_at));
    Ok(batches)
}
pub fn restore(db: &Database, data: &Path, ctx: &Context, id: &str) -> Result<Batch> {
    let _guard = files::maintenance_lock()?;
    let mut batch = load_batch(data, ctx, id)?;
    if batch.status == "purged" {
        return Err("该批次已永久删除，无法恢复".into());
    }
    if batch.status == "restored" {
        return Ok(batch);
    }
    db.with_conn(|c| {
        for id in batch.metadata.keys() {
            let path = c
                .query_row(
                    "SELECT file_path FROM packages WHERE id=?1 COLLATE NOCASE",
                    [id],
                    |r| r.get::<_, String>(0),
                )
                .ok();
            if path.is_some_and(|p| !ctx.owns(Path::new(&p))) {
                return Err(AppError::Validation(
                    "资源索引来自其他实例，请先重新扫描当前实例".into(),
                ));
            }
        }
        Ok(())
    })
    .map_err(|e| e.to_string())?;
    batch.errors.clear();
    // Reconcile interrupted writes from the durable file list. Never move or
    // remove an unverified file merely because the journal says it exists.
    for i in 0..batch.files.len() {
        if batch.files[i].status == "purged" {
            batch.errors.push(format!(
                "{} 已永久删除，无法恢复",
                batch.files[i].package_id
            ));
            continue;
        }
        let file = &batch.files[i];
        let source = Path::new(&file.original.path);
        let trash = Path::new(&file.trash_path);
        let result: Result<()> = (|| {
            if trash.exists() {
                verify_hash(trash, &file.hash)?;
                if source.exists() {
                    verify_hash(source, &file.hash)?;
                    fs::remove_file(trash).map_err(|e| e.to_string())?;
                } else {
                    files::transfer(trash, source, "move")?;
                }
            } else {
                verify_hash(source, &file.hash)?;
            }
            Ok(())
        })();
        match result {
            Ok(()) => batch.files[i].status = "restored".into(),
            Err(e) => batch.errors.push(format!("{}: {e}", file.package_id)),
        }
        write_batch(data, &batch)?;
    }
    if let Err(e) = restore_index(db, &batch, false) {
        batch.errors.push(e);
    }
    batch.status = if batch.errors.is_empty() {
        "restored"
    } else {
        "needs_repair"
    }
    .into();
    write_batch(data, &batch)?;
    Ok(batch)
}
pub fn purge(data: &Path, ctx: &Context, id: &str) -> Result<Batch> {
    purge_with_progress(data, ctx, id, &mut |_, _, _| {})
}
pub fn purge_with_progress(data: &Path, ctx: &Context, id: &str, progress: &mut impl FnMut(&str, usize, usize)) -> Result<Batch> {
    let _guard = files::maintenance_lock()?;
    let mut batch = load_batch(data, ctx, id)?;
    if !matches!(batch.status.as_str(), "completed" | "purging" | "purged") {
        return Err("该批次存在未完成恢复，请先处理异常".into());
    }
    if batch.status == "purged" {
        return Ok(batch);
    }
    if batch
        .files
        .iter()
        .any(|f| !matches!(f.status.as_str(), "trashed" | "purging" | "purged"))
    {
        return Err("该批次文件状态不完整，请先检查恢复".into());
    }
    batch.status = "purging".into();
    for start in (0..batch.files.len()).step_by(JOURNAL_CHUNK) {
        let end = (start + JOURNAL_CHUNK).min(batch.files.len());
        let mut verified = Vec::new();
        for i in start..end {
            let file = &mut batch.files[i];
            if file.status == "purged" { continue; }
            let path = Path::new(&file.trash_path);
            files::ensure_no_links(path)?;
            if file.status == "purging" && !path.try_exists().map_err(|e| e.to_string())? {
                file.status = "purged".into();
                continue;
            }
            let pinned = files::pin_readonly(path);
            let snapshot = files::fingerprint(path)?;
            let before = change_stamp(path)?;
            let pinned_stamp = pinned.as_ref().map(super::cleanup_reference_cache::pinned_change_stamp).transpose()?.flatten();
            let trusted = before.is_some() && pinned_stamp == before && before == file.verified_stamp
                && snapshot.size == file.original.size && snapshot.modified == file.original.modified;
            let stamp = if trusted { before } else {
                let mut reported = std::time::Instant::now();
                let hash = files::hash_file_with_progress(&snapshot, &mut |bytes| {
                    if bytes == snapshot.size || reported.elapsed() >= std::time::Duration::from_millis(80) {
                        progress("purging_bytes", bytes as usize, snapshot.size as usize);
                        reported = std::time::Instant::now();
                    }
                    Ok(())
                })?;
                if hash != file.hash || change_stamp(path)? != before {
                    return Err("文件内容已变化，请重新预览或检查回收记录".into());
                }
                before
            };
            verified.push((i, snapshot, stamp, pinned));
            file.status = "purging".into();
        }
        // Persist deletion intent before unlinking anything. After interruption,
        // only files marked purging may be absent and safely resumed.
        write_batch(data, &batch)?;
        for (i, snapshot, stamp, _pinned) in verified {
            // No second full read when the reliable change stamp is unchanged.
            // Filesystems without one retain the hash-based fallback.
            verify_hashed_file(&snapshot, &batch.files[i].hash, &stamp)?;
            fs::remove_file(&snapshot.path).map_err(|e| e.to_string())?;
            batch.files[i].status = "purged".into();
            progress("purging", i + 1, batch.files.len());
        }
        // The final checkpoint records both file results and the completed
        // batch status atomically; another identical journal write is unnecessary.
        if end == batch.files.len() { batch.status = "purged".into(); }
        write_batch(data, &batch)?;
    }
    if batch.status != "purged" {
        batch.status = "purged".into();
        write_batch(data, &batch)?;
    }
    Ok(batch)
}
pub fn extend(data: &Path, ctx: &Context, id: &str, days: i64) -> Result<Batch> {
    if !(1..=365).contains(&days) {
        return Err("无效保留天数".into());
    }
    let _guard = files::maintenance_lock()?;
    let mut batch = load_batch(data, ctx, id)?;
    batch.expires_at = (chrono::Utc::now() + chrono::Duration::days(days)).to_rfc3339();
    write_batch(data, &batch)?;
    Ok(batch)
}

/// Legacy deduplication may remove physical copies, but it must not bypass
/// protection or mistake two different archives with the same ID for duplicates.
pub fn validate_deletions(db: &Database, ctx: &Context, paths: &[String]) -> Result<()> {
    if preserves_package_copies(ctx, paths)? { return Ok(()); }
    validate_dedup_deletions(db, ctx, paths, &std::collections::HashMap::new())
}

/// `equivalent` is server-side evidence, verified and pinned by the dedup executor.
pub(crate) fn validate_dedup_deletions(db: &Database, ctx: &Context, paths: &[String], equivalent: &std::collections::HashMap<String,String>) -> Result<()> {
    let selected_ids: BTreeSet<_> = paths.iter().filter_map(|p| cleanup_file_id(Path::new(p))).map(|id|id.to_lowercase()).collect();
    if marks(db, ctx)?.iter().any(|m| m.protected && selected_ids.contains(&m.package_id.to_lowercase())) {
        return Err("清理项已被手动保护，请重新分析".into());
    }
    let proven_copies = !paths.is_empty() && paths.iter().all(|path| equivalent.get(path).is_some_and(|keep| {
        !paths.contains(keep) && Path::new(keep).exists() && ctx.owns(Path::new(keep))
            && Path::new(path).file_stem() == Path::new(keep).file_stem()
    }));
    if proven_copies || (equivalent.is_empty() && preserves_package_copies(ctx, paths)?) {
        return Ok(());
    }
    let cancel = AtomicBool::new(false);
    let cat = catalog(db, ctx, false, &cancel, &mut |_, _, _| {})?;
    let selected: HashSet<_> = paths.iter().map(|p| files::path_key(p)).collect();
    let mut removed = BTreeSet::new();
    for (id, p) in &cat.packages {
        let deleting: Vec<_> = p
            .files
            .iter()
            .filter(|f| selected.contains(&files::path_key(&f.path)))
            .collect();
        if deleting.is_empty() {
            continue;
        }
        let remaining: Vec<_> = p
            .files
            .iter()
            .filter(|f| !selected.contains(&files::path_key(&f.path)))
            .collect();
        if remaining.is_empty() {
            removed.insert(id.clone());
        } else {
            let unproven: Vec<_> = deleting.into_iter().filter(|file| !equivalent.get(&file.path)
                .is_some_and(|keep| remaining.iter().any(|f| f.path == *keep))).collect();
            if unproven.is_empty() { continue; }
            let hashes = remaining
                .iter()
                .map(|f| files::hash_file(f))
                .collect::<Result<Vec<_>>>()?;
            for file in unproven {
                if !hashes.contains(&files::hash_file(file)?) {
                    return Err(format!(
                        "同包同版本内容不同，不能当作重复副本删除: {}",
                        p.id
                    ));
                }
            }
        }
    }
    if removed.is_empty() {
        return Ok(());
    }
    if !cat.warnings.is_empty() {
        return Err("引用检查不完整，不能删除整个包或旧版本，请先处理扫描异常".into());
    }
    let mut roots: Vec<_> = cat
        .packages
        .keys()
        .filter(|id| !removed.contains(*id))
        .cloned()
        .collect();
    roots.extend(cat.protection.keys().cloned());
    let keep = closure(roots, &cat.edges);
    if removed.iter().any(|id| keep.contains(id)) {
        return Err("清理项仍被收藏、本地文件或其他资源引用，请在资源清理预览中查看原因".into());
    }
    Ok(())
}

/// Removing byte-identical copies leaves every package and its references available.
/// Prove that from filenames and file contents before opening unrelated archives.
fn preserves_package_copies(ctx: &Context, paths: &[String]) -> Result<bool> {
    if paths.is_empty() {
        return Ok(true);
    }
    if paths.iter().any(|path| !ctx.owns(Path::new(path))) {
        return Err("清理文件不属于当前游戏实例".into());
    }
    let has_nonstandard = paths.iter().any(|path| has_extension(Path::new(path), "var") && files::package_name(Path::new(path)).is_none());
    let selected: HashSet<_> = paths.iter().map(|p| files::path_key(p)).collect();
    let ids: BTreeSet<_> = paths
        .iter()
        .filter_map(|path| files::package_name(Path::new(path)).map(|name| name.id.to_lowercase()))
        .collect();
    let cancel = AtomicBool::new(false);
    let mut inventory = BTreeMap::new();
    let mut found = HashSet::new();
    for root in &ctx.roots {
        for file in cleanup_inventory(Path::new(root), &cancel, all_files)?.0 {
            let key = files::path_key(&file.path);
            if selected.contains(&key) {
                found.insert(key.clone());
            }
            if files::package_name(Path::new(&file.path))
                .is_some_and(|name| ids.contains(&name.id.to_lowercase()))
            {
                inventory.insert(key, file);
            }
        }
    }
    if found != selected {
        return Err("清理文件已变化、包含链接或不在资源目录中，请重新扫描".into());
    }
    if has_nonstandard { return Ok(false); }
    let mut packages: BTreeMap<String, Vec<files::DiskFile>> = BTreeMap::new();
    for file in inventory.into_values() {
        let id = files::package_name(Path::new(&file.path))
            .unwrap()
            .id
            .to_lowercase();
        packages.entry(id).or_default().push(file);
    }
    // A whole version is being removed: its consumers still need the full check.
    if packages.values().any(|copies| {
        copies
            .iter()
            .all(|f| selected.contains(&files::path_key(&f.path)))
    }) {
        return Ok(false);
    }
    for (id, copies) in packages {
        let deleting: Vec<_> = copies
            .iter()
            .filter(|f| selected.contains(&files::path_key(&f.path)))
            .collect();
        let sizes: HashSet<_> = deleting.iter().map(|f| f.size).collect();
        let mut retained_hashes = HashSet::new();
        for file in copies
            .iter()
            .filter(|f| !selected.contains(&files::path_key(&f.path)) && sizes.contains(&f.size))
        {
            retained_hashes.insert(files::hash_file(file)?);
        }
        for file in deleting {
            if !retained_hashes.contains(&files::hash_file(file)?) {
                return Err(format!("同包同版本内容不同，不能当作重复副本删除: {id}"));
            }
        }
    }
    Ok(true)
}

pub fn reconcile(db: &Database, data: &Path, ctx: &Context) -> Result<()> {
    let _guard = files::maintenance_lock()?;
    for mut batch in list_batches(data, ctx)? {
        if batch.status != "processing" {
            continue;
        }
        batch
            .errors
            .push("上次清理意外中断；已核对文件，请恢复该批次或检查错误明细".into());
        for file in &mut batch.files {
            let source = Path::new(&file.original.path);
            let trash = Path::new(&file.trash_path);
            if source.exists() && !trash.exists() && verify_hash(source, &file.hash).is_ok() {
                file.status = "restored".into();
            } else if trash.exists() && !source.exists() && verify_hash(trash, &file.hash).is_ok() {
                file.status = "trashed".into();
            } else {
                batch
                    .errors
                    .push(format!("{} 的文件位置或内容需人工检查", file.package_id));
            }
        }
        if let Err(e) = delete_index(db, &batch) {
            batch.errors.push(e);
        }
        if let Err(e) = restore_index(db, &batch, true) {
            batch.errors.push(e);
        }
        batch.status = "needs_repair".into();
        write_batch(data, &batch)?;
    }
    Ok(())
}
pub fn auto_purge(db: &Database, data: &Path, ctx: &Context) -> Result<usize> {
    if !policy(db)?.auto_purge {
        return Ok(0);
    }
    let mut count = 0;
    for batch in list_batches(data, ctx)? {
        if batch.status == "completed"
            && chrono::DateTime::parse_from_rfc3339(&batch.expires_at).map_err(|e| e.to_string())?
                < chrono::Utc::now()
        {
            purge(data, ctx, &batch.id)?;
            count += 1;
        }
    }
    Ok(count)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::AtomicU64;
    static NEXT: AtomicU64 = AtomicU64::new(0);
    #[test]
    fn recovery_metadata_queries_cross_chunks_and_preserve_unicode_aliases() {
        let f = Fixture::new();
        let ids: Vec<_> = (0..405).map(|i| format!("Äuthor.Scene{i}.1")).collect();
        f.db.with_conn(|c| {
            let tx = c.unchecked_transaction()?;
            for id in &ids {
                let path = f.dir.join("AddonPackages").join(format!("{id}.var"));
                tx.execute("INSERT INTO packages(id,creator,name,version,file_path,size_bytes,resource_types,scan_time) VALUES(?1,'Äuthor','Scene',1,?2,1,'[]','preserved')", params![id,path.to_string_lossy()])?;
                tx.execute("INSERT INTO contents(package_id,file_path,resource_type,size_bytes) VALUES(?1,'Custom/kept.bin','other',7)",[id])?;
                tx.execute("INSERT INTO package_tags(package_id,tag) VALUES(?1,'kept')",[id])?;
            }
            tx.commit()?; Ok(())
        }).unwrap();
        let items: Vec<_> = ids.iter().map(|id| PreviewItem { package_id: id.clone(), file_count: 1, size_bytes: 1,
            content_count: 1, attached: false, reasons: vec![], issue: None, invalid_name: false }).collect();
        let metadata = item_metadata(&f.db, &f.ctx, &items).unwrap();
        assert_eq!(metadata.len(), ids.len());
        for id in &ids {
            let meta = &metadata[id];
            assert_eq!(meta.tags, ["kept"]);
            let snapshot = meta.index_snapshot.as_ref().unwrap();
            assert_eq!(snapshot.id, *id);
            assert_eq!(snapshot.scan_time, "preserved");
            assert_eq!(snapshot.contents, [("Custom/kept.bin".into(),7)]);
        }
    }
    #[test]
    fn purge_skips_unchanged_verified_bytes_and_rejects_changed_files() {
        let f = Fixture::new();
        f.package("A.Scene.1", &[]);
        let batch = f.run(f.plan(&["A.Scene.1"], false));
        let path = Path::new(&batch.files[0].trash_path);
        let stamped = change_stamp(path).unwrap().is_some();
        let trusted = batch.files[0].verified_stamp.is_some();
        let size = fs::metadata(path).unwrap().len();
        files::HASHED_BYTES.with(|bytes| bytes.set(0));
        purge(&f.dir.join("data"), &f.ctx, &batch.id).unwrap();
        assert_eq!(files::HASHED_BYTES.with(|bytes| bytes.get()), if trusted { 0 } else { size * if stamped { 1 } else { 2 } });

        f.package("B.Scene.1", &[]);
        let batch = f.run(f.plan(&["B.Scene.1"], false));
        let path = Path::new(&batch.files[0].trash_path);
        let saved = fs::read(path).unwrap();
        let modified = fs::metadata(path).unwrap().modified().unwrap();
        let mut changed = saved.clone(); changed[0] ^= 1;
        fs::write(path, &changed).unwrap();
        fs::File::options().write(true).open(path).unwrap().set_times(fs::FileTimes::new().set_modified(modified)).unwrap();
        assert!(purge(&f.dir.join("data"), &f.ctx, &batch.id).is_err());
        assert_eq!(fs::read(path).unwrap(), changed);
        fs::write(path, saved).unwrap();
        assert_eq!(purge(&f.dir.join("data"), &f.ctx, &batch.id).unwrap().status, "purged");
    }
    #[test]
    fn purge_preserves_durable_intent_until_all_file_results_are_saved() {
        let f = Fixture::new();
        f.package("Intent.A.1", &[]); f.package("Intent.B.1", &[]);
        let batch = f.run(f.plan(&["Intent.A.1","Intent.B.1"], false));
        let mut observed = 0;
        let result = purge_with_progress(&f.dir.join("data"), &f.ctx, &batch.id, &mut |stage,n,_| {
            if stage != "purging" { return; }
            let journal = load_batch(&f.dir.join("data"), &f.ctx, &batch.id).unwrap();
            assert_eq!(journal.status, "purging");
            assert_eq!(journal.files[n-1].status, "purging");
            assert!(!Path::new(&journal.files[n-1].trash_path).exists());
            observed += 1;
        }).unwrap();
        assert_eq!(observed, 2);
        assert_eq!(result.status, "purged");
        let persisted = load_batch(&f.dir.join("data"), &f.ctx, &batch.id).unwrap();
        assert_eq!(persisted.status, "purged");
        assert!(persisted.files.iter().all(|f| f.status == "purged"));
    }
    #[test]
    fn replacement_files_require_a_checksum_even_with_the_original_times() {
        let f = Fixture::new();
        f.package("Replace.Scene.1", &[]);
        let batch = f.run(f.plan(&["Replace.Scene.1"], false));
        let path = Path::new(&batch.files[0].trash_path);
        let bytes = fs::read(path).unwrap();
        let modified = fs::metadata(path).unwrap().modified().unwrap();
        let replacement = f.dir.join("replacement.var");
        fs::write(&replacement, &bytes).unwrap();
        fs::File::options().write(true).open(&replacement).unwrap().set_times(fs::FileTimes::new().set_modified(modified)).unwrap();
        fs::remove_file(path).unwrap(); fs::rename(&replacement,path).unwrap();
        files::HASHED_BYTES.with(|bytes| bytes.set(0));
        assert_eq!(purge(&f.dir.join("data"), &f.ctx, &batch.id).unwrap().status, "purged");
        assert!(files::HASHED_BYTES.with(|bytes| bytes.get()) >= bytes.len() as u64);
    }
    #[test]
    fn old_journals_keep_single_checksum_verification_and_byte_progress() {
        let f = Fixture::new();
        f.package("Legacy.Scene.1", &[]);
        let batch = f.run(f.plan(&["Legacy.Scene.1"], false));
        let mut json = serde_json::to_value(&batch).unwrap();
        for file in json["files"].as_array_mut().unwrap() { file.as_object_mut().unwrap().remove("verified_stamp"); }
        let legacy: Batch = serde_json::from_value(json).unwrap();
        assert!(legacy.files[0].verified_stamp.is_none());
        write_batch(&f.dir.join("data"), &legacy).unwrap();
        let stamped = change_stamp(Path::new(&batch.files[0].trash_path)).unwrap().is_some();
        files::HASHED_BYTES.with(|bytes| bytes.set(0));
        let mut events = vec![];
        purge_with_progress(&f.dir.join("data"), &f.ctx, &batch.id, &mut |stage,n,total| events.push((stage.to_string(),n,total))).unwrap();
        let reads = if stamped { 1 } else { 2 };
        assert_eq!(files::HASHED_BYTES.with(|bytes| bytes.get()), batch.files[0].original.size * reads);
        assert!(events.iter().any(|(stage,n,total)| stage == "purging_bytes" && n == total && *total > 0));
        assert!(events.iter().any(|(stage,n,total)| stage == "purging" && *n == 1 && *total == 1));
    }
    #[test]
    fn streaming_verification_can_cancel_before_any_file_moves() {
        let f = Fixture::new();
        let mut seed = 17u32;
        let payload: Vec<_> = (0..3 * 1024 * 1024).map(|_| { seed ^= seed << 13; seed ^= seed >> 17; seed ^= seed << 5; seed as u8 }).collect();
        let path = f.raw_package("A.Large.1", &[("meta.json",b"{}"),("Custom/data.bin",&payload)]);
        f.db.with_conn(|c| files::index_package(c, &path)).unwrap();
        let plan = f.plan(&["A.Large.1"], false);
        let cancel = AtomicBool::new(false);
        let mut bytes_reported = 0;
        let result = execute_with_progress(&f.db, &f.dir.join("data"), plan, false, &cancel, &mut |stage,n,_| {
            if stage == "verifying_bytes" && n > 0 { bytes_reported = n; cancel.store(true, Ordering::Relaxed); }
        });
        assert!(result.is_err());
        assert!(bytes_reported > 0);
        assert!(path.exists());
        assert_eq!(f.count(), 1);
        assert!(list_batches(&f.dir.join("data"), &f.ctx).unwrap().is_empty());
    }
    #[test]
    fn purge_without_reliable_change_stamps_retains_hash_fallback() {
        use crate::services::cleanup_reference_cache::FORCE_SIGNATURE_CHECK;
        let f = Fixture::new();
        f.package("A.Scene.1", &[]);
        let batch = f.run(f.plan(&["A.Scene.1"], false));
        files::HASHED_BYTES.with(|bytes| bytes.set(0));
        FORCE_SIGNATURE_CHECK.with(|forced| forced.set(true));
        let result = purge(&f.dir.join("data"), &f.ctx, &batch.id);
        FORCE_SIGNATURE_CHECK.with(|forced| forced.set(false));
        assert_eq!(result.unwrap().status, "purged");
        assert_eq!(files::HASHED_BYTES.with(|bytes| bytes.get()), batch.files[0].original.size * 2);
    }
    #[test]
    fn batch_journals_are_checkpointed_and_display_does_not_discard_recovery_data() {
        let f = Fixture::new();
        let names: Vec<_> = (0..JOURNAL_CHUNK + 1).map(|i| format!("A.Scene{i}.1")).collect();
        for name in &names { f.package(name, &[]); }
        let plan = f.plan(&names.iter().map(String::as_str).collect::<Vec<_>>(), false);
        JOURNAL_IO.with(|io| io.set((0, 0)));
        let batch = f.run(plan);
        assert_eq!(JOURNAL_IO.with(|io| io.get().0), 3);
        assert!(batch.clone().for_display().metadata.is_empty());
        assert_eq!(load_batch(&f.dir.join("data"), &f.ctx, &batch.id).unwrap().metadata.len(), names.len());
        JOURNAL_IO.with(|io| io.set((0, 0)));
        assert_eq!(purge(&f.dir.join("data"), &f.ctx, &batch.id).unwrap().status, "purged");
        assert_eq!(JOURNAL_IO.with(|io| io.get().0), 4);
    }
    #[test]
    #[ignore = "isolated deletion benchmark: creates temporary archives and database only"]
    fn cleanup_delete_benchmark() {
        let f = Fixture::new();
        let mut seed = 123456789u32;
        let payload: Vec<u8> = (0..4 * 1024 * 1024).map(|_| {
            seed ^= seed << 13; seed ^= seed >> 17; seed ^= seed << 5; seed as u8
        }).collect();
        let ids: Vec<_> = (0..12).map(|i| format!("Bench.Package{i}.1")).collect();
        for id in &ids {
            let path = f.raw_package(id, &[("meta.json", b"{}"), ("Custom/audio.bin", &payload)]);
            f.db.with_conn(|c| files::index_package(c, &path)).unwrap();
        }
        f.package("Bench.Unrelated.1", &[]);
        f.db.with_conn(|c| {
            let tx = c.unchecked_transaction()?;
            { let mut insert = tx.prepare("INSERT INTO contents(package_id,file_path,resource_type,size_bytes) VALUES(?1,?2,'other',1)")?;
              for id in &ids { for i in 0..1000 { insert.execute(params![id,format!("Custom/entry{i}.bin")])?; } }
              for i in 0..500_000 { insert.execute(params!["Bench.Unrelated.1",format!("Custom/unrelated{i}.bin")])?; }
            }
            tx.commit()?; Ok(())
        }).unwrap();
        let selected: Vec<_> = ids.iter().map(String::as_str).collect();
        drop(f.plan(&selected, false));
        let start = std::time::Instant::now();
        let plan = f.plan(&selected, false);
        let preview_ms = start.elapsed().as_secs_f64() * 1000.0;
        files::HASHED_BYTES.with(|bytes| bytes.set(0));
        JOURNAL_IO.with(|io| io.set((0, 0)));
        let start = std::time::Instant::now();
        let batch = f.run(plan);
        assert_eq!(batch.status, "completed");
        eprintln!("BENCH preview_ms={preview_ms:.1} execute_ms={:.1} execute_hash_bytes={} execute_journal={:?}", start.elapsed().as_secs_f64()*1000.0, files::HASHED_BYTES.with(|b| b.get()), JOURNAL_IO.with(|io| io.get()));
        files::HASHED_BYTES.with(|bytes| bytes.set(0));
        JOURNAL_IO.with(|io| io.set((0, 0)));
        let start = std::time::Instant::now();
        assert_eq!(purge(&f.dir.join("data"), &f.ctx, &batch.id).unwrap().status, "purged");
        eprintln!("BENCH purge_ms={:.1} purge_hash_bytes={} purge_journal={:?}", start.elapsed().as_secs_f64()*1000.0, files::HASHED_BYTES.with(|b| b.get()), JOURNAL_IO.with(|io| io.get()));
    }
    struct Fixture {
        dir: PathBuf,
        db: Database,
        ctx: Context,
        _cleanup: CleanupDir,
    }
    struct CleanupDir(PathBuf);
    impl Drop for CleanupDir {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }
    impl Fixture {
        fn new() -> Self {
            let dir = std::env::temp_dir().join(format!(
                "vam-cleanup-{}-{}-{}",
                std::process::id(),
                chrono::Utc::now().timestamp_nanos_opt().unwrap_or_default(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            ));
            fs::create_dir_all(dir.join("AddonPackages")).unwrap();
            // Seed paths and database records with the same canonical root;
            // Windows CI can expose TEMP through a short (8.3) alias.
            let dir = files::canonical_dir(&dir).unwrap();
            let root = dir.to_string_lossy().into_owned();
            let db = Database::new(&dir.join("data/index.db")).unwrap();
            let ctx = Context {
                library_key: files::path_key(&root),
                vam_root: root.clone(),
                roots: vec![Path::new(&root)
                    .join("AddonPackages")
                    .to_string_lossy()
                    .into_owned()],
                managed: false,
                shared: false,
            };
            Self {
                _cleanup: CleanupDir(dir.clone()),
                dir,
                db,
                ctx,
            }
        }
        fn write(&self, path: &str, bytes: &[u8]) -> PathBuf {
            let path = self.dir.join(path);
            fs::create_dir_all(path.parent().unwrap()).unwrap();
            fs::write(&path, bytes).unwrap();
            path
        }
        fn package_at(
            &self,
            relative: &str,
            id: &str,
            deps: &[&str],
            content: serde_json::Value,
        ) -> PathBuf {
            let path = self
                .dir
                .join("AddonPackages")
                .join(relative)
                .join(format!("{id}.var"));
            fs::create_dir_all(path.parent().unwrap()).unwrap();
            let mut zip = zip::ZipWriter::new(fs::File::create(&path).unwrap());
            let opts = zip::write::SimpleFileOptions::default();
            zip.start_file("meta.json", opts).unwrap();
            let dependencies = deps
                .iter()
                .map(|id| ((*id).to_string(), serde_json::json!({})))
                .collect::<serde_json::Map<_, _>>();
            zip.write_all(
                serde_json::to_string(&serde_json::json!({"dependencies":dependencies}))
                    .unwrap()
                    .as_bytes(),
            )
            .unwrap();
            zip.start_file("Saves/scene/example.json", opts).unwrap();
            zip.write_all(serde_json::to_string(&content).unwrap().as_bytes())
                .unwrap();
            zip.finish().unwrap();
            self.db
                .with_conn(|c| files::index_package(c, &path))
                .unwrap();
            path
        }
        fn package(&self, id: &str, deps: &[&str]) -> PathBuf {
            self.package_at("", id, deps, serde_json::json!({}))
        }
        fn raw_package(&self, id: &str, entries: &[(&str, &[u8])]) -> PathBuf {
            let path = self.dir.join("AddonPackages").join(format!("{id}.var"));
            let mut zip = zip::ZipWriter::new(fs::File::create(&path).unwrap());
            for (name, body) in entries {
                zip.start_file(*name, zip::write::SimpleFileOptions::default())
                    .unwrap();
                zip.write_all(body).unwrap();
            }
            zip.finish().unwrap();
            path
        }
        fn plan(&self, ids: &[&str], deps: bool) -> Plan {
            make_plan(
                &self.db,
                self.ctx.clone(),
                format!("test-{}", NEXT.fetch_add(1, Ordering::Relaxed)),
                ids.iter().map(|s| s.to_string()).collect(),
                deps,
                vec![],
                &AtomicBool::new(false),
                &mut |_, _| {},
            )
            .unwrap()
        }
        fn run(&self, plan: Plan) -> Batch {
            execute(
                &self.db,
                &self.dir.join("data"),
                plan,
                &AtomicBool::new(false),
                &mut |_, _| {},
            )
            .unwrap()
        }
        fn count(&self) -> i64 {
            self.db
                .with_conn(|c| Ok(c.query_row("SELECT COUNT(*) FROM packages", [], |r| r.get(0))?))
                .unwrap()
        }
    }
    fn ids(items: &[PreviewItem]) -> BTreeSet<String> {
        items.iter().map(|i| i.package_id.to_lowercase()).collect()
    }

    #[test]
    fn shared_dependencies_are_computed_for_the_whole_selection() {
        let f = Fixture::new();
        f.package("A.Scene.1", &["C.Clothes.1"]);
        f.package("B.Scene.1", &["C.Clothes.1"]);
        f.package("C.Clothes.1", &["D.Texture.1"]);
        f.package("D.Texture.1", &[]);
        let one = f.plan(&["A.Scene.1"], true);
        assert_eq!(one.preview.items.len(), 1);
        assert_eq!(one.preview.retained.len(), 2);
        let both = f.plan(&["A.Scene.1", "B.Scene.1"], true);
        assert_eq!(both.preview.items.len(), 4);
        assert!(both.preview.blocked.is_empty());
        let dependency = f.plan(&["C.Clothes.1"], false);
        assert!(dependency.preview.items.is_empty());
        assert_eq!(dependency.preview.blocked.len(), 1);
    }
    #[test]
    fn local_presets_and_scene_json_protect_transitive_dependencies() {
        let f = Fixture::new();
        f.package("A.Scene.1", &["C.Clothes.1"]);
        f.package("C.Clothes.1", &["D.Texture.1"]);
        f.package("D.Texture.1", &[]);
        f.write(
            "Custom/Atom/Person/Appearance/Preset_copy.vap",
            br#"{"clothing":"C.Clothes.1:/Custom/a.vap"}"#,
        );
        let plan = f.plan(&["A.Scene.1"], true);
        assert_eq!(plan.preview.items.len(), 1);
        assert_eq!(plan.preview.retained.len(), 2);
        f.write(
            "Saves/scene/local.json",
            br#"{"asset":"A.Scene.1:/Saves/scene/example.json"}"#,
        );
        let plan = f.plan(&["A.Scene.1"], true);
        assert!(plan.preview.items.is_empty());
        assert_eq!(plan.preview.blocked[0].reasons[0].code, "local");
    }
    #[test]
    fn exact_and_latest_versions_remain_pinned() {
        let f = Fixture::new();
        f.package("A.Scene.1", &["B.Clothes.1", "B.Clothes.latest"]);
        f.package("B.Clothes.1", &[]);
        f.package("B.Clothes.2", &[]);
        f.package("B.Clothes.3", &[]);
        let plan = f.plan(&["B.Clothes.1", "B.Clothes.2", "B.Clothes.3"], false);
        assert_eq!(
            ids(&plan.preview.items),
            BTreeSet::from(["b.clothes.2".into()])
        );
        assert_eq!(plan.preview.blocked.len(), 2);
    }
    #[test]
    fn favorite_and_manual_protection_propagate_through_cycles() {
        let f = Fixture::new();
        f.package("A.Scene.1", &["B.Scene.1"]);
        f.package("B.Scene.1", &["A.Scene.1"]);
        assert_eq!(f.plan(&["A.Scene.1"], true).preview.items.len(), 2);
        f.write(
            "AddonPackagesFilePrefs/A.Scene.1/Saves/scene/example.json.fav",
            b"",
        );
        assert!(f
            .plan(&["A.Scene.1", "B.Scene.1"], true)
            .preview
            .items
            .is_empty());
        set_marks(&f.db, &f.ctx, &["B.Scene.1".into()], "protect").unwrap();
        let mark = marks(&f.db, &f.ctx).unwrap();
        assert!(mark[0].protected);
        assert!(!mark[0].pending);
    }
    #[test]
    fn unreadable_archives_are_problems_but_broken_local_json_blocks_whole_package_deletion() {
        let f = Fixture::new();
        f.package("A.Scene.1", &[]);
        // Neither VaM nor the checker can open it, so it cannot use anything.
        f.write("AddonPackages/Bad.File.1.var", b"not a zip");
        let plan = f.plan(&["A.Scene.1"], false);
        assert_eq!(plan.preview.items.len(), 1);
        assert!(plan.preview.warnings.is_empty());
        fs::remove_file(f.dir.join("AddonPackages/Bad.File.1.var")).unwrap();
        // A local file may be mid-save; its references stay unknown.
        f.write("Custom/broken.vap", b"{");
        assert!(f.plan(&["A.Scene.1"], false).preview.items.is_empty());
    }
    #[test]
    fn marks_and_plans_changed_after_preview_are_rejected_without_moves() {
        let f = Fixture::new();
        let path = f.package("A.Scene.1", &[]);
        let plan = f.plan(&["A.Scene.1"], false);
        set_marks(&f.db, &f.ctx, &["A.Scene.1".into()], "protect").unwrap();
        assert!(execute(
            &f.db,
            &f.dir.join("data"),
            plan,
            &AtomicBool::new(false),
            &mut |_, _| {}
        )
        .is_err());
        assert!(path.exists());
        set_marks(&f.db, &f.ctx, &["A.Scene.1".into()], "unprotect").unwrap();
        let plan = f.plan(&["A.Scene.1"], false);
        f.db.with_conn(|c|{c.execute("INSERT INTO on_demand_plans(id,name,package_ids) VALUES('plan','Keep','[\"A.Scene.1\"]')",[])?;Ok(())}).unwrap();
        assert!(execute(
            &f.db,
            &f.dir.join("data"),
            plan,
            &AtomicBool::new(false),
            &mut |_, _| {}
        )
        .is_err());
        assert!(path.exists());
    }
    #[test]
    fn new_consumers_and_modified_selected_files_invalidate_preview() {
        let f = Fixture::new();
        let path = f.package("A.Scene.1", &[]);
        let plan = f.plan(&["A.Scene.1"], false);
        f.write("Saves/scene/new.json", br#"{"ref":"A.Scene.1:/file"}"#);
        assert!(execute(
            &f.db,
            &f.dir.join("data"),
            plan,
            &AtomicBool::new(false),
            &mut |_, _| {}
        )
        .is_err());
        assert!(path.exists());
        fs::remove_file(f.dir.join("Saves/scene/new.json")).unwrap();
        let plan = f.plan(&["A.Scene.1"], false);
        fs::write(&path, b"changed").unwrap();
        assert!(execute(
            &f.db,
            &f.dir.join("data"),
            plan,
            &AtomicBool::new(false),
            &mut |_, _| {}
        )
        .is_err());
        assert!(path.exists());
    }
    #[test]
    fn batch_round_trip_restores_all_copies_tags_and_import_time() {
        let f = Fixture::new();
        let a = f.package("A.Scene.1", &[]);
        let b = f.package_at("copy", "A.Scene.1", &[], serde_json::json!({}));
        f.db.with_conn(|c| {
            c.execute(
                "INSERT INTO package_tags(package_id,tag) VALUES('A.Scene.1','keep-me')",
                [],
            )?;
            c.execute("UPDATE packages SET scan_time='old-import'", [])?;
            Ok(())
        })
        .unwrap();
        let plan = f.plan(&["A.Scene.1"], false);
        assert_eq!(plan.preview.total_files, 2);
        let hash = files::hash_file(&files::fingerprint(&a).unwrap()).unwrap();
        let batch = f.run(plan);
        assert_eq!(batch.status, "completed");
        assert!(!a.exists() && !b.exists());
        assert_eq!(f.count(), 0);
        let restored = restore(&f.db, &f.dir.join("data"), &f.ctx, &batch.id).unwrap();
        assert_eq!(restored.status, "restored");
        assert!(a.exists() && b.exists());
        assert_eq!(f.count(), 1);
        assert_eq!(
            files::hash_file(&files::fingerprint(&a).unwrap()).unwrap(),
            hash
        );
        f.db.with_conn(|c| {
            assert_eq!(
                c.query_row("SELECT tag FROM package_tags", [], |r| r
                    .get::<_, String>(0))?,
                "keep-me"
            );
            assert_eq!(
                c.query_row("SELECT scan_time FROM packages", [], |r| r
                    .get::<_, String>(0))?,
                "old-import"
            );
            Ok(())
        })
        .unwrap();
        assert!(crate::services::scanner::scan_addon_packages(&f.dir, None)
            .unwrap()
            .1
            .current_file_paths
            .iter()
            .all(|p| !p.contains(".trash")));
    }
    #[test]
    fn failed_index_transaction_rolls_files_back_and_preserves_mark() {
        let f = Fixture::new();
        let path = f.package("A.Scene.1", &[]);
        set_marks(&f.db, &f.ctx, &["A.Scene.1".into()], "pending").unwrap();
        let plan = f.plan(&["A.Scene.1"], false);
        f.db.with_conn(|c|{c.execute_batch("CREATE TRIGGER fail_delete BEFORE DELETE ON packages BEGIN SELECT RAISE(ABORT,'injected failure'); END;")?;Ok(())}).unwrap();
        let batch = f.run(plan);
        assert_eq!(batch.status, "rolled_back");
        assert!(path.exists());
        assert_eq!(f.count(), 1);
        assert!(marks(&f.db, &f.ctx).unwrap()[0].pending);
    }
    #[test]
    fn cancellation_during_move_rolls_back_the_entire_batch() {
        let f = Fixture::new();
        let a = f.package("A.Scene.1", &[]);
        let b = f.package("B.Scene.1", &[]);
        let plan = f.plan(&["A.Scene.1", "B.Scene.1"], false);
        let cancel = AtomicBool::new(false);
        let batch = execute(&f.db, &f.dir.join("data"), plan, &cancel, &mut |n, t| {
            if n == 1 && t == 2 {
                cancel.store(true, Ordering::Relaxed);
            }
        })
        .unwrap();
        assert_eq!(batch.status, "rolled_back");
        assert!(a.exists() && b.exists());
        assert_eq!(f.count(), 2);
    }
    #[test]
    fn restore_conflicts_never_overwrite_and_recover_after_conflict_is_resolved() {
        let f = Fixture::new();
        let path = f.package("A.Scene.1", &[]);
        let batch = f.run(f.plan(&["A.Scene.1"], false));
        fs::write(&path, b"new user content").unwrap();
        let result = restore(&f.db, &f.dir.join("data"), &f.ctx, &batch.id).unwrap();
        assert_eq!(result.status, "needs_repair");
        assert_eq!(fs::read(&path).unwrap(), b"new user content");
        assert!(Path::new(&batch.files[0].trash_path).exists());
        fs::remove_file(&path).unwrap();
        let result = restore(&f.db, &f.dir.join("data"), &f.ctx, &batch.id).unwrap();
        assert_eq!(result.status, "restored");
        assert_eq!(f.count(), 1);
    }
    #[test]
    fn interrupted_journal_is_reconciled_and_remains_restorable() {
        let f = Fixture::new();
        let path = f.package("A.Scene.1", &[]);
        let mut plan = f.plan(&["A.Scene.1"], false);
        // Execution hashes every file before the journal is first written.
        for file in &mut plan.files {
            file.hash = files::hash_file(&file.original).unwrap();
        }
        let batch = Batch {
            id: plan.preview.plan_id,
            context: f.ctx.clone(),
            created_at: chrono::Utc::now().to_rfc3339(),
            expires_at: chrono::Utc::now().to_rfc3339(),
            status: "processing".into(),
            files: plan.files,
            metadata: plan.metadata,
            errors: vec![],
        };
        write_batch(&f.dir.join("data"), &batch).unwrap();
        files::transfer(&path, Path::new(&batch.files[0].trash_path), "move").unwrap();
        reconcile(&f.db, &f.dir.join("data"), &f.ctx).unwrap();
        let saved = list_batches(&f.dir.join("data"), &f.ctx).unwrap();
        assert_eq!(saved[0].status, "needs_repair");
        assert_eq!(f.count(), 0);
        assert_eq!(
            restore(&f.db, &f.dir.join("data"), &f.ctx, &batch.id)
                .unwrap()
                .status,
            "restored"
        );
        assert!(path.exists());
        assert_eq!(f.count(), 1);
    }
    #[test]
    fn instance_state_and_manifest_paths_cannot_cross_libraries() {
        let a = Fixture::new();
        let b = Fixture::new();
        a.package("A.Scene.1", &[]);
        b.package("A.Scene.1", &[]);
        set_marks(&a.db, &a.ctx, &["A.Scene.1".into()], "protect").unwrap();
        assert!(marks(&a.db, &b.ctx).unwrap().is_empty());
        set_marks(&a.db, &a.ctx, &["A.Scene.1".into()], "unprotect").unwrap();
        let batch = a.run(a.plan(&["A.Scene.1"], false));
        assert!(validate_batch(&batch, &b.ctx).is_err());
        assert!(list_batches(&a.dir.join("data"), &b.ctx)
            .unwrap()
            .is_empty());
        let mut injected = batch;
        injected.files[0].original.path = b
            .dir
            .join("AddonPackages/A.Scene.1.var")
            .to_string_lossy()
            .into_owned();
        assert!(validate_batch(&injected, &a.ctx).is_err());
        assert!(restore(&a.db, &a.dir.join("data"), &a.ctx, "../outside").is_err());
    }
    #[test]
    fn automatic_purge_defaults_off_and_only_removes_expired_completed_batches() {
        let f = Fixture::new();
        f.package("A.Scene.1", &[]);
        let mut batch = f.run(f.plan(&["A.Scene.1"], false));
        batch.expires_at = (chrono::Utc::now() - chrono::Duration::days(1)).to_rfc3339();
        write_batch(&f.dir.join("data"), &batch).unwrap();
        assert_eq!(auto_purge(&f.db, &f.dir.join("data"), &f.ctx).unwrap(), 0);
        assert!(Path::new(&batch.files[0].trash_path).exists());
        save_policy(&f.db, 30, true).unwrap();
        batch.status = "needs_repair".into();
        write_batch(&f.dir.join("data"), &batch).unwrap();
        assert_eq!(auto_purge(&f.db, &f.dir.join("data"), &f.ctx).unwrap(), 0);
        batch.status = "completed".into();
        write_batch(&f.dir.join("data"), &batch).unwrap();
        assert_eq!(auto_purge(&f.db, &f.dir.join("data"), &f.ctx).unwrap(), 1);
        assert!(!Path::new(&batch.files[0].trash_path).exists());
    }
    #[test]
    fn legacy_dedup_checks_shared_dependencies_and_content_equality() {
        let f = Fixture::new();
        f.package("A.Scene.1", &["B.Clothes.1"]);
        let b = f.package("B.Clothes.1", &[]);
        assert!(validate_deletions(&f.db, &f.ctx, &[b.to_string_lossy().into_owned()]).is_err());
        let copy = f.package_at(
            "copy",
            "B.Clothes.1",
            &[],
            serde_json::json!({"different":true}),
        );
        assert!(validate_deletions(&f.db, &f.ctx, &[copy.to_string_lossy().into_owned()]).is_err());
        fs::copy(&b, &copy).unwrap();
        validate_deletions(&f.db, &f.ctx, &[copy.to_string_lossy().into_owned()]).unwrap();
    }

    #[test]
    fn identical_copy_validation_does_not_scan_references_or_wait_for_the_database() {
        let f = Fixture::new();
        let source = f.package("A.Asset.1", &[]);
        let copy = f.write(
            "AddonPackages/copy/A.Asset.1.var",
            &fs::read(source).unwrap(),
        );
        f.write(
            "AddonPackages/Unrelated.Broken.1.var",
            b"unreadable unrelated archive",
        );
        let paths = vec![copy.to_string_lossy().into_owned()];
        let (sender, receiver) = std::sync::mpsc::channel();
        std::thread::scope(|scope| {
            let result =
                f.db.with_conn(|_| {
                    scope.spawn(|| {
                        let _ = sender.send(validate_deletions(&f.db, &f.ctx, &paths));
                    });
                    // The file proof must finish while the database remains busy.
                    Ok(receiver.recv_timeout(std::time::Duration::from_secs(5)))
                })
                .unwrap();
            result
                .expect("copy validation waited for the reference catalog")
                .unwrap();
        });
        assert!(copy.is_file());
    }

    #[test]
    fn identical_copy_validation_checks_all_selected_paths_and_preserves_the_last_copy() {
        let f = Fixture::new();
        let source = f.package("A.Asset.1", &[]);
        let copy = f.write(
            "AddonPackages/copy/A.Asset.1.var",
            &fs::read(&source).unwrap(),
        );
        let selected = vec![copy.to_string_lossy().into_owned()];
        assert!(preserves_package_copies(&f.ctx, &selected).unwrap());
        assert!(!preserves_package_copies(
            &f.ctx,
            &[selected[0].clone(), source.to_string_lossy().into_owned()]
        )
        .unwrap());
        let changed = f.write(
            "AddonPackages/changed/A.Asset.1.var",
            b"different size and contents",
        );
        assert!(
            validate_deletions(&f.db, &f.ctx, &[changed.to_string_lossy().into_owned()])
                .unwrap_err()
                .contains("同包同版本内容不同")
        );
        assert!(preserves_package_copies(
            &f.ctx,
            &[f.dir.join("outside.var").to_string_lossy().into_owned()]
        )
        .is_err());
        assert!(preserves_package_copies(
            &f.ctx,
            &[f.dir
                .join("AddonPackages/missing.var")
                .to_string_lossy()
                .into_owned()]
        )
        .is_err());
        let internal = f.write(
            "AddonPackages/.trash/A.Asset.1.var",
            &fs::read(source).unwrap(),
        );
        assert!(
            preserves_package_copies(&f.ctx, &[internal.to_string_lossy().into_owned()]).is_err()
        );
    }

    #[test]
    #[ignore = "requires VAM_DEDUP_FIXTURE_ROOT with the reported copies; validates the real library without writing to it"]
    fn reported_duplicate_validation_real_library_is_read_only() {
        let root =
            files::canonical_dir(Path::new(&std::env::var("VAM_DEDUP_FIXTURE_ROOT").unwrap()))
                .unwrap();
        let f = Fixture::new();
        let ctx = Context {
            library_key: files::path_key(&root.to_string_lossy()),
            vam_root: root.parent().unwrap().to_string_lossy().into_owned(),
            roots: vec![root.to_string_lossy().into_owned()],
            managed: false,
            shared: false,
        };
        let snapshots: Vec<_> = ["场景", "其他"]
            .into_iter()
            .map(|folder| {
                files::fingerprint(&root.join(folder).join("Amaimon.Bhaviour.2.var")).unwrap()
            })
            .collect();
        let hashes: Vec<_> = snapshots
            .iter()
            .map(|file| files::hash_file(file).unwrap())
            .collect();
        assert_eq!(hashes[0], hashes[1]);
        let started = std::time::Instant::now();
        validate_deletions(&f.db, &ctx, &[snapshots[1].path.clone()]).unwrap();
        eprintln!("Real-library duplicate validation: {:?}", started.elapsed());
        for (file, hash) in snapshots.iter().zip(&hashes) {
            files::verify(file).unwrap();
            assert_eq!(files::hash_file(file).unwrap(), *hash);
        }
    }
    #[test]
    fn excluding_an_attached_package_preserves_its_dependency_closure() {
        let f = Fixture::new();
        f.package("A.Scene.1", &["B.Clothes.1"]);
        f.package("B.Clothes.1", &["C.Texture.1"]);
        f.package("C.Texture.1", &[]);
        let plan = make_plan(
            &f.db,
            f.ctx.clone(),
            "excluded-test".into(),
            vec!["A.Scene.1".into()],
            true,
            vec!["B.Clothes.1".into()],
            &AtomicBool::new(false),
            &mut |_, _| {},
        )
        .unwrap();
        assert_eq!(plan.preview.items.len(), 1);
        assert_eq!(plan.preview.retained.len(), 2);
        assert_eq!(plan.preview.retained[0].reasons[0].code, "excluded");
    }
    #[test]
    fn archive_cache_checks_directory_crc_even_when_size_and_mtime_are_preserved() {
        let f = Fixture::new();
        let a = f.package("A.Scene.1", &[]);
        f.package("C.Scene.1", &[]);
        let b = f.package("B.Scene.1", &["C.Scene.1"]);
        assert_eq!(f.plan(&["A.Scene.1"], false).preview.items.len(), 1);
        let modified = fs::metadata(&b).unwrap().modified().unwrap();
        let size = fs::metadata(&b).unwrap().len();
        f.package("B.Scene.1", &["A.Scene.1"]);
        fs::OpenOptions::new()
            .write(true)
            .open(&b)
            .unwrap()
            .set_times(fs::FileTimes::new().set_modified(modified))
            .unwrap();
        assert_eq!(fs::metadata(&b).unwrap().len(), size);
        assert!(f.plan(&["A.Scene.1"], false).preview.items.is_empty());
        assert!(a.exists());
    }
    #[test]
    fn persisted_reference_index_reopens_without_reading_unchanged_archives() {
        let mut f = Fixture::new();
        f.package("A.Scene.1", &[]);
        f.package("B.Scene.1", &["C.Scene.1"]);
        f.package("C.Scene.1", &[]);
        assert_eq!(f.plan(&["A.Scene.1"], false).preview.items.len(), 1);
        drop(f.db);
        f.db = Database::new(&f.dir.join("data/index.db")).unwrap();
        REFERENCE_READS.with(|reads| reads.set(0));
        let mut events = vec![];
        let plan = make_plan_with_progress(
            &f.db,
            f.ctx.clone(),
            "reopened".into(),
            vec!["A.Scene.1".into()],
            false,
            vec![],
            &AtomicBool::new(false),
            &mut |stage, n, total| events.push((stage.to_string(), n, total)),
        )
        .unwrap();
        assert_eq!(plan.preview.items.len(), 1);
        REFERENCE_READS.with(|reads| assert_eq!(reads.get(), 0));
        // Change-stamp checks report progress; no archive is re-read.
        assert!(events.contains(&("checking_files".into(), 3, 3)));
        events.retain(|(stage, ..)| stage != "checking_files");
        assert_eq!(
            events,
            vec![("loading_index".into(), 0, 0), ("analyzing".into(), 0, 0)]
        );
    }
    fn serde_refs(bytes: &[u8]) -> std::result::Result<BTreeSet<String>, String> {
        let mut refs = References::default();
        stream_references(bytes, &mut refs, &AtomicBool::new(false)).map(|_| refs.candidates)
    }
    fn fast_refs(bytes: &[u8]) -> std::result::Result<BTreeSet<String>, String> {
        let mut refs = References::default();
        scan_references(
            bytes,
            bytes.len() as u64,
            &mut refs,
            &AtomicBool::new(false),
            false,
        )
        .map(|_| refs.candidates)
    }
    #[test]
    fn fast_reference_scan_matches_serde_on_generated_documents() {
        // Deterministic pseudo-random documents with references in values,
        // escaped and unicode strings, nested arrays/objects and scalars.
        let mut seed = 0x2545_f491_4f6c_dd1d_u64;
        let mut next = move |n: u64| {
            seed ^= seed << 13;
            seed ^= seed >> 7;
            seed ^= seed << 17;
            seed % n
        };
        let strings = [
            "Creator.Pkg.12:/Custom/a.vam",
            "creator.Pkg.latest:/x",
            "SELF:/Custom/Hair/x.vam",
            "Bad.Ref:/no-version",
            "A.B.C.7:/deep/path",
            "路径.包名.3:/中文.json",
            "plain text: with colon",
            "Author.Name.9",
            // JSON escapes (raw strings: the backslashes reach the document).
            r"\u0041uthor.Esc.4\u003a/escaped colon",
            r#"quote \" inside A.B.1:/not-first-colon"#,
            r"tab\tA.B.2:/x",
            r"Back\\slash.P.5:/x",
            r"Esc.Slash.6\/x:/y",
            "",
        ];
        let mut found = 0;
        for doc in 0..400 {
            let mut out = String::new();
            fn value(
                out: &mut String,
                depth: u32,
                next: &mut dyn FnMut(u64) -> u64,
                strings: &[&str],
            ) {
                match if depth > 4 { 3 + next(3) } else { next(6) } {
                    0 | 1 => {
                        out.push('{');
                        for k in 0..next(4) {
                            if k > 0 {
                                out.push(',');
                            }
                            out.push_str(&format!("\"k{}\":", next(50)));
                            value(out, depth + 1, next, strings);
                        }
                        out.push('}');
                    }
                    2 => {
                        out.push('[');
                        for k in 0..next(4) {
                            if k > 0 {
                                out.push(',');
                            }
                            value(out, depth + 1, next, strings);
                        }
                        out.push(']');
                    }
                    3 | 4 => out.push_str(&format!(
                        "\"{}\"",
                        strings[next(strings.len() as u64) as usize]
                    )),
                    _ => out.push_str(["true", "null", "-1.5e3", "42"][next(4) as usize]),
                }
            }
            value(&mut out, 0, &mut next, &strings);
            let bytes = out.as_bytes();
            let expected = serde_refs(bytes);
            found += expected.as_ref().map_or(0, BTreeSet::len);
            assert_eq!(fast_refs(bytes), expected, "document {doc}: {out}");
        }
        assert!(
            found > 100,
            "generator produced too few references: {found}"
        );
    }
    #[test]
    fn fast_reference_scan_keeps_blocking_corrupt_files() {
        for bad in [
            &b"{"[..],
            b"",
            b"   ",
            b"[1,2",
            b"{\"a\":\"A.B.1:/x}",
            b"{\"a\":\"\\",
            b"[\"\\u12\"]",
        ] {
            assert!(
                fast_refs(bad).is_err(),
                "{:?}",
                String::from_utf8_lossy(bad)
            );
        }
        assert!(fast_refs(b"\xef\xbb\xbf[\"A.B.1:/x\"]")
            .unwrap()
            .contains("a.b.1"));
        assert!(fast_refs(b"42").unwrap().is_empty());
        // VaM accepts trailing credits; inspect them rather than blocking.
        assert!(fast_refs(br#"{} Thanks:"A.B.1:/x""#)
            .unwrap()
            .contains("a.b.1"));
        assert!(fast_refs(b"{} garbage").unwrap().is_empty());
        // An open container still indicates truncation, even if a closing
        // bracket has the wrong type (the game ignores bracket types).
        assert!(fast_refs(b"{\"a\":[}").is_err());
        assert!(fast_refs(b"\"A.B.1:/x\" 1").unwrap().contains("a.b.1"));
        assert!(fast_refs(br#"{"asset":1"A.B.1:/x"}"#)
            .unwrap()
            .contains("1a.b.1"));
        // Dependency keys have no ':' marker; those files take the serde path.
        let with_deps = br#"{"dependencies":{"Dep.Pkg.3":{}},"x":"A.B.latest:/y"}"#;
        assert_eq!(fast_refs(with_deps), serde_refs(with_deps));
        assert!(fast_refs(with_deps).unwrap().contains("dep.pkg.3"));
    }
    #[test]
    fn previews_do_not_hash_and_execution_hashes_each_file_once() {
        let f = Fixture::new();
        let a = f.package("A.Scene.1", &[]);
        f.package("B.Scene.1", &[]);
        let plan = f.plan(&["A.Scene.1"], false);
        assert!(plan.files.iter().all(|file| file.hash.is_empty()));
        let expected = files::hash_file(&files::fingerprint(&a).unwrap()).unwrap();
        let mut stages = vec![];
        let batch = execute_with_progress(
            &f.db,
            &f.dir.join("data"),
            plan,
            false,
            &AtomicBool::new(false),
            &mut |stage, n, t| stages.push((stage.to_string(), n, t)),
        )
        .unwrap();
        assert_eq!(batch.status, "completed");
        assert_eq!(batch.files[0].hash, expected);
        assert!(stages.contains(&("verifying".into(), 1, 1)));
        assert!(stages.contains(&("moving".into(), 1, 1)));
        let verify_at = stages.iter().position(|s| s.0 == "verifying").unwrap();
        let move_at = stages.iter().position(|s| s.0 == "moving").unwrap();
        assert!(verify_at < move_at);
        assert_eq!(
            restore(&f.db, &f.dir.join("data"), &f.ctx, &batch.id)
                .unwrap()
                .status,
            "restored"
        );
        assert!(a.exists());
    }
    #[test]
    fn content_changed_after_preview_with_preserved_times_is_never_moved() {
        // Without change stamps the only evidence is the content itself.
        use super::super::cleanup_reference_cache::FORCE_SIGNATURE_CHECK;
        struct Reset;
        impl Drop for Reset {
            fn drop(&mut self) {
                FORCE_SIGNATURE_CHECK.with(|flag| flag.set(false));
            }
        }
        for force in [false, true] {
            FORCE_SIGNATURE_CHECK.with(|flag| flag.set(force));
            let _reset = Reset;
            let f = Fixture::new();
            let a = f.package("A.Scene.1", &[]);
            let plan = f.plan(&["A.Scene.1"], false);
            let before = files::fingerprint(&a).unwrap();
            let modified = fs::metadata(&a).unwrap().modified().unwrap();
            f.package_at("", "A.Scene.1", &["Z.Other.1"], serde_json::json!({}));
            fs::OpenOptions::new()
                .write(true)
                .open(&a)
                .unwrap()
                .set_times(fs::FileTimes::new().set_modified(modified))
                .unwrap();
            if files::fingerprint(&a).unwrap() != before {
                continue; // size changed; covered by fingerprint checks
            }
            assert!(execute(
                &f.db,
                &f.dir.join("data"),
                plan,
                &AtomicBool::new(false),
                &mut |_, _| {}
            )
            .is_err());
            assert!(a.exists(), "force={force}");
        }
    }
    fn quick_fixture() -> Fixture {
        let f = Fixture::new();
        f.package("A.Scene.1", &[]);
        // B uses A inside a scene without declaring it in meta.json.
        f.package_at(
            "",
            "B.Scene.1",
            &[],
            serde_json::json!({"atoms":[{"url":"A.Scene.1:/Custom/x.vam"}]}),
        );
        f.package("C.Scene.1", &[]);
        f.package("D.Scene.1", &["C.Scene.1"]);
        f
    }
    fn plan_in(f: &Fixture, quick: bool, ids: &[&str]) -> Plan {
        let cancel = AtomicBool::new(false);
        let cat = build_catalog(&f.db, &f.ctx, quick, &cancel, &mut |_, _, _| {}).unwrap();
        make_plan_from_catalog(
            &f.db,
            f.ctx.clone(),
            &cat,
            format!("mode-{}", NEXT.fetch_add(1, Ordering::Relaxed)),
            ids.iter().map(|s| s.to_string()).collect(),
            false,
            vec![],
            &cancel,
        )
        .unwrap()
    }
    #[test]
    fn quick_check_reads_only_declared_dependencies_and_executes_in_quick_mode() {
        let f = quick_fixture();
        let quick = plan_in(&f, true, &["A.Scene.1"]);
        assert!(quick.preview.quick);
        // B's undeclared use of A is not seen by a quick check of B.
        assert_eq!(
            ids(&quick.preview.items),
            BTreeSet::from(["a.scene.1".into()])
        );
        // Declared dependencies are still honoured in quick mode.
        let declared = plan_in(&f, true, &["C.Scene.1"]);
        assert!(declared.preview.items.is_empty());
        assert_eq!(declared.preview.blocked[0].reasons[0].code, "referenced");
        let batch = execute_with_progress(
            &f.db,
            &f.dir.join("data"),
            quick,
            true,
            &AtomicBool::new(false),
            &mut |_, _, _| {},
        )
        .unwrap();
        assert_eq!(batch.status, "completed");
        assert!(!f.dir.join("AddonPackages/A.Scene.1.var").exists());
    }
    #[test]
    fn quick_check_uses_full_index_entries_and_stale_quick_plans_are_rejected() {
        let f = quick_fixture();
        let early_quick = plan_in(&f, true, &["A.Scene.1"]);
        assert_eq!(early_quick.preview.items.len(), 1);
        let full = plan_in(&f, false, &["A.Scene.1"]);
        assert!(full.preview.items.is_empty() && !full.preview.quick);
        assert_ne!(full.revision, early_quick.revision);
        // Once B is fully indexed, quick checks use what the full index knows.
        REFERENCE_READS.with(|reads| reads.set(0));
        let quick = plan_in(&f, true, &["A.Scene.1"]);
        REFERENCE_READS.with(|reads| assert_eq!(reads.get(), 0));
        assert!(quick.preview.items.is_empty());
        assert_eq!(quick.preview.blocked[0].reasons[0].code, "referenced");
        // The quick plan made before that is now stale and moves nothing.
        assert!(execute(
            &f.db,
            &f.dir.join("data"),
            early_quick,
            &AtomicBool::new(false),
            &mut |_, _| {}
        )
        .is_err());
        assert!(f.dir.join("AddonPackages/A.Scene.1.var").exists());
    }
    #[test]
    fn quick_entries_never_satisfy_a_full_check() {
        let f = Fixture::new();
        f.package("A.Scene.1", &[]);
        f.package_at(
            "",
            "B.Scene.1",
            &[],
            serde_json::json!({"atoms":[{"url":"A.Scene.1:/Custom/x.vam"}]}),
        );
        let cancel = AtomicBool::new(false);
        REFERENCE_READS.with(|reads| reads.set(0));
        build_catalog(&f.db, &f.ctx, true, &cancel, &mut |_, _, _| {}).unwrap();
        REFERENCE_READS.with(|reads| assert_eq!(reads.get(), 2));
        // A later quick check reuses the quick entries.
        REFERENCE_READS.with(|reads| reads.set(0));
        build_catalog(&f.db, &f.ctx, true, &cancel, &mut |_, _, _| {}).unwrap();
        REFERENCE_READS.with(|reads| assert_eq!(reads.get(), 0));
        // A full check re-reads them and finds the undeclared reference.
        let mut events = vec![];
        let full = make_plan_with_progress(
            &f.db,
            f.ctx.clone(),
            "full-after-quick".into(),
            vec!["A.Scene.1".into()],
            false,
            vec![],
            &cancel,
            &mut |stage, n, t| events.push((stage.to_string(), n, t)),
        )
        .unwrap();
        assert!(full.preview.items.is_empty());
        assert!(events.contains(&("updating_index".into(), 2, 2)));
        // ... and the upgraded entries serve quick checks without new reads.
        REFERENCE_READS.with(|reads| reads.set(0));
        build_catalog(&f.db, &f.ctx, true, &cancel, &mut |_, _, _| {}).unwrap();
        REFERENCE_READS.with(|reads| assert_eq!(reads.get(), 0));
    }
    #[test]
    fn background_index_policy_defaults_on_and_migrates_old_tables() {
        let conn = rusqlite::Connection::open_in_memory().unwrap();
        conn.execute_batch(
            "CREATE TABLE cleanup_policy (id INTEGER PRIMARY KEY CHECK(id=1),
             retention_days INTEGER NOT NULL DEFAULT 30, auto_purge INTEGER NOT NULL DEFAULT 0);
             INSERT INTO cleanup_policy(id,retention_days,auto_purge) VALUES(1,45,1);",
        )
        .unwrap();
        init_schema(&conn).unwrap();
        init_schema(&conn).unwrap();
        let row: (i64, bool, bool) = conn
            .query_row(
                "SELECT retention_days,auto_purge,background_index FROM cleanup_policy",
                [],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
            )
            .unwrap();
        assert_eq!(row, (45, true, true));
        let f = Fixture::new();
        assert!(policy(&f.db).unwrap().background_index);
        save_policy_with_index(&f.db, 10, false, false).unwrap();
        save_policy(&f.db, 12, true).unwrap();
        let saved = policy(&f.db).unwrap();
        assert_eq!(
            (
                saved.retention_days,
                saved.auto_purge,
                saved.background_index
            ),
            (12, true, false)
        );
    }
    #[test]
    fn catalog_built_on_the_background_pool_matches_the_foreground_result() {
        struct Reset;
        impl Drop for Reset {
            fn drop(&mut self) {
                PARALLEL.with(|p| p.set(false));
            }
        }
        let f = quick_fixture();
        f.write("Saves/scene/local.json", br#"{"a":"C.Scene.1:/x"}"#);
        let cancel = AtomicBool::new(false);
        let summary = |cat: &Catalog| {
            (
                cat.revision.clone(),
                cat.edges.clone(),
                cat.warnings.clone(),
            )
        };
        PARALLEL.with(|p| p.set(true));
        let _reset = Reset;
        let pool = super::super::cleanup_indexer::background_pool().unwrap();
        let background = with_worker_pool(pool, || {
            build_catalog(&f.db, &f.ctx, false, &cancel, &mut |_, _, _| {}).unwrap()
        });
        PARALLEL.with(|p| p.set(false));
        let foreground = build_catalog(&f.db, &f.ctx, false, &cancel, &mut |_, _, _| {}).unwrap();
        assert_eq!(summary(&background), summary(&foreground));
        // The override is scoped to the closure.
        WORKER_POOL.with(|p| assert!(p.borrow().is_none()));
    }
    #[test]
    fn parallel_index_and_hashing_match_sequential_results() {
        struct Reset;
        impl Drop for Reset {
            fn drop(&mut self) {
                PARALLEL.with(|p| p.set(false));
            }
        }
        let summary = |plan: &Plan| {
            (
                ids(&plan.preview.items),
                plan.preview
                    .blocked
                    .iter()
                    .chain(&plan.preview.retained)
                    .map(|i| (i.package_id.to_lowercase(), i.reasons.clone()))
                    .collect::<Vec<_>>(),
                plan.files
                    .iter()
                    .map(|f| f.original.path.clone())
                    .collect::<Vec<_>>(),
                plan.revision.clone(),
                plan.preview.warnings.clone(),
            )
        };
        let f = Fixture::new();
        for i in 0..40 {
            let deps = if i % 3 == 0 {
                vec![format!("D.Shared.{}", i % 5 + 1)]
            } else {
                vec![]
            };
            let deps: Vec<&str> = deps.iter().map(String::as_str).collect();
            f.package(&format!("A.Scene.{i}"), &deps);
        }
        for v in 1..=5 {
            f.package(&format!("D.Shared.{v}"), &[]);
        }
        f.write("Saves/scene/local.json", br#"{"a":"D.Shared.2:/x"}"#);
        f.write("Custom/Atom/Person/Textures/skin.png", b"not json");
        f.write("AddonPackagesFilePrefs/A.Scene.7/x.fav", b"");
        f.write("AddonPackages/broken.var", b"not a package");
        f.write("Custom/broken.vap", b"{");
        let selection: Vec<String> = (0..40).map(|i| format!("A.Scene.{i}")).collect();
        let selection: Vec<&str> = selection.iter().map(String::as_str).collect();
        // Cold index, parallel.
        PARALLEL.with(|p| p.set(true));
        let _reset = Reset;
        let parallel = f.plan(&selection, true);
        PARALLEL.with(|p| p.set(false));
        // Warm index, sequential.
        let sequential = f.plan(&selection, true);
        assert_eq!(summary(&parallel), summary(&sequential));
        // Cold index again, sequential, for a fully independent comparison.
        f.db.with_conn(|c| {
            c.execute("DELETE FROM cleanup_reference_cache", [])?;
            Ok(())
        })
        .unwrap();
        assert_eq!(summary(&f.plan(&selection, true)), summary(&parallel));
        assert!(parallel
            .preview
            .warnings
            .iter()
            .any(|w| w.contains("broken.vap")));
        assert!(parallel
            .preview
            .invalid_files
            .iter()
            .any(|w| w.contains("broken.var")));
    }
    #[test]
    fn reused_catalog_plans_match_fresh_plans_and_stale_reuse_is_rejected() {
        let f = Fixture::new();
        f.package("A.Scene.1", &["B.Scene.1"]);
        f.package("B.Scene.1", &[]);
        f.package("C.Scene.1", &["B.Scene.1"]);
        let cancel = AtomicBool::new(false);
        let cat = build_catalog(&f.db, &f.ctx, false, &cancel, &mut |_, _, _| {}).unwrap();
        let plan = |id: &str, excluded: Vec<String>| {
            make_plan_from_catalog(
                &f.db,
                f.ctx.clone(),
                &cat,
                id.into(),
                vec!["A.Scene.1".into(), "C.Scene.1".into()],
                true,
                excluded,
                &cancel,
            )
            .unwrap()
        };
        let first = plan("reuse-1", vec![]);
        assert_eq!(
            ids(&first.preview.items),
            ids(&f.plan(&["A.Scene.1", "C.Scene.1"], true).preview.items)
        );
        REFERENCE_READS.with(|reads| reads.set(0));
        let kept = plan("reuse-2", vec!["c.scene.1".into()]);
        REFERENCE_READS.with(|reads| assert_eq!(reads.get(), 0));
        assert_eq!(
            ids(&kept.preview.items),
            BTreeSet::from(["a.scene.1".into()])
        );
        assert_eq!(kept.preview.retained[0].reasons[0].code, "referenced");
        // The library changes after the catalog was built: execution rejects it.
        f.package("D.Scene.1", &["A.Scene.1"]);
        let batch = execute(&f.db, &f.dir.join("data"), kept, &cancel, &mut |_, _| {});
        assert!(batch.is_err());
        assert!(f.dir.join("AddonPackages/A.Scene.1.var").exists());
    }
    #[test]
    fn preserved_size_and_mtime_only_reindexes_the_changed_consumer() {
        let f = Fixture::new();
        f.package("A.Scene.1", &[]);
        let b = f.package("B.Scene.1", &["C.Scene.1"]);
        f.package("C.Scene.1", &[]);
        let old_plan = f.plan(&["A.Scene.1"], false);
        let before = files::fingerprint(&b).unwrap();
        let modified = fs::metadata(&b).unwrap().modified().unwrap();
        f.package("B.Scene.1", &["A.Scene.1"]);
        fs::OpenOptions::new()
            .write(true)
            .open(&b)
            .unwrap()
            .set_times(fs::FileTimes::new().set_modified(modified))
            .unwrap();
        assert_eq!(files::fingerprint(&b).unwrap(), before);
        REFERENCE_READS.with(|reads| reads.set(0));
        assert!(f.plan(&["A.Scene.1"], false).preview.items.is_empty());
        REFERENCE_READS.with(|reads| assert_eq!(reads.get(), 1));
        assert!(execute(
            &f.db,
            &f.dir.join("data"),
            old_plan,
            &AtomicBool::new(false),
            &mut |_, _| {}
        )
        .is_err());
        assert!(f.dir.join("AddonPackages/A.Scene.1.var").exists());
    }
    #[test]
    fn local_reference_edits_and_removals_are_incremental_and_keep_protection_current() {
        let f = Fixture::new();
        f.package("A.Scene.1", &[]);
        f.package("C.Scene.1", &[]);
        let local = f.write("Saves/scene/local.json", br#"{"asset":"C.Scene.1:/asset"}"#);
        assert_eq!(f.plan(&["A.Scene.1"], false).preview.items.len(), 1);
        let modified = fs::metadata(&local).unwrap().modified().unwrap();
        let before = files::fingerprint(&local).unwrap();
        fs::write(&local, br#"{"asset":"A.Scene.1:/asset"}"#).unwrap();
        fs::OpenOptions::new()
            .write(true)
            .open(&local)
            .unwrap()
            .set_times(fs::FileTimes::new().set_modified(modified))
            .unwrap();
        assert_eq!(files::fingerprint(&local).unwrap(), before);
        REFERENCE_READS.with(|reads| reads.set(0));
        let plan = f.plan(&["A.Scene.1"], false);
        assert!(plan.preview.items.is_empty());
        assert_eq!(plan.preview.blocked[0].reasons[0].code, "local");
        REFERENCE_READS.with(|reads| assert_eq!(reads.get(), 1));
        fs::remove_file(local).unwrap();
        REFERENCE_READS.with(|reads| reads.set(0));
        assert_eq!(f.plan(&["A.Scene.1"], false).preview.items.len(), 1);
        REFERENCE_READS.with(|reads| assert_eq!(reads.get(), 0));
        assert_eq!(
            ReferenceCache::load(&f.db, &f.ctx.library_key)
                .unwrap()
                .entries
                .len(),
            2
        );
    }
    #[test]
    fn cancelling_initial_index_keeps_completed_entries_for_the_next_preview() {
        let f = Fixture::new();
        f.package("A.Scene.1", &[]);
        f.package("B.Scene.1", &[]);
        f.package("C.Scene.1", &[]);
        let cancel = AtomicBool::new(false);
        let result = make_plan_with_progress(
            &f.db,
            f.ctx.clone(),
            "cancel-index".into(),
            vec!["A.Scene.1".into()],
            false,
            vec![],
            &cancel,
            &mut |stage, n, total| {
                if stage == "building_index" && n == 1 {
                    assert_eq!(total, 3);
                    cancel.store(true, Ordering::Relaxed);
                }
            },
        );
        assert!(result.is_err());
        assert_eq!(
            ReferenceCache::load(&f.db, &f.ctx.library_key)
                .unwrap()
                .entries
                .len(),
            1
        );
        REFERENCE_READS.with(|reads| reads.set(0));
        let mut events = vec![];
        assert_eq!(
            make_plan_with_progress(
                &f.db,
                f.ctx.clone(),
                "resume-index".into(),
                vec!["A.Scene.1".into()],
                false,
                vec![],
                &AtomicBool::new(false),
                &mut |stage, n, total| events.push((stage.to_string(), n, total))
            )
            .unwrap()
            .preview
            .items
            .len(),
            1
        );
        REFERENCE_READS.with(|reads| assert_eq!(reads.get(), 2));
        assert!(events.contains(&("updating_index".into(), 0, 2)));
        assert!(events.contains(&("updating_index".into(), 2, 2)));
    }
    #[test]
    fn corrupt_or_outdated_cache_rows_are_rebuilt_without_losing_references() {
        let f = Fixture::new();
        f.package("A.Scene.1", &[]);
        f.package("B.Scene.1", &["A.Scene.1"]);
        assert!(f.plan(&["A.Scene.1"], false).preview.items.is_empty());
        let key = files::path_key(&f.dir.join("AddonPackages/B.Scene.1.var").to_string_lossy());
        for sql in [
            "UPDATE cleanup_reference_cache SET payload='invalid' WHERE path_key=?1",
            "UPDATE cleanup_reference_cache SET version=0 WHERE path_key=?1",
        ] {
            f.db.with_conn(|conn| {
                conn.execute(sql, [&key])?;
                Ok(())
            })
            .unwrap();
            REFERENCE_READS.with(|reads| reads.set(0));
            assert!(f.plan(&["A.Scene.1"], false).preview.items.is_empty());
            REFERENCE_READS.with(|reads| assert_eq!(reads.get(), 1));
        }
    }
    #[test]
    fn filesystems_without_change_stamps_still_detect_preserved_time_edits() {
        use super::super::cleanup_reference_cache::FORCE_SIGNATURE_CHECK;
        struct Reset;
        impl Drop for Reset {
            fn drop(&mut self) {
                FORCE_SIGNATURE_CHECK.with(|flag| flag.set(false));
            }
        }
        FORCE_SIGNATURE_CHECK.with(|flag| flag.set(true));
        let _reset = Reset;
        let f = Fixture::new();
        f.package("A.Scene.1", &[]);
        let b = f.package("B.Scene.1", &["C.Scene.1"]);
        f.package("C.Scene.1", &[]);
        let local = f.write("Saves/scene/local.json", br#"{"asset":"C.Scene.1:/asset"}"#);
        assert_eq!(f.plan(&["A.Scene.1"], false).preview.items.len(), 1);
        let bm = fs::metadata(&b).unwrap().modified().unwrap();
        let lm = fs::metadata(&local).unwrap().modified().unwrap();
        let before_b = files::fingerprint(&b).unwrap();
        let before_l = files::fingerprint(&local).unwrap();
        f.package("B.Scene.1", &["A.Scene.1"]);
        fs::write(&local, br#"{"asset":"A.Scene.1:/asset"}"#).unwrap();
        for (path, modified) in [(&b, bm), (&local, lm)] {
            fs::OpenOptions::new()
                .write(true)
                .open(path)
                .unwrap()
                .set_times(fs::FileTimes::new().set_modified(modified))
                .unwrap();
        }
        assert_eq!(files::fingerprint(&b).unwrap(), before_b);
        assert_eq!(files::fingerprint(&local).unwrap(), before_l);
        let plan = f.plan(&["A.Scene.1"], false);
        assert!(plan.preview.items.is_empty());
        assert!(plan.preview.blocked[0]
            .reasons
            .iter()
            .any(|reason| reason.code == "local"));
        fs::remove_file(local).unwrap();
        let plan = f.plan(&["A.Scene.1"], false);
        assert!(plan.preview.items.is_empty());
        assert!(plan.preview.blocked[0]
            .reasons
            .iter()
            .any(|reason| reason.code == "referenced"));
    }
    #[test]
    fn malformed_dependency_objects_are_not_treated_as_empty_dependencies() {
        let f = Fixture::new();
        f.package("A.Scene.1", &[]);
        f.raw_package(
            "B.Invalid.1",
            &[("meta.json", br#"{"dependencies":["A.Scene.1"]}"#)],
        );
        let plan = f.plan(&["A.Scene.1"], false);
        assert!(plan.preview.items.is_empty());
        assert!(plan.preview.warnings.is_empty());
        assert!(!plan.preview.diagnostics.is_empty());
        assert!(plan.preview.blocked[0]
            .reasons
            .iter()
            .any(|r| r.code == "referenced"));
        f.raw_package("B.Invalid.1", &[("meta.json", br#"{"dependencies":[]}"#)]);
        assert_eq!(f.plan(&["A.Scene.1"], false).preview.items.len(), 1);
    }
    #[test]
    fn compatible_formatting_in_other_packages_does_not_block_unrelated_cleanup() {
        let f = Fixture::new();
        f.package("A.Scene.1", &[]);
        f.raw_package(
            "B.Nonstandard.1",
            &[
                (
                    "meta.json",
                    br#"{"dependencies":{"Other.Package.1":{} "Other.Package.2":{}},}"#,
                ),
                (
                    "Custom/hair.vaj",
                    br#"{"asset":"Other.Package.latest:/hair"] Thanks:"Creator""#,
                ),
            ],
        );
        f.raw_package(
            "C.TruncatedMeta.1",
            &[("meta.json", br#"{"dependencies":{"Other.Package.1":{}}"#)],
        );
        f.raw_package(
            "D.NoMeta.1",
            &[(
                "Custom/preset.vap",
                br#"{"asset":"Other.Package.1:/preset"}"#,
            )],
        );
        let plan = f.plan(&["A.Scene.1"], false);
        assert_eq!(plan.preview.items.len(), 1);
        assert!(plan.preview.warnings.is_empty());
        assert_eq!(plan.preview.diagnostics.len(), 4);
        assert_eq!(f.run(plan).status, "completed");
    }
    #[test]
    fn compatible_references_still_protect_selected_packages() {
        let fixtures: &[&[(&str, &[u8])]] = &[
            &[(
                "meta.json",
                br#"{"dependencies":{"Other.Pack.1":{} "A.Scene.1":{}}}"#,
            )],
            &[(
                "meta.json",
                br#"{"dependencies":{"clothing:A.Scene.latest":{}}}"#,
            )],
            &[("meta.json", br#"{"dependencies":{"\u0041.Scene.1":{}}"#)],
            &[
                ("meta.json", b"{}"),
                (
                    "Custom/hair.vaj",
                    br#"{"asset":"Other.Pack.1:/hair"} Thanks:"A.Scene.1:/file""#,
                ),
            ],
            &[("Custom/preset.vap", br#"{"asset":"A.Scene.1:/preset"]"#)],
            &[("Custom/preset.vap", br#"{"asset":"A.Scene.1:/preset""#)],
            &[
                ("meta.json", b"{}"),
                ("Custom/preset.vap", b"{\"asset\":\"A.Sce\rne.1:/file\"}"),
            ],
        ];
        for entries in fixtures {
            let f = Fixture::new();
            f.package("A.Scene.1", &[]);
            f.raw_package("B.Consumer.1", entries);
            let plan = f.plan(&["A.Scene.1"], false);
            assert!(plan.preview.items.is_empty());
            assert!(
                plan.preview.warnings.is_empty(),
                "{:?}",
                plan.preview.warnings
            );
            assert!(plan.preview.blocked[0]
                .reasons
                .iter()
                .any(|r| r.code == "referenced"));
        }
    }
    #[test]
    fn unreadable_compatible_content_still_blocks_and_changed_references_invalidate_preview() {
        let f = Fixture::new();
        let selected = f.package("A.Scene.1", &[]);
        for body in [
            &b"{\"asset\":\"A.Scene.1:/file}"[..],
            &b"{\"asset\":\"\xff\"}"[..],
            &b"{\"asset\":\"\\uZZZZ\"}"[..],
            &b"{"[..],
        ] {
            f.write("Custom/bad.vap", body);
            let plan = f.plan(&["A.Scene.1"], false);
            assert!(plan.preview.items.is_empty());
            assert!(!plan.preview.warnings.is_empty());
        }
        fs::remove_file(f.dir.join("Custom/bad.vap")).unwrap();
        f.raw_package("B.Consumer.1", &[("meta.json", br#"{"dependencies":[],}"#)]);
        let plan = f.plan(&["A.Scene.1"], false);
        assert_eq!(plan.preview.items.len(), 1);
        f.raw_package(
            "B.Consumer.1",
            &[("meta.json", br#"{"dependencies":["A.Scene.1"],}"#)],
        );
        assert!(execute(
            &f.db,
            &f.dir.join("data"),
            plan,
            &AtomicBool::new(false),
            &mut |_, _| {}
        )
        .is_err());
        assert!(selected.exists());
    }
    #[test]
    fn compatibility_diagnostics_and_references_survive_cache_reopen() {
        let mut f = Fixture::new();
        f.package("A.Scene.1", &[]);
        f.package("C.Scene.1", &[]);
        f.raw_package(
            "B.Consumer.1",
            &[("meta.json", br#"{"dependencies":["A.Scene.1"],}"#)],
        );
        let before = f.plan(&["C.Scene.1"], false).preview;
        assert_eq!(before.items.len(), 1);
        assert!(!before.diagnostics.is_empty());
        drop(f.db);
        f.db = Database::new(&f.dir.join("data/index.db")).unwrap();
        REFERENCE_READS.with(|reads| reads.set(0));
        let after = f.plan(&["C.Scene.1"], false).preview;
        assert_eq!(after.diagnostics, before.diagnostics);
        assert_eq!(after.items.len(), 1);
        assert!(f.plan(&["A.Scene.1"], false).preview.items.is_empty());
        REFERENCE_READS.with(|reads| assert_eq!(reads.get(), 0));
    }
    #[test]
    #[ignore = "requires VAM_CLEANUP_COMPAT_MANIFEST: JSON paths to reported VARs; reads originals without modifying the library"]
    fn reported_nonstandard_archives_are_fully_inspected_read_only() {
        let manifest = std::env::var("VAM_CLEANUP_COMPAT_MANIFEST").unwrap();
        let paths: Vec<String> = serde_json::from_slice(&fs::read(manifest).unwrap()).unwrap();
        assert!(!paths.is_empty());
        let mut issues = 0;
        let mut references = 0;
        for path in &paths {
            let file = files::fingerprint(Path::new(path)).unwrap();
            let stamp = change_stamp(Path::new(path)).unwrap();
            let entry = archive_references(&file, &stamp, None, false, &AtomicBool::new(false))
                .unwrap_or_else(|e| {
                    panic!(
                        "{}: {e}",
                        Path::new(path).file_name().unwrap().to_string_lossy()
                    )
                });
            issues += entry.diagnostics.len();
            references += entry.refs.len();
            assert!(entry.full);
            verify_reference_source(&file, &stamp).unwrap();
        }
        eprintln!("Read-only compatibility verification: {} archives, {issues} recovered format issues, {references} reference IDs", paths.len());
        assert!(issues > 0);
    }
    #[test]
    fn restores_keep_newer_user_tags_and_import_time_when_identical_original_exists() {
        let f = Fixture::new();
        let path = f.package("A.Scene.1", &[]);
        f.db.with_conn(|c| {
            c.execute(
                "INSERT INTO package_tags(package_id,tag) VALUES('A.Scene.1','old')",
                [],
            )?;
            Ok(())
        })
        .unwrap();
        let batch = f.run(f.plan(&["A.Scene.1"], false));
        fs::copy(&batch.files[0].trash_path, &path).unwrap();
        f.db.with_conn(|c| {
            files::index_package(c, &path)?;
            c.execute(
                "INSERT INTO package_tags(package_id,tag) VALUES('A.Scene.1','new')",
                [],
            )?;
            c.execute("UPDATE packages SET scan_time='new-import'", [])?;
            Ok(())
        })
        .unwrap();
        assert_eq!(
            restore(&f.db, &f.dir.join("data"), &f.ctx, &batch.id)
                .unwrap()
                .status,
            "restored"
        );
        f.db.with_conn(|c| {
            assert_eq!(
                c.query_row("SELECT tag FROM package_tags", [], |r| r
                    .get::<_, String>(0))?,
                "new"
            );
            assert_eq!(
                c.query_row("SELECT scan_time FROM packages", [], |r| r
                    .get::<_, String>(0))?,
                "new-import"
            );
            Ok(())
        })
        .unwrap();
    }
    #[test]
    fn interrupted_permanent_deletion_requires_explicit_resume_and_cannot_be_restored() {
        let f = Fixture::new();
        f.package("A.Scene.1", &[]);
        let mut batch = f.run(f.plan(&["A.Scene.1"], false));
        batch.status = "purging".into();
        batch.files[0].status = "purging".into();
        write_batch(&f.dir.join("data"), &batch).unwrap();
        fs::remove_file(&batch.files[0].trash_path).unwrap();
        save_policy(&f.db, 30, true).unwrap();
        assert_eq!(auto_purge(&f.db, &f.dir.join("data"), &f.ctx).unwrap(), 0);
        assert_eq!(
            purge(&f.dir.join("data"), &f.ctx, &batch.id)
                .unwrap()
                .status,
            "purged"
        );
        assert!(restore(&f.db, &f.dir.join("data"), &f.ctx, &batch.id).is_err());
    }
    #[test]
    fn foreign_index_metadata_is_rejected_before_any_resource_move() {
        let a = Fixture::new();
        let b = Fixture::new();
        let original = a.package("A.Scene.1", &[]);
        let foreign = b.package("A.Scene.1", &[]);
        a.db.with_conn(|c| {
            c.execute(
                "UPDATE packages SET file_path=?1",
                [foreign.to_string_lossy().as_ref()],
            )?;
            Ok(())
        })
        .unwrap();
        assert!(make_plan(
            &a.db,
            a.ctx.clone(),
            "foreign-index".into(),
            vec!["A.Scene.1".into()],
            false,
            vec![],
            &AtomicBool::new(false),
            &mut |_, _| {}
        )
        .is_err());
        assert!(original.exists() && foreign.exists());
    }
    #[test]
    fn unicode_package_keys_match_frontend_selection_and_local_references() {
        let f = Fixture::new();
        f.package("Äuthor.Scene.1", &[]);
        set_marks(&f.db, &f.ctx, &["äuthor.scene.1".into()], "protect").unwrap();
        assert!(f.plan(&["äuthor.scene.1"], false).preview.items.is_empty());
        set_marks(&f.db, &f.ctx, &["äuthor.scene.1".into()], "unprotect").unwrap();
        assert_eq!(f.plan(&["äuthor.scene.1"], false).preview.items.len(), 1);
        f.write(
            "Custom/local.vap",
            br#"{"asset":"\u00e4uthor.scene.1:/file"}"#,
        );
        assert!(f.plan(&["äuthor.scene.1"], false).preview.items.is_empty());
    }
    #[test]
    fn streaming_reference_scan_accepts_bom_short_reads_and_large_json_without_materializing_it() {
        let f = Fixture::new();
        f.package("A.Scene.1", &[]);
        let path = f.dir.join("Custom/large.vap");
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        let mut output = fs::File::create(&path).unwrap();
        output.write_all(b"\xef\xbb\xbf").unwrap();
        output
            .write_all(br#"{"values":[true,null,123,1.5,{"asset":"A.Scene.1:/file"}]}"#)
            .unwrap();
        let spaces = [b' '; 64 * 1024];
        for _ in 0..1025 {
            output.write_all(&spaces).unwrap();
        }
        drop(output);
        let plan = f.plan(&["A.Scene.1"], false);
        assert!(plan.preview.items.is_empty());
        assert!(plan.preview.warnings.is_empty());
        assert_eq!(plan.preview.blocked[0].reasons[0].code, "local");
        let mut refs = References::default();
        stream_references(
            std::io::Cursor::new(b"[]"),
            &mut refs,
            &AtomicBool::new(false),
        )
        .unwrap();
        assert!(stream_references(
            std::io::Cursor::new(b"{} garbage"),
            &mut refs,
            &AtomicBool::new(false)
        )
        .is_err());
    }

    fn removal_plan(f: &Fixture, selected: &[&str], deps: bool, quick: bool) -> Plan {
        let cancel = AtomicBool::new(false);
        let cat = build_catalog(&f.db, &f.ctx, quick, &cancel, &mut |_, _, _| {}).unwrap();
        make_plan_with_removal_choice(&f.db, f.ctx.clone(), &cat,
            format!("removal-{}", NEXT.fetch_add(1, Ordering::Relaxed)),
            selected.iter().map(|s| s.to_string()).collect(), deps, vec![],
            &selected.iter().map(|s| s.to_string()).collect::<Vec<_>>(), &cancel).unwrap()
    }
    fn run_confirmed(f: &Fixture, plan: Plan) -> Result<Batch> {
        execute_with_progress(&f.db, &f.dir.join("data"), plan, true,
            &AtomicBool::new(false), &mut |_, _, _| {})
    }
    #[test]
    fn renamed_archives_are_checked_cached_and_preserve_their_dependencies() {
        let mut f = Fixture::new();
        let a = f.package("A.Scene.1", &[]);
        f.package("C.Unrelated.1", &[]);
        let b = f.raw_package("B.Renamed.1_2", &[("meta.json", br#"{"dependencies":{"A.Scene.1":{}}}"#)]);
        let original = fs::read(&b).unwrap();
        let plan = f.plan(&["A.Scene.1"], false);
        assert!(plan.preview.items.is_empty());
        assert!(plan.preview.warnings.is_empty());
        assert!(plan.preview.blocked[0].reasons.iter().any(|r| r.code == "referenced" && r.source == "B.Renamed.1_2"));
        assert_eq!(f.plan(&["C.Unrelated.1"], false).preview.items.len(), 1);
        drop(f.db);
        f.db = Database::new(&f.dir.join("data/index.db")).unwrap();
        REFERENCE_READS.with(|reads| reads.set(0));
        assert!(f.plan(&["A.Scene.1"], false).preview.items.is_empty());
        REFERENCE_READS.with(|reads| assert_eq!(reads.get(), 0));
        assert!(a.exists());
        assert_eq!(fs::read(b).unwrap(), original);
    }
    #[test]
    fn abnormal_names_can_be_cleaned_with_damaged_packages_and_restored() {
        let f = Fixture::new();
        let names = ["Qing.Alet01.1_2", "jyy.Bush42.2_2", "V2.Morphs.latest", "renamed"];
        let mut originals = vec![];
        for id in names {
            let path = f.raw_package(id, &[("meta.json", b"{\"dependencies\":{}}")]);
            let bytes = fs::read(&path).unwrap();
            f.db.with_conn(|c| {
                c.execute("INSERT INTO physical_packages(file_path,package_id,size_bytes,scan_time,scan_status,last_error) VALUES(?1,?2,?3,'old','failed','Invalid filename')",
                    params![path.to_string_lossy(),id,bytes.len() as i64])?;
                Ok(())
            }).unwrap();
            originals.push((path, bytes));
        }
        let copy = f.write("AddonPackages/nested/Qing.Alet01.1_2.var", &originals[0].1);
        originals.push((copy, originals[0].1.clone()));
        let damaged = f.write("AddonPackages/Bad.Archive.1.var", b"PKxxtruncated");
        originals.push((damaged, b"PKxxtruncated".to_vec()));
        let mut selected = names.to_vec();
        selected.push("Bad.Archive.1");
        let plan = f.plan(&selected, false);
        assert_eq!(plan.preview.items.len(), 5);
        assert_eq!(plan.preview.total_files, 6);
        assert!(plan.preview.blocked.is_empty());
        assert_eq!(plan.preview.items.iter().filter(|p| p.invalid_name).count(), 4);
        assert!(plan.preview.items.iter().filter(|p| p.invalid_name).all(|p| p.issue.is_none()));
        let batch = f.run(plan);
        assert_eq!(batch.status, "completed");
        assert!(originals.iter().all(|(path, _)| !path.exists()));
        assert_eq!(list_batches(&f.dir.join("data"), &f.ctx).unwrap().len(), 1);
        let restored = restore(&f.db, &f.dir.join("data"), &f.ctx, &batch.id).unwrap();
        assert_eq!(restored.status, "restored");
        for (path, bytes) in &originals { assert_eq!(fs::read(path).unwrap(), *bytes); }
        f.db.with_conn(|c| {
            // Recoverable names now regain their full records. Truly unreadable
            // files retain their previous failure status and original identity.
            let failed: Vec<String> = c.prepare("SELECT package_id FROM physical_packages WHERE scan_status='failed' ORDER BY package_id")?
                .query_map([], |r| r.get(0))?.collect::<rusqlite::Result<_>>()?;
            assert_eq!(failed, ["Bad.Archive.1", "renamed"]);
            let indexed: Vec<String> = c.prepare("SELECT id FROM packages ORDER BY id")?
                .query_map([], |r| r.get(0))?.collect::<rusqlite::Result<_>>()?;
            assert_eq!(indexed, ["Qing.Alet01.1_2", "V2.Morphs.latest", "jyy.Bush42.2_2"]);
            let ok: i64 = c.query_row("SELECT COUNT(*) FROM physical_packages WHERE scan_status='ok'", [], |r| r.get(0))?;
            assert_eq!(ok, 4);
            Ok(())
        }).unwrap();
        let batch = f.run(f.plan(&selected, false));
        assert_eq!(purge(&f.dir.join("data"), &f.ctx, &batch.id).unwrap().status, "purged");
        assert!(batch.files.iter().all(|file| !Path::new(&file.trash_path).exists()));
    }
    #[test]
    fn dedup_checks_consumers_and_manual_protection_of_nonstandard_names() {
        let f = Fixture::new();
        let renamed = f.raw_package("A.Asset.1_2", &[("meta.json", b"{}")]);
        f.raw_package("B.Scene.1", &[("meta.json", br#"{"dependencies":{"A.Asset.1_2":{}}}"#)]);
        let paths = vec![renamed.to_string_lossy().into_owned()];
        assert!(!preserves_package_copies(&f.ctx, &paths).unwrap());
        assert!(validate_deletions(&f.db, &f.ctx, &paths).unwrap_err().contains("引用"));
        set_marks(&f.db, &f.ctx, &["A.Asset.1_2".into()], "protect").unwrap();
        assert!(validate_deletions(&f.db, &f.ctx, &paths).unwrap_err().contains("手动保护"));
        assert!(renamed.exists());
    }

    #[test]
    fn abnormal_names_preserve_dependencies_and_latest_resolution() {
        let f = Fixture::new();
        f.package("A.Morphs.2", &[]);
        f.package("A.Morphs.10", &[]);
        f.raw_package("A.Morphs.latest", &[("meta.json", br#"{"dependencies":{"A.Morphs.latest":{}}}"#)]);
        let cat = build_catalog(&f.db, &f.ctx, false, &AtomicBool::new(false), &mut |_, _, _| {}).unwrap();
        assert_eq!(targets("A.Morphs.latest", &cat.packages), vec!["a.morphs.10"]);
        let health = package_health(&cat);
        assert!(health.iter().any(|p| p.package_id == "A.Morphs.latest" && p.invalid_name && p.issue.is_none()));
        assert!(f.plan(&["A.Morphs.10"], false).preview.items.is_empty());
        let together = f.plan(&["A.Morphs.latest", "A.Morphs.10"], false);
        assert_eq!(together.preview.items.len(), 2);
        set_marks(&f.db, &f.ctx, &["A.Morphs.latest".into()], "protect").unwrap();
        assert!(f.plan(&["A.Morphs.latest"], false).preview.blocked[0].reasons.iter().any(|r| r.code == "manual"));
    }
    #[test]
    fn abnormal_names_keep_directory_and_incomplete_check_protections() {
        let mut f = Fixture::new();
        f.raw_package("A.Renamed.1_2", &[("meta.json", b"{}")]);
        f.write("Custom/broken.vap", b"{");
        assert!(f.plan(&["A.Renamed.1_2"], false).preview.blocked[0].reasons.iter().any(|r| r.code == "incomplete"));
        for (managed, shared) in [(true, false), (false, true)] {
            f.ctx.managed = managed; f.ctx.shared = shared;
            assert!(removal_plan(&f, &["A.Renamed.1_2"], false, false).preview.items.is_empty());
        }
    }
    #[test]
    fn abnormal_name_cleanup_rejects_paths_and_tampered_manifests() {
        for id in ["", ".", "..", "../outside", "..\\outside", "C:outside", "file:stream", "a/b", "a\\b", "bad\0name", "bad*name", "trailing.", "trailing "] {
            assert!(validate_request(&[id.into()]).is_err(), "{id:?}");
        }
        let f = Fixture::new();
        f.raw_package("A.Renamed.1_2", &[("meta.json", b"{}")]);
        let batch = f.run(f.plan(&["A.Renamed.1_2"], false));
        for source in [f.dir.join("outside.var"), f.dir.join("AddonPackages/A.Renamed.1_2.txt"), f.dir.join("AddonPackages/Other.1_2.var"), f.dir.join("AddonPackages/../A.Renamed.1_2.var")] {
            let mut altered = batch.clone();
            altered.files[0].original.path = source.to_string_lossy().into_owned();
            assert!(validate_batch(&altered, &f.ctx).is_err());
        }
        assert_eq!(restore(&f.db, &f.dir.join("data"), &f.ctx, &batch.id).unwrap().status, "restored");
    }
    #[test]
    fn download_error_pages_are_not_unknown_consumers_but_corrupt_zips_are() {
        let f = Fixture::new();
        f.package("A.Scene.1", &[]);
        let html = f.write("AddonPackages/B.Download.1.var", b"<!DOCTYPE html><html>download failed</html>");
        let xml = f.write("AddonPackages/C.Download.latest.var", b"<?xml version=\"1.0\"?><Error><Code>AccessDenied</Code></Error>");
        let plan = f.plan(&["A.Scene.1"], true);
        assert_eq!(plan.preview.items.len(), 1);
        assert!(plan.preview.warnings.is_empty());
        assert_eq!(plan.preview.invalid_files.len(), 2);
        assert!(!plan.preview.requires_confirmation);
        REFERENCE_READS.with(|reads| reads.set(0));
        assert_eq!(f.plan(&["A.Scene.1"], false).preview.invalid_files.len(), 2);
        REFERENCE_READS.with(|reads| assert_eq!(reads.get(), 0));
        let original = fs::read(&html).unwrap();
        let batch = f.run(plan);
        assert_eq!(batch.status, "completed");
        assert_eq!(fs::read(html).unwrap(), original);
        assert!(xml.exists());
        restore(&f.db, &f.dir.join("data"), &f.ctx, &batch.id).unwrap();
        f.write("AddonPackages/D.Broken.1.var", b"PKxxtruncated");
        f.write("AddonPackages/E.Packed.1.var", &[0x37, 0x7a, 0xbc, 0xaf, 0x27, 0x1c, 0, 4]);
        let plan = f.plan(&["A.Scene.1"], false);
        assert_eq!(plan.preview.items.len(), 1);
        assert!(plan.preview.warnings.is_empty());
        let cat = build_catalog(&f.db, &f.ctx, false, &AtomicBool::new(false), &mut |_, _, _| {}).unwrap();
        assert_eq!(cat.problems.get("b.download.1").map(String::as_str), Some("invalid_download"));
        assert_eq!(cat.problems.get("d.broken.1").map(String::as_str), Some("damaged"));
        assert_eq!(cat.problems.get("e.packed.1").map(String::as_str), Some("wrong_format"));
        // Selecting an unusable package never needs an override, even when it is "used".
        f.package("F.Consumer.1", &["D.Broken.1"]);
        let plan = f.plan(&["D.Broken.1"], false);
        assert_eq!(ids(&plan.preview.items), BTreeSet::from(["d.broken.1".into()]));
        assert_eq!(plan.preview.items[0].issue.as_deref(), Some("damaged"));
        assert!(!plan.preview.requires_confirmation);
    }
    #[test]
    fn explicit_removal_requires_acknowledgment_and_keeps_unknown_attached_dependencies() {
        let f = Fixture::new();
        let a = f.package("A.Scene.1", &["D.Dependency.1"]);
        let d = f.package("D.Dependency.1", &[]);
        f.write("Custom/broken.vap", b"{");
        let safe = f.plan(&["A.Scene.1"], true);
        assert!(safe.preview.items.is_empty());
        let plan = removal_plan(&f, &["A.Scene.1"], true, false);
        assert_eq!(ids(&plan.preview.items), BTreeSet::from(["a.scene.1".into()]));
        assert!(plan.preview.requires_confirmation);
        assert_eq!(ids(&plan.preview.retained), BTreeSet::from(["d.dependency.1".into()]));
        assert!(execute(&f.db, &f.dir.join("data"), plan.clone(), &AtomicBool::new(false), &mut |_, _| {}).is_err());
        assert!(a.exists() && d.exists());
        let batch = run_confirmed(&f, plan).unwrap();
        assert_eq!(batch.status, "completed");
        assert!(!a.exists() && d.exists());
        assert_eq!(restore(&f.db, &f.dir.join("data"), &f.ctx, &batch.id).unwrap().status, "restored");
        assert!(a.exists() && d.exists());
    }
    #[test]
    fn explicit_removal_reports_known_consumers_and_does_not_remove_their_dependencies() {
        let f = Fixture::new();
        let a = f.package("A.Scene.1", &["D.Dependency.1"]);
        let b = f.package("B.Consumer.1", &["A.Scene.1"]);
        let d = f.package("D.Dependency.1", &[]);
        f.write("Saves/scene/local.json", br#"{"asset":"A.Scene.1:/file"}"#);
        f.write("AddonPackagesFilePrefs/A.Scene.1/Saves/scene/example.json.fav", b"");
        let plan = removal_plan(&f, &["A.Scene.1"], true, false);
        assert_eq!(plan.preview.items.len(), 1);
        assert!(plan.preview.requires_confirmation);
        assert!(plan.preview.items[0].reasons.iter().any(|r| r.code == "local"));
        assert!(plan.preview.items[0].reasons.iter().any(|r| r.code == "favorite"));
        let batch = run_confirmed(&f, plan).unwrap();
        assert_eq!(batch.status, "completed");
        assert!(!a.exists() && b.exists() && d.exists());
        assert_eq!(restore(&f.db, &f.dir.join("data"), &f.ctx, &batch.id).unwrap().status, "restored");
    }
    #[test]
    fn explicit_removal_never_overrides_manual_or_directory_protection_or_exclusions() {
        let mut f = Fixture::new();
        let a = f.package("A.Scene.1", &[]);
        set_marks(&f.db, &f.ctx, &["A.Scene.1".into()], "protect").unwrap();
        assert!(removal_plan(&f, &["A.Scene.1"], false, false).preview.items.is_empty());
        set_marks(&f.db, &f.ctx, &["A.Scene.1".into()], "unprotect").unwrap();
        for (managed, shared) in [(true, false), (false, true)] {
            f.ctx.managed = managed; f.ctx.shared = shared;
            assert!(removal_plan(&f, &["A.Scene.1"], false, false).preview.items.is_empty());
        }
        f.ctx.managed = false; f.ctx.shared = false;
        let cancel = AtomicBool::new(false);
        let cat = build_catalog(&f.db, &f.ctx, false, &cancel, &mut |_, _, _| {}).unwrap();
        let plan = make_plan_with_removal_choice(&f.db, f.ctx.clone(), &cat, "excluded".into(), vec!["A.Scene.1".into()], false, vec!["A.Scene.1".into()], &["A.Scene.1".into()], &cancel).unwrap();
        assert!(plan.preview.items.is_empty());
        assert!(a.exists());
    }
    #[test]
    fn quick_removal_keeps_attached_dependencies_and_requires_acknowledgment() {
        let f = Fixture::new();
        let a = f.package("A.Scene.1", &["D.Dependency.1"]);
        let d = f.package("D.Dependency.1", &[]);
        let plan = removal_plan(&f, &["A.Scene.1"], true, true);
        assert_eq!(plan.preview.items.len(), 1);
        assert!(plan.preview.requires_confirmation);
        assert_eq!(plan.preview.retained.len(), 1);
        assert!(execute(&f.db, &f.dir.join("data"), plan.clone(), &AtomicBool::new(false), &mut |_, _| {}).is_err());
        assert_eq!(run_confirmed(&f, plan).unwrap().status, "completed");
        assert!(!a.exists() && d.exists());
    }
    #[test]
    fn explicit_removal_still_rejects_stale_previews_before_moving() {
        let f = Fixture::new();
        let a = f.package("A.Scene.1", &[]);
        f.write("Custom/broken.vap", b"{");
        let plan = removal_plan(&f, &["A.Scene.1"], false, false);
        f.package("B.NewConsumer.1", &["A.Scene.1"]);
        assert!(run_confirmed(&f, plan).is_err());
        assert!(a.exists());
    }
    #[test]
    fn packages_naming_themselves_are_not_reported_as_their_own_users() {
        let f = Fixture::new();
        f.package_at("", "A.Look.1", &[], serde_json::json!({"id":"A.Look.1:/Custom/Atom/Person/Appearance/a.vap"}));
        f.package("B.Scene.1", &["A.Look.1"]);
        let plan = f.plan(&["A.Look.1"], false);
        let users: Vec<_> = plan.preview.blocked[0].reasons.iter().map(|r| r.source.as_str()).collect();
        assert_eq!(users, vec!["B.Scene.1"]);
        assert_eq!(f.plan(&["A.Look.1", "B.Scene.1"], false).preview.items.len(), 2);
    }
    #[test]
    fn dependencies_only_the_selection_uses_are_offered_with_their_size() {
        let f = Fixture::new();
        f.package("A.Scene.1", &["C.Clothes.1", "D.Shared.1"]);
        f.package("B.Scene.1", &["D.Shared.1"]);
        f.package("C.Clothes.1", &[]);
        f.package("D.Shared.1", &[]);
        let plan = f.plan(&["A.Scene.1"], false);
        assert_eq!(ids(&plan.preview.items), BTreeSet::from(["a.scene.1".into()]));
        assert_eq!(ids(&plan.preview.suggested), BTreeSet::from(["c.clothes.1".into()]));
        assert_eq!(plan.preview.suggested_bytes, plan.preview.suggested[0].size_bytes);
        assert!(plan.files.iter().all(|file| file.package_id == "A.Scene.1"));
        let included = f.plan(&["A.Scene.1"], true);
        assert!(included.preview.suggested.is_empty());
        assert_eq!(ids(&included.preview.items), BTreeSet::from(["a.scene.1".into(), "c.clothes.1".into()]));
    }
    #[test]
    fn removing_anyway_applies_only_to_the_packages_chosen() {
        let f = Fixture::new();
        f.package("A.Look.1", &[]);
        f.package("B.Look.1", &[]);
        f.package("C.Scene.1", &["A.Look.1", "B.Look.1"]);
        let cancel = AtomicBool::new(false);
        let cat = build_catalog(&f.db, &f.ctx, false, &cancel, &mut |_, _, _| {}).unwrap();
        let plan = make_plan_with_removal_choice(&f.db, f.ctx.clone(), &cat, "force".into(),
            vec!["A.Look.1".into(), "B.Look.1".into()], false, vec![], &["a.look.1".into()], &cancel).unwrap();
        assert_eq!(ids(&plan.preview.items), BTreeSet::from(["a.look.1".into()]));
        assert_eq!(ids(&plan.preview.blocked), BTreeSet::from(["b.look.1".into()]));
        assert!(plan.preview.requires_confirmation);
        assert!(make_plan_with_removal_choice(&f.db, f.ctx.clone(), &cat, "outside".into(),
            vec!["A.Look.1".into()], false, vec![], &["C.Scene.1".into()], &cancel).is_err());
    }
    #[test]
    fn package_health_reports_unusable_files_copies_and_unused_packages() {
        let f = Fixture::new();
        f.package("A.Scene.1", &["B.Morph.1"]);
        f.package("B.Morph.1", &[]);
        let copy = f.package("C.Morph.1", &[]);
        fs::create_dir_all(f.dir.join("AddonPackages/Sub")).unwrap();
        fs::copy(&copy, f.dir.join("AddonPackages/Sub/C.Morph.1.var")).unwrap();
        f.package("D.Fav.1", &[]);
        f.write("AddonPackagesFilePrefs/D.Fav.1/Custom/a.vap.fav", b"");
        f.package("E.Local.1", &[]);
        f.write("Saves/scene/local.json", br#"{"a":"E.Local.1:/x"}"#);
        f.write("AddonPackages/F.Broken.1.var", b"PKxx");
        f.package("G.Kept.1", &[]);
        set_marks(&f.db, &f.ctx, &["G.Kept.1".into()], "protect").unwrap();
        let cat = build_catalog(&f.db, &f.ctx, false, &AtomicBool::new(false), &mut |_, _, _| {}).unwrap();
        let health: BTreeMap<_, _> = package_health(&cat).into_iter().map(|h| (h.package_id.clone(), h)).collect();
        assert!(health["A.Scene.1"].unused);
        assert!(!health["A.Scene.1"].standalone);
        assert!(!health.contains_key("B.Morph.1"));
        assert_eq!((health["C.Morph.1"].copies, health["C.Morph.1"].unused), (2, true));
        assert!(health["C.Morph.1"].standalone);
        assert!(!health.contains_key("D.Fav.1"));
        assert!(!health.contains_key("E.Local.1"));
        assert_eq!(health["F.Broken.1"].issue.as_deref(), Some("damaged"));
        assert!(!health["F.Broken.1"].standalone);
        // Manual protection is the page's own state, not a use by another resource.
        assert!(health["G.Kept.1"].unused);
    }
    #[test]
    fn standalone_packages_ignore_program_versions_and_plugin_storable_ids() {
        let f = Fixture::new();
        f.raw_package("A.Complete.1", &[
            ("meta.json", br#"{"programVersion":"1.22.0.3","dependencies":{}}"#),
            ("Saves/scene/example.json", br#"{"version":"1.22.0.3","storables":[{"id":"plugin#0_MacGruber.PostMagic.DepthOfField"}]}"#),
        ]);
        let cat = build_catalog(&f.db, &f.ctx, false, &AtomicBool::new(false), &mut |_, _, _| {}).unwrap();
        assert!(cat.warnings.is_empty(), "{:?}", cat.warnings);
        let health = package_health(&cat);
        assert!(health.iter().any(|h| h.package_id == "A.Complete.1" && h.standalone), "{health:?}");
    }
    #[test]
    #[ignore = "requires VAM_CLEANUP_STANDALONE_MANIFEST listing self-contained archives; reads originals only"]
    fn reported_standalone_archives_are_checked_read_only() {
        let manifest = std::env::var("VAM_CLEANUP_STANDALONE_MANIFEST").unwrap();
        let paths: Vec<String> = serde_json::from_slice(&fs::read(manifest).unwrap()).unwrap();
        assert!(!paths.is_empty());
        for path in &paths {
            let path = Path::new(path);
            let file = files::disk_file(path, &fs::metadata(path).unwrap()).unwrap();
            let stamp = change_stamp(path).unwrap();
            let entry = archive_references(&file, &stamp, None, false, &AtomicBool::new(false)).unwrap();
            assert!(entry.problem().is_none());
            assert!(entry.full);
            let id = cleanup_file_id(path).unwrap().to_lowercase();
            let latest = format!("{}.latest", id.rsplit_once('.').unwrap().0);
            assert!(entry.package_refs.iter().all(|r| r == &id || r == &latest), "{id}: {:?}", entry.package_refs);
        }
        eprintln!("Read-only standalone verification: {} real archives checked", paths.len());
    }
    #[test]
    fn standalone_packages_preserve_explicit_numeric_and_nonstandard_dependencies() {
        for (meta, content) in [
            (&br#"{"programVersion":"1.22.0.3","dependencies":{"1.22.0.3":{}}}"#[..], &b"{}"[..]),
            (br#"{"dependencies":{"clothing:1.22.0.3":{}}}"#, b"{}"),
            (br#"{"dependencies":{"A.Pack.1":{"dependencies":{"1.22.0.3":{}}}}}"#, b"{}"),
            (br#"{"dependencies":{"A.Pack.unknown":{}}}"#, b"{}"),
            (br#"{"dependencies":["1.22.0.3"]}"#, b"{}"),
            (b"{}", br#"{"path":"1.22.0.3:/Custom/x.vap"}"#),
            (b"{}", br#"{"path":"A.Pack.unknown:/Custom/x.vap"}"#),
            (b"{}", br#"{"dependencies":{"1.22.0.3":{}}}"#),
            (b"{}", br#"{"dependencies":{"1.22.0.3":{}}"#),
        ] {
            let f = Fixture::new();
            f.raw_package("A.Complete.1", &[("meta.json", meta), ("Custom/x.vap", content)]);
            let cat = build_catalog(&f.db, &f.ctx, false, &AtomicBool::new(false), &mut |_, _, _| {}).unwrap();
            assert!(cat.warnings.is_empty(), "{:?}", cat.warnings);
            assert!(!package_health(&cat).iter().any(|h| h.standalone), "meta={:?}, content={:?}", meta, content);
        }
    }
    #[test]
    fn standalone_packages_keep_ambiguous_references_to_installed_packages() {
        let f = Fixture::new();
        f.package("1.22.0.3", &[]);
        f.raw_package("A.Complete.1", &[("meta.json", br#"{"programVersion":"1.22.0.3","dependencies":{}}"#)]);
        let cat = build_catalog(&f.db, &f.ctx, false, &AtomicBool::new(false), &mut |_, _, _| {}).unwrap();
        assert!(!package_health(&cat).iter().any(|h| h.standalone));
        let plan = f.plan(&["1.22.0.3"], false);
        assert!(plan.preview.items.is_empty());
        assert!(plan.preview.blocked[0].reasons.iter().any(|r| r.code == "referenced"));
    }
    #[test]
    fn standalone_packages_rebuild_legacy_references_and_reuse_corrected_cache() {
        let f = Fixture::new();
        f.raw_package("A.Complete.1", &[("meta.json", br#"{"programVersion":"1.22.0.3","dependencies":{}}"#)]);
        let cancel = AtomicBool::new(false);
        build_catalog(&f.db, &f.ctx, false, &cancel, &mut |_, _, _| {}).unwrap();
        f.db.with_conn(|c| {
            c.execute("UPDATE cleanup_reference_cache SET version=2, payload=json_remove(payload, '$.package_refs')", [])?;
            Ok(())
        }).unwrap();
        for expected_reads in [1, 0] {
            REFERENCE_READS.with(|reads| reads.set(0));
            let cat = build_catalog(&f.db, &f.ctx, false, &cancel, &mut |_, _, _| {}).unwrap();
            assert!(package_health(&cat).iter().any(|h| h.package_id == "A.Complete.1" && h.standalone));
            REFERENCE_READS.with(|reads| assert_eq!(reads.get(), expected_reads));
        }
    }
    #[test]
    fn standalone_packages_require_no_incoming_or_outgoing_references() {
        let f = Fixture::new();
        f.package("A.Complete.1", &[]);
        f.package("B.Scene.1", &["C.Morph.1"]);
        f.package("C.Morph.1", &[]);
        f.package("D.Missing.1", &["Missing.Dependency.1"]);
        f.package_at("", "E.Undeclared.1", &[], serde_json::json!({"path": "C.Morph.1:/x"}));
        f.package_at("", "F.MissingContent.1", &[], serde_json::json!({"path": "Missing.Asset.1:/x"}));
        f.package_at("", "G.Self.1", &["G.Self.1", "G.Self.latest"], serde_json::json!({
            "path": "G.Self.1:/x", "local": "SELF:/Custom/x", "builtIn": "Custom/x"
        }));
        let cat = build_catalog(&f.db, &f.ctx, false, &AtomicBool::new(false), &mut |_, _, _| {}).unwrap();
        let standalone: BTreeSet<_> = package_health(&cat).into_iter()
            .filter(|h| h.standalone).map(|h| h.package_id).collect();
        assert_eq!(standalone, BTreeSet::from(["A.Complete.1".into(), "G.Self.1".into()]));
        // All eligible packages still go through the ordinary cleanup preview.
        assert_eq!(f.plan(&["A.Complete.1", "G.Self.1"], false).preview.items.len(), 2);
    }
    #[test]
    fn standalone_packages_keep_uses_by_local_files_favorites_and_launch_sets() {
        let f = Fixture::new();
        f.package("A.Local.1", &[]);
        f.package("B.Favorite.1", &[]);
        f.package("C.Plan.1", &[]);
        f.write("Saves/scene/local.json", br#"{"path":"A.Local.latest:/x"}"#);
        f.write("AddonPackagesFilePrefs/B.Favorite.1/Custom/x.vap.fav", b"");
        f.db.with_conn(|c| {
            c.execute("INSERT INTO on_demand_plans (name,package_ids) VALUES (?1,?2)",
                rusqlite::params!["test", "[\"C.Plan.1\"]"])?;
            Ok(())
        }).unwrap();
        let cat = build_catalog(&f.db, &f.ctx, false, &AtomicBool::new(false), &mut |_, _, _| {}).unwrap();
        assert!(!package_health(&cat).iter().any(|h| h.standalone));
    }
    #[test]
    fn standalone_packages_check_latest_other_versions_and_every_copy() {
        let f = Fixture::new();
        f.package("A.Versioned.1", &["A.Versioned.latest"]);
        f.package("A.Versioned.2", &[]);
        f.package("B.OtherVersion.2", &["B.OtherVersion.1"]);
        f.package("C.Copies.1", &[]);
        f.package_at("Sub", "C.Copies.1", &["Missing.Dependency.1"], serde_json::json!({}));
        let cat = build_catalog(&f.db, &f.ctx, false, &AtomicBool::new(false), &mut |_, _, _| {}).unwrap();
        assert!(!package_health(&cat).iter().any(|h| h.standalone));
    }
    #[test]
    fn standalone_packages_are_not_inferred_from_quick_or_incomplete_checks() {
        let f = Fixture::new();
        f.package("A.Complete.1", &[]);
        let quick = build_catalog(&f.db, &f.ctx, true, &AtomicBool::new(false), &mut |_, _, _| {}).unwrap();
        assert!(!package_health(&quick).iter().any(|h| h.standalone));
        let mut unsafe_ctx = f.ctx.clone();
        unsafe_ctx.shared = true;
        let unsafe_cat = build_catalog(&f.db, &unsafe_ctx, false, &AtomicBool::new(false), &mut |_, _, _| {}).unwrap();
        assert!(!package_health(&unsafe_cat).iter().any(|h| h.standalone));
        f.write("Saves/scene/unreadable.json", br#"{"path":"unfinished"#);
        let incomplete = build_catalog(&f.db, &f.ctx, false, &AtomicBool::new(false), &mut |_, _, _| {}).unwrap();
        assert!(!incomplete.warnings.is_empty());
        assert!(!package_health(&incomplete).iter().any(|h| h.standalone));
    }
    #[test]
    fn packages_with_malformed_package_info_can_be_cleaned_up_and_restored() {
        let f = Fixture::new();
        // Shapes reported by the scan: a missing separator and a truncated object.
        let missing_comma = f.raw_package("A1X.Violet.2", &[
            ("meta.json", br#"{"creatorName":"A1X" "dependencies":{}}"#),
            ("Saves/scene/a.json", br#"{"id":"A1X.Violet.2:/Custom/a.vap"}"#),
        ]);
        let truncated = f.raw_package("Damarmau.Lilys_Throne.3", &[("meta.json", br#"{"dependencies":{"#)]);
        let plan = f.plan(&["A1X.Violet.2", "Damarmau.Lilys_Throne.3"], false);
        assert!(plan.preview.warnings.is_empty(), "{:?}", plan.preview.warnings);
        assert_eq!(plan.preview.items.len(), 2);
        assert!(!plan.preview.requires_confirmation);
        let batch = f.run(plan);
        assert_eq!(batch.status, "completed");
        assert!(!missing_comma.exists() && !truncated.exists());
        assert_eq!(restore(&f.db, &f.dir.join("data"), &f.ctx, &batch.id).unwrap().status, "restored");
        assert!(missing_comma.exists() && truncated.exists());
    }
    #[test]
    fn unreadable_selected_packages_restore_bytes_and_prior_records_without_reparsing() {
        let f = Fixture::new();
        let a = f.package("A.Scene.1", &[]);
        fs::write(&a, b"PKxxbroken after indexing").unwrap();
        let plan = removal_plan(&f, &["A.Scene.1"], false, false);
        let batch = run_confirmed(&f, plan).unwrap();
        assert_eq!(batch.status, "completed");
        assert!(!a.exists());
        let restored = restore(&f.db, &f.dir.join("data"), &f.ctx, &batch.id).unwrap();
        assert_eq!(restored.status, "restored", "{:?}", restored.errors);
        assert_eq!(fs::read(&a).unwrap(), b"PKxxbroken after indexing");
        assert_eq!(f.count(), 1);
        f.db.with_conn(|c| {
            assert_eq!(c.query_row("SELECT scan_status FROM physical_packages WHERE package_id='A.Scene.1'", [], |r| r.get::<_, String>(0))?, "failed");
            Ok(())
        }).unwrap();
        let b = f.write("AddonPackages/B.Unindexed.1.var", b"PKxxbroken");
        let batch = run_confirmed(&f, removal_plan(&f, &["B.Unindexed.1"], false, false)).unwrap();
        assert_eq!(restore(&f.db, &f.dir.join("data"), &f.ctx, &batch.id).unwrap().status, "restored");
        assert_eq!(fs::read(b).unwrap(), b"PKxxbroken");
        assert_eq!(f.count(), 1);
    }
    #[test]
    #[ignore = "uses VAM_CLEANUP_TEST_ROOT and VAM_CLEANUP_CACHE_DB; original library/cache are read-only"]
    fn reported_cleanup_library_preview_is_read_only() {
        let f = Fixture::new();
        let root = files::canonical_dir(Path::new(&std::env::var("VAM_CLEANUP_TEST_ROOT").unwrap())).unwrap();
        let root = root.to_string_lossy().into_owned();
        let ctx = Context { library_key: files::path_key(&root), vam_root: root.clone(),
            roots: vec![Path::new(&root).join("AddonPackages").to_string_lossy().into_owned()], managed: false, shared: false };
        let source = rusqlite::Connection::open_with_flags(std::env::var("VAM_CLEANUP_CACHE_DB").unwrap(), rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY).unwrap();
        let mut stmt = source.prepare("SELECT path_key,version,payload FROM cleanup_reference_cache WHERE library_key=?1").unwrap();
        let entries: Vec<(String, i64, String)> = stmt.query_map([&ctx.library_key], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?))).unwrap().collect::<rusqlite::Result<_>>().unwrap();
        f.db.with_conn(|c| {
            let tx = c.unchecked_transaction()?;
            for (path, version, payload) in entries { tx.execute("INSERT INTO cleanup_reference_cache(library_key,path_key,version,payload) VALUES(?1,?2,?3,?4)", params![ctx.library_key,path,version,payload])?; }
            tx.commit()?; Ok(())
        }).unwrap();
        let cancel = AtomicBool::new(false);
        let cat = build_catalog(&f.db, &ctx, false, &cancel, &mut |_, _, _| {}).unwrap();
        assert!(cat.warnings.is_empty(), "{:?}", cat.warnings);
        assert_eq!(cat.invalid_files.len(), 3);
        let selected = std::env::var("VAM_CLEANUP_SELECTED_ID").unwrap();
        let safe = make_plan_from_catalog(&f.db, ctx.clone(), &cat, "real-safe".into(), vec![selected.clone()], true, vec![], &cancel).unwrap();
        let explicit = make_plan_with_removal_choice(&f.db, ctx, &cat, "real-explicit".into(), vec![selected.clone()], true, vec![], &[selected], &cancel).unwrap();
        assert!(!explicit.preview.items.is_empty());
        eprintln!("Read-only library preview: {} packages checked, {} incomplete checks, {} invalid downloads; safe candidates {}, explicit candidates {}", cat.packages.len(), cat.warnings.len(), cat.invalid_files.len(), safe.preview.items.len(), explicit.preview.items.len());
        // Intentionally never execute either plan against original files.
    }
}
