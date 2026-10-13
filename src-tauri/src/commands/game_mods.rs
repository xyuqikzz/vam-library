use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    fs,
    io::{Read, Seek, SeekFrom, Write},
    path::{Path, PathBuf},
    sync::Mutex,
};

const PAYLOAD: &[u8] = include_bytes!("../../resources/mods/VamLibrary.SceneBrowser.dll");
const MANIFEST: &str = include_str!("../../resources/mods/scene-browser.json");
const TARGET: &str = "BepInEx/plugins/VamLibrary.SceneBrowser/VamLibrary.SceneBrowser.dll";
// Frozen legacy releases. New releases establish ownership with an exact-hash marker.
const PREVIOUS_SHA256: &[&str] = &[
    "ee1a3178f538047be8f1a2e7b9e6134ad32820eaf9983e9550a33f8469040e40",
    "51e6ef72ee53f657347706990a515c0d88ea260e7b772a6cc1679fd2bcf4aea8",
    "4900b9c7e4c87e5fd58d2a2b9c0b974952f0fa0dff8fbb667b5cf9d106c08b04",
    "32257830cec3e6e507bd33fcd9c71b259798d51eef471c227be2a973d791a5b0",
    "8289dbe2e8a0d582f8b74e1d0a66b4c1fb346f728168344bf04e51418f3da984",
    "8916ec27265e27563a1012c9bb7504b039c6a0a0f4a475d996054c71ad5fa44e",
    "b4043bd7d4da33c89f5d1f64d1c7ea57d763439b28c5ba6e7e52be66d94c4fad",
    "ebc0e87863aee1a539e8ad773d3d9c2a8fa8159c94326474b4844ef3b8e879d4",
    // 1.0.9, the last release with native VAR decompression.
    "1ae5fc781757779c043b432774eefdd64e9f0a3ed7a077e6eedea767b4af1da3",
];
// These 1.0.8 builds differ only in the legacy compiler's timestamp, MVID and
// MVID-derived private type name. Keep exact hashes; never trust a version label.
const V1_0_8_SHA256: &[&str] = &[
    "a559c0bf87f14fd09e87f9237f99625eb08636e0f4d94eb4d93066d12c9a1da5",
    "b341d26e28dd5977dfed2a44af4e505b070809bd323484f468194b51a8c975f5",
];
static MOD_LOCK: Mutex<()> = Mutex::new(());

