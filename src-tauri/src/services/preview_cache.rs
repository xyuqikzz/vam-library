//! Immutable, instance-scoped previews. A cache hit reads metadata, never the VAR.
use std::{fs, io::Write, path::{Path, PathBuf}, sync::{Condvar, LazyLock, Mutex, RwLock}, time::UNIX_EPOCH};
use serde::{de::DeserializeOwned, Serialize};
use sha2::{Digest, Sha256};

pub const DIRECTORY: &str = "thumbnails-v10";
static ACCESS: RwLock<()> = RwLock::new(());
static KEYS: LazyLock<Vec<Mutex<()>>> = LazyLock::new(|| (0..64).map(|_| Mutex::new(())).collect());

// Only cache misses acquire a slot. All windows and image commands share it.
static EXTRACTIONS: (Mutex<usize>, Condvar) = (Mutex::new(0), Condvar::new());
struct ExtractionSlot;
impl ExtractionSlot {
    fn acquire() -> Result<Self, String> {
        let mut active = EXTRACTIONS.0.lock().map_err(|e| e.to_string())?;
        while *active >= 4 {
            active = EXTRACTIONS.1.wait(active).map_err(|e| e.to_string())?;
        }
        *active += 1;
        Ok(Self)
    }
}
impl Drop for ExtractionSlot {
    fn drop(&mut self) {
        if let Ok(mut active) = EXTRACTIONS.0.lock() {
            *active -= 1;
            EXTRACTIONS.1.notify_one();
        }
    }
}

pub struct PreviewCache {
    directory: PathBuf,
    source: PathBuf,
    logical_key: String,
    key: String,
    cacheable: bool,
}

fn key(source: &Path, logical_key: &str) -> Result<(String, bool), String> {
    let meta = fs::metadata(source).map_err(|e| e.to_string())?;
    let modified = meta.modified().map_err(|e| e.to_string())?.duration_since(UNIX_EPOCH).map_err(|e| e.to_string())?;
    let mut hash = Sha256::new();
    hash.update(source.to_string_lossy().as_bytes());
    hash.update([0]);
    hash.update(logical_key.as_bytes());
    hash.update(meta.len().to_le_bytes());
    hash.update(modified.as_nanos().to_le_bytes());
    // Reuse the metadata-only file ID/change-time query used by resource checks.
    // Size + mtime alone miss replacements that preserve archive timestamps.
    let stamp = super::cleanup_reference_cache::change_stamp(source)?;
    if let Some(stamp) = &stamp { hash.update(stamp.as_bytes()); }
    Ok((hex::encode(hash.finalize()), stamp.is_some()))
}

impl PreviewCache {
    pub fn new(directory: &Path, source: &Path, logical_key: &str) -> Result<Self, String> {
        let (key, cacheable) = key(source, logical_key)?;
        Ok(Self { directory: directory.into(), source: source.into(), logical_key: logical_key.into(), key, cacheable })
    }

    fn manifest(&self) -> PathBuf { self.directory.join(format!("{}.json", self.key)) }

    fn read<T: DeserializeOwned>(&self) -> Result<Option<T>, String> {
        // Without a reliable change stamp, re-read the source instead of treating
        // a possibly stale positive or negative entry as valid.
        if !self.cacheable { return Ok(None); }
        match fs::read(self.manifest()) {
            Ok(bytes) => Ok(serde_json::from_slice(&bytes).ok()), // An interrupted/old manifest is a miss.
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(e) => Err(e.to_string()),
        }
    }

    fn write(&self, path: &Path, bytes: &[u8]) -> Result<(), String> {
        fs::create_dir_all(&self.directory).map_err(|e| e.to_string())?;
        let temporary = path.with_extension("pending");
        let mut file = fs::File::create(&temporary).map_err(|e| e.to_string())?;
        file.write_all(bytes).map_err(|e| e.to_string())?;
        drop(file);
        fs::rename(&temporary, path).map_err(|e| e.to_string())
    }

    fn unchanged(&self) -> Result<(), String> {
        if self.key == key(&self.source, &self.logical_key)?.0 { Ok(()) }
        else { Err("资源已改变，请刷新预览".into()) }
    }

    fn read_image(&self) -> Result<Option<Option<String>>, String> {
        let Some(extension) = self.read::<Option<String>>()? else { return Ok(None); };
        let Some(extension) = extension else { return Ok(Some(None)); };
        if !matches!(extension.as_str(), "jpg" | "jpeg" | "png" | "webp" | "bmp" | "gif") { return Ok(None); }
        let path = self.directory.join(format!("{}.{}", self.key, extension));
        Ok(path.is_file().then(|| Some(path.to_string_lossy().into_owned())))
    }

