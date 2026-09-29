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
// Only the exact released 1.0.0/1.0.1 payloads may be upgraded in place.
const PREVIOUS_SHA256: &[&str] = &[
    "ee1a3178f538047be8f1a2e7b9e6134ad32820eaf9983e9550a33f8469040e40",
    "51e6ef72ee53f657347706990a515c0d88ea260e7b772a6cc1679fd2bcf4aea8",
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
    reason: Option<String>,
    target_path: String,
}

fn hash(bytes: &[u8]) -> String {
    hex::encode(Sha256::digest(bytes))
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

fn inspect(root: &Path) -> Result<ModStatus, String> {
    let manifest: Manifest = serde_json::from_str(MANIFEST).map_err(|e| e.to_string())?;
    let target = safe_target(root)?;
    let existing = match fs::read(&target) {
        Ok(bytes) => Some(bytes),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => None,
        Err(e) => return Err(e.to_string()),
    };
    let installed = existing.as_deref() == Some(PAYLOAD);
    let update_available = existing
        .as_ref()
        .is_some_and(|bytes| PREVIOUS_SHA256.contains(&hash(bytes).as_str()));
    let conflict = existing.is_some() && !installed && !update_available;
    let mut reason = None;
    for dependency in manifest.files {
        match fs::read(root.join(&dependency.path)) {
            Ok(bytes) if hash(&bytes) == dependency.sha256 => {}
            Ok(_) => {
                reason = Some(format!(
                    "当前 {} 与模组编译目标不同，需要针对该游戏版本重新构建模组",
                    dependency.path
                ));
                break;
            }
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
        version: manifest.version,
        installed,
        update_available,
        conflict,
        compatible: reason.is_none(),
        reason,
        target_path: target.to_string_lossy().into_owned(),
    })
}

fn ensure_game_stopped() -> Result<(), String> {
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        let output = std::process::Command::new("tasklist.exe")
            .args(["/FI", "IMAGENAME eq VaM.exe", "/FO", "CSV", "/NH"])
            .creation_flags(0x08000000)
            .output()
            .map_err(|e| format!("无法检查游戏进程: {e}"))?;
        if !output.status.success() {
            return Err("无法检查游戏进程，请稍后重试".into());
        }
        if String::from_utf8_lossy(&output.stdout)
            .to_ascii_lowercase()
            .contains("\"vam.exe\"")
        {
            return Err("请先退出 VaM，再安装或卸载模组".into());
        }
        Ok(())
    }
    #[cfg(not(windows))]
    {
        Err("此游戏模组目前仅支持 Windows".into())
    }
}

fn install(root: &Path) -> Result<ModStatus, String> {
    let status = inspect(root)?;
    if status.conflict {
        return Err("同名 DLL 已存在且内容不同，未覆盖；请先移开该文件".into());
    }
    if !status.compatible {
        return Err(status.reason.unwrap_or_default());
    }
    if status.installed {
        return Ok(status);
    }
    let target = safe_target(root)?;
    if status.update_available {
        upgrade_known_version(&target)?;
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
    if !PREVIOUS_SHA256.contains(&hash(&original).as_str()) {
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
        fs::remove_file(safe_target(root)?).map_err(|e| e.to_string())?;
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
pub async fn install_scene_browser_mod(vam_root: String) -> Result<ModStatus, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let _guard = MOD_LOCK.lock().map_err(|e| e.to_string())?;
        let root = checked_root(&vam_root)?;
        ensure_game_stopped()?;
        install(&root)
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
pub async fn uninstall_scene_browser_mod(vam_root: String) -> Result<ModStatus, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let _guard = MOD_LOCK.lock().map_err(|e| e.to_string())?;
        let root = checked_root(&vam_root)?;
        ensure_game_stopped()?;
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
    fn missing_dependencies_do_not_write_mod() {
        let f = Fixture::new();
        assert!(!inspect(&f.0).unwrap().compatible);
        assert!(install(&f.0).is_err());
        assert!(!f.0.join(TARGET).exists());
    }
    #[test]
    fn foreign_dll_is_neither_overwritten_nor_removed() {
        let f = Fixture::new();
        let target = f.0.join(TARGET);
        fs::create_dir_all(target.parent().unwrap()).unwrap();
        fs::write(&target, b"user mod").unwrap();
        assert!(inspect(&f.0).unwrap().conflict);
        assert!(install(&f.0).is_err());
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
        assert!(install(&root).unwrap().installed);
        assert_eq!(hash(&fs::read(root.join(TARGET)).unwrap()), hash(PAYLOAD));
        assert!(install(&root).unwrap().installed);
        assert!(!uninstall(&root).unwrap().installed);
        let previous =
            include_bytes!("../../../mods/SceneBrowser/tests/fixtures/SceneBrowser-1.0.0.dll");
        assert!(PREVIOUS_SHA256.contains(&hash(previous).as_str()));
        fs::write(root.join(TARGET), previous).unwrap();
        let old_status = inspect(&root).unwrap();
        assert!(old_status.update_available && !old_status.conflict);
        assert!(install(&root).unwrap().installed);
        assert_eq!(fs::read(root.join(TARGET)).unwrap(), PAYLOAD);
        assert!(!uninstall(&root).unwrap().installed);
        let previous =
            include_bytes!("../../../mods/SceneBrowser/tests/fixtures/SceneBrowser-1.0.1.dll");
        assert!(PREVIOUS_SHA256.contains(&hash(previous).as_str()));
        fs::write(root.join(TARGET), previous).unwrap();
        assert!(inspect(&root).unwrap().update_available);
        assert!(install(&root).unwrap().installed);
        assert_eq!(fs::read(root.join(TARGET)).unwrap(), PAYLOAD);
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
