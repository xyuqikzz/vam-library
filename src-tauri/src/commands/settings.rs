use serde::{Deserialize, Serialize};
use std::io::Write;
use std::path::Path;
use tauri::State;

use crate::services::install_context::{
    resolve_install_context, settings_file_path, DownloadAfterAction, DownloadTargetPolicy,
    InstallContext,
};
use crate::{db::Database, errors::AppError};

const MIN_CONCURRENT_DOWNLOADS: usize = 1;
const MAX_CONCURRENT_DOWNLOADS: usize = 5;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
#[serde(rename_all = "camelCase")]
pub struct VamInstance {
    pub id: String,
    pub name: String,
    pub root_path: String,
    pub managed_enabled: bool,
    pub managed_library_path: Option<String>,
    pub download_target_policy: DownloadTargetPolicy,
    pub download_after_action: DownloadAfterAction,
}

impl Default for VamInstance {
    fn default() -> Self {
        Self {
            id: String::new(),
            name: String::new(),
            root_path: String::new(),
            managed_enabled: false,
            managed_library_path: None,
            download_target_policy: DownloadTargetPolicy::Auto,
            download_after_action: DownloadAfterAction::LibraryOnly,
        }
    }
}

/// 应用设置，持久化到 settings.json。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct AppSettings {
    /// VAM 安装根目录。
    pub vam_root_path: Option<String>,
    /// 是否启动时自动扫描。
    pub auto_scan: bool,
    /// UI 主题。
    pub theme: String,
    /// 是否模糊所有资源预览图，旧配置默认开启。
    pub blur_previews: bool,
    /// 是否已启用资源接管。
    pub managed_enabled: bool,
    /// 自定义托管库路径，空值时使用 VAMBoxLibrary/AddonPackages。
    pub managed_library_path: Option<String>,
    /// 下载目标策略。
    pub download_target_policy: DownloadTargetPolicy,
    /// 下载完成后的动作。
    pub download_after_action: DownloadAfterAction,
    /// 最大同时下载数。
    pub max_concurrent_downloads: usize,
    /// 下载限速，0 表示不限速。
    pub speed_limit_kb: u64,
    /// 多 VAM 实例列表。
    pub vam_instances: Vec<VamInstance>,
    /// 当前实例 ID。
    pub active_instance_id: Option<String>,
    /// VAM Hub 登录 Cookie。
    pub hub_auth_cookie: Option<String>,
    /// 是否认为 Hub 登录态可用。
    pub hub_logged_in: bool,
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            vam_root_path: None,
            auto_scan: false,
            theme: "dark".to_string(),
            blur_previews: true,
            managed_enabled: false,
            managed_library_path: None,
            download_target_policy: DownloadTargetPolicy::Auto,
            download_after_action: DownloadAfterAction::LibraryOnly,
            max_concurrent_downloads: 1,
            speed_limit_kb: 0,
            vam_instances: Vec::new(),
            active_instance_id: None,
            hub_auth_cookie: None,
            hub_logged_in: false,
        }
    }
}

/// 读取设置；旧版 settings.json 缺少字段时自动补默认值。
#[tauri::command]
pub async fn get_settings(app_handle: tauri::AppHandle) -> Result<AppSettings, String> {
    read_settings_file(&settings_file_path(&app_handle)?)
}

/// 保存设置。
#[tauri::command]
pub async fn save_settings(
    mut settings: AppSettings,
    app_handle: tauri::AppHandle,
) -> Result<(), String> {
    let path = settings_file_path(&app_handle)?;
    normalize_settings(&mut settings);
    sync_current_instance_state(&mut settings);
    sync_active_instance(&mut settings);

    write_settings_file(&path, &settings)?;

    crate::services::scene_browser_index::schedule_sync(&app_handle);
    Ok(())
}

#[tauri::command]
pub async fn save_vam_instances(
    app_handle: tauri::AppHandle,
    instances: Vec<VamInstance>,
    active_instance_id: Option<String>,
) -> Result<AppSettings, String> {
    let path = settings_file_path(&app_handle)?;
    let mut settings = read_settings_file(&path)?;

    settings.vam_instances = dedupe_instances(instances);
    settings.active_instance_id = active_instance_id;
    normalize_settings(&mut settings);
    sync_active_instance(&mut settings);

    write_settings_file(&path, &settings)?;
    crate::services::scene_browser_index::schedule_sync(&app_handle);
    Ok(settings)
}