    /// Outer None = not cached; Some(None) = a persistent no-image result.
    pub fn lookup(&self) -> Result<Option<Option<String>>, String> {
        let _access = ACCESS.read().map_err(|e| e.to_string())?;
        self.read_image()
    }

    pub fn image(&self, extract: impl FnOnce() -> Result<Option<(String, Vec<u8>)>, String>) -> Result<Option<String>, String> {
        let _access = ACCESS.read().map_err(|e| e.to_string())?;
        let _key = KEYS[self.key.as_bytes()[0] as usize % KEYS.len()].lock().map_err(|e| e.to_string())?;
        if let Some(cached) = self.read_image()? { return Ok(cached); }
        let image = {
            let slot = {
                let _timer = crate::services::perf::CommandTimer::new("thumbnails.extract_wait");
                ExtractionSlot::acquire()?
            };
            let _slot = slot;
            let _timer = crate::services::perf::CommandTimer::new("thumbnails.extract");
            extract()?
        };
        self.unchanged()?;
        let extension = image.as_ref().map(|(name, _)| Path::new(name).extension().and_then(|e| e.to_str()).unwrap_or("").to_ascii_lowercase());
        if let (Some((_, bytes)), Some(ext)) = (&image, &extension) {
            if !matches!(ext.as_str(), "jpg" | "jpeg" | "png" | "webp" | "bmp" | "gif") { return Err("不支持的预览图片格式".into()); }
            self.write(&self.directory.join(format!("{}.{}", self.key, ext)), bytes)?;
        }
        self.write(&self.manifest(), &serde_json::to_vec(&extension).map_err(|e| e.to_string())?)?;
        Ok(extension.map(|ext| self.directory.join(format!("{}.{}", self.key, ext)).to_string_lossy().into_owned()))
    }

    pub fn json<T: Serialize + DeserializeOwned>(&self, load: impl FnOnce() -> Result<T, String>) -> Result<T, String> {
        let _access = ACCESS.read().map_err(|e| e.to_string())?;
        let _key = KEYS[self.key.as_bytes()[0] as usize % KEYS.len()].lock().map_err(|e| e.to_string())?;
        if let Some(value) = self.read()? { return Ok(value); }
        let value = load()?;
        self.unchanged()?;
        self.write(&self.manifest(), &serde_json::to_vec(&value).map_err(|e| e.to_string())?)?;
        Ok(value)
    }
}