#[derive(Deserialize)]
struct Manifest {
    version: String,
    files: Vec<Dependency>,
}
#[derive(Deserialize)]
struct Dependency {
    path: String,
    sha256: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ModStatus {
    version: String,
    installed: bool,
    update_available: bool,
    conflict: bool,
    compatible: bool,
    verified: bool,
    runtime: Option<RuntimeStatus>,
    reason: Option<String>,
    target_path: String,
}

#[derive(Serialize)]
pub struct RuntimeStatus { browser: String, launch: String }
const OWNER: &str = "VamLibrary.SceneBrowser.owner";

fn data_path(target: &Path, name: &str) -> Result<PathBuf, String> {
    let path = target.parent().ok_or("无效的插件目录")?.join(name);
    match fs::symlink_metadata(&path) {
        Ok(meta) => {
            #[cfg(windows)]
            let linked = { use std::os::windows::fs::MetadataExt; meta.file_attributes() & 0x400 != 0 };
            #[cfg(not(windows))]
            let linked = meta.file_type().is_symlink();
            if linked || !meta.is_file() { return Err("插件记录路径不是普通文件".into()); }
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {},
        Err(e) => return Err(e.to_string()),
    }
    Ok(path)
}

fn marked_owner(target: &Path, bytes: &[u8]) -> bool {
    let Ok(path) = data_path(target, OWNER) else { return false; };
    if fs::metadata(&path).map_or(true, |m| m.len() > 128) { return false; }
    fs::read_to_string(path).ok().is_some_and(|text| text.trim_end() == format!("sha256\t{}", hash(bytes)))
}
fn owned(target: &Path, bytes: &[u8]) -> bool {
    matches_payload(bytes, PAYLOAD) || is_known_release(bytes) || marked_owner(target, bytes)
}
fn write_owner(target: &Path) -> Result<(), String> {
    let owner = data_path(target, OWNER)?;
    let temp = data_path(target, &format!("VamLibrary.SceneBrowser.owner-{}-{}.tmp", std::process::id(), chrono::Utc::now().timestamp_nanos_opt().unwrap_or_default()))?;
    let mut file = fs::OpenOptions::new().write(true).create_new(true).open(&temp).map_err(|e| e.to_string())?;
    let result = (|| {
        writeln!(file, "sha256\t{}", hash(PAYLOAD))?;
        file.sync_all()?; drop(file);
        fs::rename(&temp, owner)
    })();
    if result.is_err() { let _ = fs::remove_file(temp); }
    result.map_err(|e| e.to_string())
}
fn runtime_status(target: &Path, version: &str) -> Option<RuntimeStatus> {
    let path = data_path(target, "plugin-status.tsv").ok()?;
    if fs::metadata(&path).ok()?.len() > 4096 { return None; }
    let text = fs::read_to_string(path).ok()?;
    let lines: Vec<_> = text.lines().collect();
    if lines.len() != 4 || lines[0] != "# VAM Library plugin status v1" || lines[1] != format!("version\t{version}") { return None; }
    let browser = lines[2].strip_prefix("browser\t")?;
    let launch = lines[3].strip_prefix("launch\t")?;
    if !["ok", "unavailable"].contains(&browser) || !["ok", "unavailable"].contains(&launch) { return None; }
    Some(RuntimeStatus { browser: browser.into(), launch: launch.into() })
}

fn hash(bytes: &[u8]) -> String {
    hex::encode(Sha256::digest(bytes))
}

fn matches_payload(bytes: &[u8], payload: &[u8]) -> bool {
    bytes == payload
        || (V1_0_8_SHA256.contains(&hash(payload).as_str())
            && V1_0_8_SHA256.contains(&hash(bytes).as_str()))
}

fn is_known_release(bytes: &[u8]) -> bool {
    let digest = hash(bytes);
    PREVIOUS_SHA256.contains(&digest.as_str()) || V1_0_8_SHA256.contains(&digest.as_str())
}

// Do not follow a junction/symlink in the installation destination, even if a
// parent currently resolves inside the root. A game root itself may be a junction.
fn safe_target(root: &Path) -> Result<PathBuf, String> {
    let mut current = root.to_path_buf();
    for part in TARGET.split('/') {
        current.push(part);
        match fs::symlink_metadata(&current) {
            Ok(meta) => {
                #[cfg(windows)]
                let linked = {
                    use std::os::windows::fs::MetadataExt;
                    meta.file_attributes() & 0x400 != 0
                };
                #[cfg(not(windows))]
                let linked = meta.file_type().is_symlink();
                if linked {
                    return Err(format!(
                        "安装路径包含链接，请使用普通目录: {}",
                        current.display()
                    ));
                }
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => return Err(e.to_string()),
        }
    }
    Ok(current)
}

fn checked_root(root: &str) -> Result<PathBuf, String> {
    let root = fs::canonicalize(root).map_err(|e| format!("无法读取游戏目录: {e}"))?;
    if !root.join("VaM.exe").is_file() || !root.join("AddonPackages").is_dir() {
        return Err("请选择包含 VaM.exe 和 AddonPackages 的游戏根目录".into());
    }
    Ok(root)
}

pub(crate) fn owned_mod_directory(root: &Path) -> Result<Option<PathBuf>, String> {
    let target = safe_target(root)?;
    let bytes = match fs::read(&target) {
        Ok(bytes) => bytes,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(error.to_string()),
    };
    if !owned(&target, &bytes) {
        return Ok(None);
    }
    Ok(target.parent().map(Path::to_path_buf))
}

pub(crate) fn scene_launch_directory(root: &Path) -> Result<PathBuf, String> {
    let status = inspect(root)?;
    if !status.installed {
        return Err("请先退出 VaM，在设置中安装或更新 VaM 管理增强插件，再启动场景".into());
    }
    if !status.compatible {
        return Err(status.reason.unwrap_or_default());
    }
    if status.runtime.as_ref().is_some_and(|s| s.launch == "unavailable") {
        return Err("launch_unavailable".into());
    }
    Ok(safe_target(root)?
        .parent()
        .ok_or("无效模组路径")?
        .join("scene-launch"))
}

fn inspect(root: &Path) -> Result<ModStatus, String> {
    let manifest: Manifest = serde_json::from_str(MANIFEST).map_err(|e| e.to_string())?;
    let target = safe_target(root)?;
    let existing = match fs::read(&target) {
        Ok(bytes) => Some(bytes),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => None,
        Err(e) => return Err(e.to_string()),
    };
    let installed = existing
        .as_deref()
        .is_some_and(|bytes| matches_payload(bytes, PAYLOAD));
    let update_available = !installed && existing.as_deref().is_some_and(|bytes| owned(&target, bytes));
    let conflict = existing.is_some() && !installed && !update_available;
    let mut reason = None;
    let mut verified = true;
    for dependency in manifest.files {
        match fs::read(root.join(&dependency.path)) {
            Ok(bytes) if hash(&bytes) == dependency.sha256 => {}
            Ok(_) => { verified = false; }
            Err(_) => {
                reason = Some(format!(
                    "缺少 {}；请先安装兼容的 BepInEx 5",
                    dependency.path
                ));
                break;
            }
        }
    }
    Ok(ModStatus {
        runtime: if installed { runtime_status(&target, &manifest.version) } else { None },
        version: manifest.version,
        installed,
        update_available,
        conflict,
        verified: verified && reason.is_none(),
        compatible: reason.is_none(),
        reason,
        target_path: target.to_string_lossy().into_owned(),
    })
}

fn ensure_game_stopped(root: &Path) -> Result<(), String> {
    if crate::services::game_process::game_running(root)? { return Err("请先退出当前游戏目录的 VaM，再安装或卸载插件".into()); }
    Ok(())
}

fn install(root: &Path, allow_unverified: bool) -> Result<ModStatus, String> {
    let status = inspect(root)?;
    if status.conflict {
        return Err("同名 DLL 已存在且内容不同，未覆盖；请先移开该文件".into());
    }
    if !status.compatible {
        return Err(status.reason.unwrap_or_default());
    }
    if !status.verified && !allow_unverified { return Err("unverified_game".into()); }
    data_path(&safe_target(root)?, OWNER)?;
    if status.installed {
        write_owner(&safe_target(root)?)?;
        return Ok(status);
    }
    let target = safe_target(root)?;
    if status.update_available {
        upgrade_known_version(&target)?;
        write_owner(&target)?;
        let result = inspect(root)?;
        if !result.installed {
            return Err("更新后的 DLL 校验失败".into());
        }
        return Ok(result);
    }
    let parent = target.parent().ok_or("无效的安装路径")?;
    fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    safe_target(root)?;
    // create_new guarantees an existing file cannot be overwritten.
    let mut file = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&target)
        .map_err(|e| e.to_string())?;
    let written = file.write_all(PAYLOAD).and_then(|_| file.sync_all());
    drop(file);
    if let Err(e) = written {
        let cleanup = fs::remove_file(&target);
        return Err(format!("写入模组失败: {e}; 清理不完整文件: {cleanup:?}"));
    }
    write_owner(&target)?;
    let result = inspect(root)?;
    if !result.installed {
        return Err("安装后的 DLL 校验失败".into());
    }
    Ok(result)
}

fn upgrade_known_version(target: &Path) -> Result<(), String> {
    let mut options = fs::OpenOptions::new();
    options.read(true).write(true);
    #[cfg(windows)]
    {
        use std::os::windows::fs::OpenOptionsExt;
        options.share_mode(0);
    }
    let mut file = options.open(target).map_err(|e| e.to_string())?;
    let mut original = Vec::new();
    file.read_to_end(&mut original).map_err(|e| e.to_string())?;
    if !owned(target, &original) {
        return Err("旧版 DLL 在检查后发生变化，已取消更新".into());
    }
    let write = |file: &mut fs::File, bytes: &[u8]| -> std::io::Result<()> {
        file.seek(SeekFrom::Start(0))?;
        file.write_all(bytes)?;
        file.set_len(bytes.len() as u64)?;
        file.sync_all()
    };
    if let Err(e) = write(&mut file, PAYLOAD) {
        return match write(&mut file, &original) {
            Ok(()) => Err(format!("更新失败，已恢复原 DLL: {e}")),
            Err(restore) => Err(format!(
                "更新失败: {e}；恢复失败: {restore}，请重新安装模组"
            )),
        };
    }
    Ok(())
}

fn uninstall(root: &Path) -> Result<ModStatus, String> {
    let status = inspect(root)?;
    if status.conflict {
        return Err("DLL 内容与本软件附带版本不同，未删除".into());
    }
    if status.installed || status.update_available {
        let target = safe_target(root)?;
        let owner = data_path(&target, OWNER)?;
        fs::remove_file(target).map_err(|e| e.to_string())?;
        match fs::remove_file(owner) { Ok(()) => {}, Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}, Err(e) => return Err(e.to_string()) }

    }
    inspect(root)
}

#[tauri::command]
pub async fn get_scene_browser_mod_status(vam_root: String) -> Result<ModStatus, String> {
    tauri::async_runtime::spawn_blocking(move || inspect(&checked_root(&vam_root)?))
        .await
        .map_err(|e| e.to_string())?
}

#[tauri::command]
pub async fn install_scene_browser_mod(
    app_handle: tauri::AppHandle,
    vam_root: String,
    allow_unverified: Option<bool>,
) -> Result<ModStatus, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let _guard = MOD_LOCK.lock().map_err(|e| e.to_string())?;
        let root = checked_root(&vam_root)?;
        ensure_game_stopped(&root)?;
        let result = install(&root, allow_unverified.unwrap_or(false))?;
        crate::services::scene_browser_index::schedule_sync(&app_handle);
        Ok(result)
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
pub async fn uninstall_scene_browser_mod(vam_root: String) -> Result<ModStatus, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let _guard = MOD_LOCK.lock().map_err(|e| e.to_string())?;
        let root = checked_root(&vam_root)?;
        ensure_game_stopped(&root)?;
        uninstall(&root)
    })
    .await
    .map_err(|e| e.to_string())?
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU64, Ordering};
    static NEXT_FIXTURE: AtomicU64 = AtomicU64::new(0);
    struct Fixture(PathBuf);
    impl Fixture {
        fn new() -> Self {
            let path = std::env::temp_dir().join(format!(
                "vam-scene-mod-{}-{}-{}",
                std::process::id(),
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_nanos(),
                NEXT_FIXTURE.fetch_add(1, Ordering::Relaxed)
            ));
            fs::create_dir_all(&path).unwrap();
            Self(path)
        }
    }
    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn exact_owner_marker_allows_future_releases_but_modified_dll_is_protected() {
        let f = Fixture::new();
        let target = f.0.join(TARGET);
        fs::create_dir_all(target.parent().unwrap()).unwrap();
        let previous = b"future deterministic release";
        fs::write(&target, previous).unwrap();
        let marker = target.parent().unwrap().join(OWNER);
        fs::write(&marker, format!("sha256\t{}\n", hash(previous))).unwrap();
        assert!(inspect(&f.0).unwrap().update_available);
        assert!(owned_mod_directory(&f.0).unwrap().is_some());
        fs::write(&target, b"modified DLL").unwrap();
        assert!(inspect(&f.0).unwrap().conflict);
        assert!(uninstall(&f.0).is_err());
        assert!(upgrade_known_version(&target).is_err());
        fs::write(&target, previous).unwrap();
        upgrade_known_version(&target).unwrap();
        write_owner(&target).unwrap();
        assert_eq!(fs::read_to_string(&marker).unwrap(), format!("sha256\t{}\n", hash(PAYLOAD)));
        uninstall(&f.0).unwrap();
        assert!(!marker.exists());
    }

    #[test]
    fn untested_fingerprints_require_explicit_consent_and_runtime_is_versioned() {
        let f = Fixture::new();
        let manifest: Manifest = serde_json::from_str(MANIFEST).unwrap();
        for dependency in manifest.files {
            let path = f.0.join(dependency.path);
            fs::create_dir_all(path.parent().unwrap()).unwrap();
            fs::write(path, b"untested dependency fixture").unwrap();
        }
        let status = inspect(&f.0).unwrap();
        assert!(status.compatible && !status.verified);
        assert_eq!(install(&f.0, false).err().as_deref(), Some("unverified_game"));
        assert!(!f.0.join(TARGET).exists());
        assert!(install(&f.0, true).unwrap().installed);
        let path = f.0.join(TARGET).parent().unwrap().join("plugin-status.tsv");
        fs::write(&path, "# VAM Library plugin status v1\nversion\t0.0.0\nbrowser\tok\nlaunch\tunavailable\n").unwrap();
        assert!(inspect(&f.0).unwrap().runtime.is_none());
        fs::write(&path, format!("# VAM Library plugin status v1\nversion\t{}\nbrowser\tok\nlaunch\tunavailable\n", status.version)).unwrap();
        assert_eq!(scene_launch_directory(&f.0).unwrap_err(), "launch_unavailable");
        assert_eq!(inspect(&f.0).unwrap().runtime.unwrap().browser, "ok");
        fs::remove_file(path).unwrap();
        assert!(scene_launch_directory(&f.0).is_ok());
    }

    #[test]
    fn missing_dependencies_do_not_write_mod() {
        let f = Fixture::new();
        assert!(!inspect(&f.0).unwrap().compatible);
        assert!(install(&f.0, false).is_err());
        assert!(!f.0.join(TARGET).exists());
    }
    #[test]
    fn foreign_dll_is_neither_overwritten_nor_removed() {
        let f = Fixture::new();
        let target = f.0.join(TARGET);
        fs::create_dir_all(target.parent().unwrap()).unwrap();
        fs::write(&target, b"user mod").unwrap();
        assert!(inspect(&f.0).unwrap().conflict);
        assert!(install(&f.0, false).is_err());
        assert!(uninstall(&f.0).is_err());
        assert_eq!(fs::read(target).unwrap(), b"user mod");
    }
    #[test]
    fn own_dll_can_be_removed_without_touching_resources() {
        let f = Fixture::new();
        let target = f.0.join(TARGET);
        fs::create_dir_all(target.parent().unwrap()).unwrap();
        fs::write(&target, PAYLOAD).unwrap();
        fs::write(f.0.join("scene.var"), b"resource").unwrap();
        assert!(inspect(&f.0).unwrap().installed);
        assert!(!uninstall(&f.0).unwrap().installed);
        assert_eq!(fs::read(f.0.join("scene.var")).unwrap(), b"resource");
    }

    #[test]
    fn bundled_payload_is_newer_than_every_previous_release() {
        // Fails when mod sources changed but mods/SceneBrowser/build.ps1 was not rerun.
        assert!(
            !is_known_release(PAYLOAD),
            "bundled DLL is a previous release; rebuild the mod"
        );
        let v1_0_9 =
            include_bytes!("../../../mods/SceneBrowser/tests/fixtures/SceneBrowser-1.0.9.dll");
        assert_eq!(hash(v1_0_9), PREVIOUS_SHA256[8]);
    }

    #[test]
    fn verified_previous_rebuild_is_upgradable_and_owned() {
        let f = Fixture::new();
        let target = f.0.join(TARGET);
        let rebuild = include_bytes!(
            "../../../mods/SceneBrowser/tests/fixtures/SceneBrowser-1.0.8-rebuild.dll"
        );
        assert_eq!(hash(rebuild), V1_0_8_SHA256[1]);
        assert!(!matches_payload(rebuild, PAYLOAD));
        // A future payload must offer an upgrade, not treat 1.0.8 as current.
        assert!(!matches_payload(rebuild, b"future release"));
        assert!(is_known_release(rebuild));
        fs::create_dir_all(target.parent().unwrap()).unwrap();
        fs::write(&target, rebuild).unwrap();
        let status = inspect(&f.0).unwrap();
        assert!(!status.installed && status.update_available && !status.conflict);
        assert_eq!(
            owned_mod_directory(&f.0).unwrap(),
            target.parent().map(Path::to_path_buf)
        );
        assert!(!uninstall(&f.0).unwrap().installed);
    }

    #[test]
    fn modified_rebuild_remains_protected() {
        let f = Fixture::new();
        let target = f.0.join(TARGET);
        let mut rebuild = include_bytes!(
            "../../../mods/SceneBrowser/tests/fixtures/SceneBrowser-1.0.8-rebuild.dll"
        )
        .to_vec();
        // Same assembly identity/version, but changed executable contents.
        rebuild[512] ^= 1;
        fs::create_dir_all(target.parent().unwrap()).unwrap();
        fs::write(&target, &rebuild).unwrap();
        let status = inspect(&f.0).unwrap();
        assert!(!status.installed && !status.update_available && status.conflict);
        assert!(owned_mod_directory(&f.0).unwrap().is_none());
        assert!(install(&f.0, false).is_err());
        assert!(uninstall(&f.0).is_err());
        assert!(upgrade_known_version(&target).is_err());
        assert_eq!(fs::read(target).unwrap(), rebuild);
    }

    #[test]
    #[ignore = "requires VAM_MOD_TEST_ROOT pointing to the matching local game; writes only to a temporary fixture"]
    fn installation_round_trip_with_local_game() {
        let source =
            PathBuf::from(std::env::var("VAM_MOD_TEST_ROOT").expect("Set VAM_MOD_TEST_ROOT"));
        let f = Fixture::new();
        let manifest: Manifest = serde_json::from_str(MANIFEST).unwrap();
        for dependency in manifest.files {
            let destination = f.0.join(&dependency.path);
            fs::create_dir_all(destination.parent().unwrap()).unwrap();
            fs::copy(source.join(&dependency.path), destination).unwrap();
        }
        fs::write(f.0.join("VaM.exe"), b"fixture").unwrap();
        fs::create_dir(f.0.join("AddonPackages")).unwrap();
        fs::write(f.0.join("AddonPackages/Keep.Scene.1.var"), b"keep").unwrap();
        let root = checked_root(f.0.to_str().unwrap()).unwrap();
        assert!(inspect(&root).unwrap().compatible);
        assert!(install(&root, false).unwrap().installed);
        assert_eq!(hash(&fs::read(root.join(TARGET)).unwrap()), hash(PAYLOAD));
        assert!(install(&root, false).unwrap().installed);
        assert!(!uninstall(&root).unwrap().installed);
        let previous =
            include_bytes!("../../../mods/SceneBrowser/tests/fixtures/SceneBrowser-1.0.0.dll");
        assert!(PREVIOUS_SHA256.contains(&hash(previous).as_str()));
        fs::write(root.join(TARGET), previous).unwrap();
        let old_status = inspect(&root).unwrap();
        assert!(old_status.update_available && !old_status.conflict);
        assert!(install(&root, false).unwrap().installed);
        assert_eq!(fs::read(root.join(TARGET)).unwrap(), PAYLOAD);
        assert!(!uninstall(&root).unwrap().installed);
        let previous =
            include_bytes!("../../../mods/SceneBrowser/tests/fixtures/SceneBrowser-1.0.1.dll");
        assert!(PREVIOUS_SHA256.contains(&hash(previous).as_str()));
        fs::write(root.join(TARGET), previous).unwrap();
        assert!(inspect(&root).unwrap().update_available);
        assert!(install(&root, false).unwrap().installed);
        assert_eq!(fs::read(root.join(TARGET)).unwrap(), PAYLOAD);
        assert!(!uninstall(&root).unwrap().installed);
        let previous =
            include_bytes!("../../../mods/SceneBrowser/tests/fixtures/SceneBrowser-1.0.2.dll");
        assert!(PREVIOUS_SHA256.contains(&hash(previous).as_str()));
        fs::write(root.join(TARGET), previous).unwrap();
        assert!(owned_mod_directory(&root).unwrap().is_some());
        assert!(inspect(&root).unwrap().update_available);
        assert!(install(&root, false).unwrap().installed);
        assert_eq!(fs::read(root.join(TARGET)).unwrap(), PAYLOAD);
        assert!(!uninstall(&root).unwrap().installed);
        let previous =
            include_bytes!("../../../mods/SceneBrowser/tests/fixtures/SceneBrowser-1.0.4.dll");
        assert!(PREVIOUS_SHA256.contains(&hash(previous).as_str()));
        fs::write(root.join(TARGET), previous).unwrap();
        assert!(inspect(&root).unwrap().update_available);
        assert!(install(&root, false).unwrap().installed);
        assert_eq!(fs::read(root.join(TARGET)).unwrap(), PAYLOAD);
        assert!(!uninstall(&root).unwrap().installed);
        let previous =
            include_bytes!("../../../mods/SceneBrowser/tests/fixtures/SceneBrowser-1.0.5.dll");
        assert!(PREVIOUS_SHA256.contains(&hash(previous).as_str()));
        fs::write(root.join(TARGET), previous).unwrap();
        assert!(inspect(&root).unwrap().update_available);
        assert!(install(&root, false).unwrap().installed);
        assert_eq!(fs::read(root.join(TARGET)).unwrap(), PAYLOAD);
        assert!(!uninstall(&root).unwrap().installed);
        let previous =
            include_bytes!("../../../mods/SceneBrowser/tests/fixtures/SceneBrowser-1.0.6.dll");
        assert!(PREVIOUS_SHA256.contains(&hash(previous).as_str()));
        fs::write(root.join(TARGET), previous).unwrap();
        assert!(inspect(&root).unwrap().update_available);
        assert!(install(&root, false).unwrap().installed);
        assert_eq!(fs::read(root.join(TARGET)).unwrap(), PAYLOAD);
        assert!(!uninstall(&root).unwrap().installed);
        let previous =
            include_bytes!("../../../mods/SceneBrowser/tests/fixtures/SceneBrowser-1.0.7.dll");
        assert!(PREVIOUS_SHA256.contains(&hash(previous).as_str()));
        fs::write(root.join(TARGET), previous).unwrap();
        assert!(inspect(&root).unwrap().update_available);
        assert!(install(&root, false).unwrap().installed);
        assert_eq!(fs::read(root.join(TARGET)).unwrap(), PAYLOAD);
        assert!(!uninstall(&root).unwrap().installed);
        let rebuild = include_bytes!(
            "../../../mods/SceneBrowser/tests/fixtures/SceneBrowser-1.0.8-rebuild.dll"
        );
        fs::write(root.join(TARGET), rebuild).unwrap();
        assert!(inspect(&root).unwrap().update_available);
        assert!(install(&root, false).unwrap().installed);
        assert_eq!(fs::read(root.join(TARGET)).unwrap(), PAYLOAD);
        assert!(!uninstall(&root).unwrap().installed);
        let previous =
            include_bytes!("../../../mods/SceneBrowser/tests/fixtures/SceneBrowser-1.0.9.dll");
        fs::write(root.join(TARGET), previous).unwrap();
        assert!(inspect(&root).unwrap().update_available);
        assert!(install(&root, false).unwrap().installed);
        assert_eq!(fs::read(root.join(TARGET)).unwrap(), PAYLOAD);
        // Once upgraded, reinstalling keeps the exact current payload.
        assert!(install(&root, false).unwrap().installed);
        assert_eq!(fs::read(root.join(TARGET)).unwrap(), PAYLOAD);
        assert_eq!(
            scene_launch_directory(&root).unwrap(),
            root.join(TARGET).parent().unwrap().join("scene-launch")
        );
        assert!(!uninstall(&root).unwrap().installed);
        assert_eq!(
            fs::read(root.join("AddonPackages/Keep.Scene.1.var")).unwrap(),
            b"keep"
        );
    }

    #[test]
    fn changed_dll_is_not_upgraded() {
        let f = Fixture::new();
        let target = f.0.join("changed.dll");
        fs::write(&target, b"modified after status check").unwrap();
        assert!(upgrade_known_version(&target).is_err());
        assert_eq!(fs::read(&target).unwrap(), b"modified after status check");
    }
}