#[tauri::command]
pub async fn save_hub_auth_cookie(
    app_handle: tauri::AppHandle,
    cookie: Option<String>,
) -> Result<AppSettings, String> {
    let path = settings_file_path(&app_handle)?;
    let mut settings = read_settings_file(&path)?;

    let normalized = cookie
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty());
    settings.hub_logged_in = normalized.is_some();
    settings.hub_auth_cookie = normalized;
    normalize_settings(&mut settings);

    write_settings_file(&path, &settings)?;
    Ok(settings)
}

/// 返回统一安装上下文，供设置页、下载中心、按需启动页共享同一状态。
#[tauri::command]
pub async fn get_install_context(app_handle: tauri::AppHandle) -> Result<InstallContext, String> {
    resolve_install_context(&app_handle)
}

#[tauri::command]
pub async fn clear_local_database(
    app_handle: tauri::AppHandle,
    db: State<'_, Database>,
) -> Result<(), String> {
    db.with_conn(|conn| {
        conn.execute_batch(
            "
            BEGIN IMMEDIATE;
            DELETE FROM cleanup_trash;
            DELETE FROM resource_migration_log;
            DELETE FROM on_demand_state;
            DELETE FROM on_demand_plans;
            DELETE FROM physical_packages;
            DELETE FROM scene_references;
            DELETE FROM dependencies;
            DELETE FROM contents;
            DELETE FROM package_tags;
            DELETE FROM packages;
            DELETE FROM sqlite_sequence
            WHERE name IN (
                'contents',
                'dependencies',
                'scene_references',
                'resource_migration_log',
                'cleanup_trash'
            );
            COMMIT;
            ",
        )
        .map_err(|e| AppError::Database(format!("清空本地数据库失败: {}", e)))?;
        Ok(())
    })
    .map_err(|e| e.to_string())?;

    crate::services::scene_browser_index::schedule_sync(&app_handle);
    Ok(())
}

pub fn mark_managed_enabled(
    app_handle: &tauri::AppHandle,
    library_path: Option<String>,
) -> Result<(), String> {
    let path = settings_file_path(app_handle)?;
    let mut settings = read_settings_file(&path)?;

    settings.managed_enabled = true;
    settings.managed_library_path = library_path;
    normalize_settings(&mut settings);

    write_settings_file(&path, &settings)?;
    Ok(())
}

pub fn mark_managed_disabled(app_handle: &tauri::AppHandle) -> Result<(), String> {
    let path = settings_file_path(app_handle)?;
    let mut settings = read_settings_file(&path)?;

    settings.managed_enabled = false;
    settings.managed_library_path = None;
    normalize_settings(&mut settings);

    write_settings_file(&path, &settings)?;
    Ok(())
}

fn read_settings_file(path: &Path) -> Result<AppSettings, String> {
    let contents = match std::fs::read_to_string(path) {
        Ok(contents) => contents,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return Ok(AppSettings::default())
        }
        Err(error) => return Err(format!("读取设置文件失败: {}", error)),
    };
    let mut settings: AppSettings =
        serde_json::from_str(&contents).map_err(|error| format!("解析设置文件失败: {}", error))?;
    normalize_settings(&mut settings);
    Ok(settings)
}

pub(crate) fn write_settings_file(path: &Path, settings: &impl Serialize) -> Result<(), String> {
    static SEQUENCE: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let contents = serde_json::to_vec_pretty(settings)
        .map_err(|error| format!("序列化设置失败: {}", error))?;
    let temp = path.with_file_name(format!(
        ".settings-{}-{}.tmp",
        std::process::id(),
        SEQUENCE.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
    ));
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&temp)
        .map_err(|error| format!("创建临时设置文件失败: {}", error))?;
    // Readers see one complete settings snapshot; failure leaves the previous file intact.
    let result = (|| {
        file.write_all(&contents)?;
        file.sync_all()?;
        drop(file);
        std::fs::rename(&temp, path)
    })();
    if result.is_err() {
        let _ = std::fs::remove_file(&temp);
    }
    result.map_err(|error| format!("写入设置文件失败: {}", error))
}

fn normalize_settings(settings: &mut AppSettings) {
    settings.max_concurrent_downloads = settings
        .max_concurrent_downloads
        .clamp(MIN_CONCURRENT_DOWNLOADS, MAX_CONCURRENT_DOWNLOADS);
}


fn normalize_instance_root_path(path: &str) -> String {
    path.trim()
        .replace('/', "\\")
        .trim_end_matches('\\')
        .to_lowercase()
}