pub fn clear(app_data: &Path) -> Result<(), String> {
    let _access = ACCESS.write().map_err(|e| e.to_string())?;
    // Only the application's named thumbnail directories are disposable.
    for name in [DIRECTORY, "thumbnails-v9"] {
        let path = app_data.join(name);
        crate::services::resource_files::ensure_no_links(&path)?;
        if path.exists() { fs::remove_dir_all(path).map_err(|e| e.to_string())?; }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    static PREVIEW_TEST: Mutex<()> = Mutex::new(());

    #[test]
    fn preserved_size_and_mtime_invalidate_previews_and_unsupported_stamps_do_not_reuse() {
        let _test = PREVIEW_TEST.lock().unwrap();
        let dir = crate::services::resource_files::tests::TestDir::new();
        let source = dir.0.join("A.B.1.var");
        fs::write(&source, b"first").unwrap();
        let modified = fs::metadata(&source).unwrap().modified().unwrap();
        let old = PreviewCache::new(&dir.0.join("cache"), &source, "cover:A.B.1").unwrap();
        old.image(|| Ok(Some(("cover.jpg".into(), b"old image".to_vec())))).unwrap();
        std::thread::sleep(std::time::Duration::from_millis(5));
        fs::write(&source, b"other").unwrap();
        fs::File::options().write(true).open(&source).unwrap().set_modified(modified).unwrap();
        let new = PreviewCache::new(&dir.0.join("cache"), &source, "cover:A.B.1").unwrap();
        assert!(new.lookup().unwrap().is_none());
        // Simulate filesystems without trustworthy metadata and preserve the
        // normal negative-cache behavior when trustworthy metadata is available.
        let mut unsupported = new;
        unsupported.cacheable = false;
        unsupported.image(|| Ok(None)).unwrap();
        assert!(unsupported.lookup().unwrap().is_none());
        let mut extracted = false;
        unsupported.image(|| { extracted = true; Ok(None) }).unwrap();
        assert!(extracted);
    }

    #[test]
    fn warm_hits_bypass_extraction_limit_and_failures_release_slots() {
        let _test = PREVIEW_TEST.lock().unwrap();
        let dir = crate::services::resource_files::tests::TestDir::new();
        let source = dir.0.join("A.B.1.var");
        fs::write(&source, b"fixture").unwrap();
        let cache = PreviewCache::new(&dir.0.join("cache"), &source, "cover:A.B.1").unwrap();
        assert!(cache.image(|| Err("temporary failure".into())).is_err());
        assert_eq!(cache.lookup().unwrap(), None, "failure must not become a negative cache entry");
        assert_eq!(cache.image(|| Ok(None)).unwrap(), None);
        let slots: Vec<_> = (0..4).map(|_| ExtractionSlot::acquire().unwrap()).collect();
        let (send, receive) = std::sync::mpsc::channel();
        let worker = std::thread::spawn(move || {
            send.send(cache.image(|| panic!("a warm hit must not extract"))).unwrap();
        });
        let warm = receive.recv_timeout(std::time::Duration::from_secs(2));
        drop(slots);
        worker.join().unwrap();
        assert_eq!(warm.unwrap().unwrap(), None);
    }

    #[test]
    fn extraction_limit_is_shared_between_workers() {
        let _test = PREVIEW_TEST.lock().unwrap();
        use std::sync::atomic::{AtomicUsize, Ordering};
        let active = AtomicUsize::new(0);
        let peak = AtomicUsize::new(0);
        let start = std::sync::Barrier::new(12);
        std::thread::scope(|scope| {
            for _ in 0..12 {
                scope.spawn(|| {
                    start.wait();
                    let _slot = ExtractionSlot::acquire().unwrap();
                    let current = active.fetch_add(1, Ordering::SeqCst) + 1;
                    peak.fetch_max(current, Ordering::SeqCst);
                    std::thread::sleep(std::time::Duration::from_millis(5));
                    active.fetch_sub(1, Ordering::SeqCst);
                });
            }
        });
        assert!(peak.load(Ordering::SeqCst) <= 4);
        assert_eq!(active.load(Ordering::SeqCst), 0);
    }
    #[test]
    fn exact_keys_negative_results_replacement_and_clear() {
        let _test = PREVIEW_TEST.lock().unwrap();
        let root = std::env::temp_dir().join(format!("vam-preview-{}-{}", std::process::id(), chrono::Utc::now().timestamp_nanos_opt().unwrap()));
        fs::create_dir_all(&root).unwrap();
        let source = root.join("A.B.1.var");
        let other = root.join("A.B.10.var");
        fs::write(&source, b"first").unwrap(); fs::write(&other, b"second").unwrap();
        let dir = root.join(DIRECTORY);
        let first = PreviewCache::new(&dir, &source, "cover:A.B.1").unwrap();
        let second = PreviewCache::new(&dir, &other, "cover:A.B.10").unwrap();
        assert_eq!(first.lookup().unwrap(), None);
        assert_eq!(first.image(|| Ok(None)).unwrap(), None);
        assert_eq!(first.lookup().unwrap(), Some(None));
        first.image(|| panic!("a negative hit must never open the archive")).unwrap();
        let image = second.image(|| Ok(Some(("preview.jpg".into(), b"image".to_vec())))).unwrap().unwrap();
        assert_eq!(fs::read(&image).unwrap(), b"image");
        second.image(|| panic!("a positive hit must never open the archive")).unwrap();
        fs::write(&source, b"replacement with a different size").unwrap();
        assert_eq!(PreviewCache::new(&dir, &source, "cover:A.B.1").unwrap().lookup().unwrap(), None);
        let before = PreviewCache::new(&dir, &source, "cover:A.B.1").unwrap();
        let now = fs::metadata(&source).unwrap().modified().unwrap() + std::time::Duration::from_secs(1);
        fs::File::options().write(true).open(&source).unwrap().set_modified(now).unwrap();
        assert_ne!(before.key, PreviewCache::new(&dir, &source, "cover:A.B.1").unwrap().key);
        clear(&root).unwrap();
        assert!(!dir.exists()); assert!(source.exists()); assert!(other.exists());
        fs::remove_dir_all(root).unwrap();
    }
}