fn dedupe_instances(instances: Vec<VamInstance>) -> Vec<VamInstance> {
    let mut seen = std::collections::HashSet::new();
    let mut result = Vec::new();

    for instance in instances {
        let key = normalize_instance_root_path(&instance.root_path);
        if key.is_empty() || !seen.insert(key) {
            continue;
        }
        result.push(instance);
    }

    result
}

fn sync_active_instance(settings: &mut AppSettings) {
    if settings.vam_instances.is_empty() {
        settings.active_instance_id = None;
        return;
    }

    let active_id = settings
        .active_instance_id
        .clone()
        .filter(|id| {
            settings
                .vam_instances
                .iter()
                .any(|instance| &instance.id == id)
        })
        .or_else(|| {
            settings
                .vam_instances
                .first()
                .map(|instance| instance.id.clone())
        });

    if let Some(active_id) = active_id {
        if let Some(active) = settings
            .vam_instances
            .iter()
            .find(|instance| instance.id == active_id)
        {
            settings.vam_root_path = Some(active.root_path.clone());
            settings.active_instance_id = Some(active_id);
            settings.managed_enabled = active.managed_enabled;
            settings.managed_library_path = active.managed_library_path.clone();
            settings.download_target_policy = active.download_target_policy.clone();
            settings.download_after_action = active.download_after_action.clone();
        }
    }
}

fn sync_current_instance_state(settings: &mut AppSettings) {
    let Some(active_id) = settings.active_instance_id.clone() else {
        return;
    };
    if let Some(instance) = settings
        .vam_instances
        .iter_mut()
        .find(|instance| instance.id == active_id)
    {
        if let Some(root_path) = settings.vam_root_path.clone() {
            instance.root_path = root_path;
        }
        instance.managed_enabled = settings.managed_enabled;
        instance.managed_library_path = settings.managed_library_path.clone();
        instance.download_target_policy = settings.download_target_policy.clone();
        instance.download_after_action = settings.download_after_action.clone();
    }
}

#[cfg(test)]
mod preview_settings_tests {
    use super::AppSettings;

    #[test]
    fn preview_blur_defaults_on_and_persists_explicit_off() {
        assert!(AppSettings::default().blur_previews);
        let legacy: AppSettings =
            serde_json::from_str(r#"{"theme":"dark","auto_scan":false}"#).unwrap();
        assert!(legacy.blur_previews);
        let disabled: AppSettings = serde_json::from_str(r#"{"blur_previews":false}"#).unwrap();
        let restored: AppSettings =
            serde_json::from_str(&serde_json::to_string(&disabled).unwrap()).unwrap();
        assert!(!restored.blur_previews);
    }
}

#[cfg(test)]
mod persistence_tests {
    use super::*;
    use crate::services::resource_files::tests::TestDir;

    #[test]
    fn missing_and_legacy_settings_load_but_corruption_is_not_reset() {
        let root = TestDir::new();
        let path = root.0.join("settings.json");
        assert!(read_settings_file(&path).is_ok());
        std::fs::write(&path, r#"{"max_concurrent_downloads":99,"theme":"light"}"#).unwrap();
        let settings = read_settings_file(&path).unwrap();
        assert_eq!(settings.max_concurrent_downloads, 5);
        assert_eq!(settings.theme, "light");
        std::fs::write(&path, b"broken JSON").unwrap();
        assert!(read_settings_file(&path).is_err());
        assert_eq!(std::fs::read(&path).unwrap(), b"broken JSON");
    }

    #[test]
    fn settings_are_replaced_completely_and_failed_writes_remove_only_their_temp_file() {
        let root = TestDir::new();
        let path = root.0.join("settings.json");
        let mut settings = AppSettings::default();
        write_settings_file(&path, &settings).unwrap();
        settings.theme = "light".into();
        write_settings_file(&path, &settings).unwrap();
        assert_eq!(read_settings_file(&path).unwrap().theme, "light");
        assert_eq!(std::fs::read_dir(&root.0).unwrap().count(), 1);
        let blocked = root.0.join("blocked");
        std::fs::create_dir(&blocked).unwrap();
        std::fs::write(blocked.join("keep"), b"user data").unwrap();
        assert!(write_settings_file(&blocked, &settings).is_err());
        assert_eq!(std::fs::read(blocked.join("keep")).unwrap(), b"user data");
        assert_eq!(std::fs::read_dir(&root.0).unwrap().count(), 2);
    }
}
