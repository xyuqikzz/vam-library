use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use tauri::Manager;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum DownloadTargetPolicy {
    Auto,
    RealAddon,
    ManagedLibrary,
}

impl Default for DownloadTargetPolicy {
    fn default() -> Self {
        Self::Auto
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum DownloadAfterAction {
    LibraryOnly,
    AddToActivePlan,
    AddAndApply,
}

impl Default for DownloadAfterAction {
    fn default() -> Self {
        Self::LibraryOnly
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InstallContext {
    pub vam_root: String,
    pub real_addon_dir: String,
    pub managed_library_dir: String,
    pub download_temp_dir: String,
    pub manifest_path: String,
    pub managed_state_path: String,
    pub is_managed: bool,
    pub active_on_demand_plan_id: Option<String>,
    pub download_target_dir: String,
    pub download_target_policy: DownloadTargetPolicy,
    pub download_after_action: DownloadAfterAction,
    pub mode_label: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ManagedState {
    pub managed: bool,
    pub version: u32,
    pub library_path: String,
    pub created_at: String,
    pub last_applied_plan_id: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(default)]
struct PersistedSettings {
    vam_root_path: Option<String>,
    managed_enabled: bool,
    managed_library_path: Option<String>,
    download_target_policy: DownloadTargetPolicy,
    download_after_action: DownloadAfterAction,
}

impl Default for PersistedSettings {
    fn default() -> Self {
        Self {
            vam_root_path: None,
            managed_enabled: false,
            managed_library_path: None,
            download_target_policy: DownloadTargetPolicy::Auto,
            download_after_action: DownloadAfterAction::LibraryOnly,
        }
    }
}

pub fn settings_file_path(app_handle: &tauri::AppHandle) -> Result<PathBuf, String> {
    let app_data_dir = app_handle
        .path()
        .app_data_dir()
        .map_err(|e| format!("解析应用数据目录失败: {}", e))?;
    std::fs::create_dir_all(&app_data_dir).map_err(|e| format!("创建应用数据目录失败: {}", e))?;
    Ok(app_data_dir.join("settings.json"))
}

pub fn resolve_install_context(app_handle: &tauri::AppHandle) -> Result<InstallContext, String> {
    let settings = read_settings(app_handle)?;
    let vam_root = settings
        .vam_root_path
        .as_ref()
        .ok_or_else(|| "VAM 根目录未配置，请先在设置中进行配置。".to_string())?;

    resolve_install_context_from_root(vam_root, &settings)
}

fn resolve_install_context_from_root(
    vam_root: &str,
    settings: &PersistedSettings,
) -> Result<InstallContext, String> {
    let root = PathBuf::from(vam_root);
    let real_addon_dir = root.join("AddonPackages");
    let default_library_dir = root.join("VAMBoxLibrary").join("AddonPackages");
    let managed_library_dir = settings
        .managed_library_path
        .as_ref()
        .filter(|path| !path.trim().is_empty())
        .map(PathBuf::from)
        .unwrap_or(default_library_dir);
    let vambox_library_root = managed_library_dir
        .parent()
        .map(Path::to_path_buf)
        .unwrap_or_else(|| root.join("VAMBoxLibrary"));

    let managed_state_path = vambox_library_root.join("managed-state.json");
    let manifest_path = vambox_library_root.join("on-demand-links.json");
    let managed_state = read_managed_state_path(&managed_state_path);
    let is_managed = settings.managed_enabled
        || managed_state
            .as_ref()
            .map(|state| state.managed)
            .unwrap_or(false);

    let download_target_dir = match settings.download_target_policy {
        DownloadTargetPolicy::RealAddon => real_addon_dir.clone(),
        DownloadTargetPolicy::ManagedLibrary => managed_library_dir.clone(),
        DownloadTargetPolicy::Auto => {
            if is_managed {
                managed_library_dir.clone()
            } else {
                real_addon_dir.clone()
            }
        }
    };

    let download_temp_dir = if is_managed {
        vambox_library_root.join(".downloads")
    } else {
        real_addon_dir.join(".VAMBoxLibrary-downloads")
    };

    let mode_label = if is_managed {
        "managed_library".to_string()
    } else {
        "real_addon".to_string()
    };

    Ok(InstallContext {
        vam_root: path_to_string(root),
        real_addon_dir: path_to_string(real_addon_dir),
        managed_library_dir: path_to_string(managed_library_dir),
        download_temp_dir: path_to_string(download_temp_dir),
        manifest_path: path_to_string(manifest_path),
        managed_state_path: path_to_string(managed_state_path),
        is_managed,
        active_on_demand_plan_id: managed_state.and_then(|state| state.last_applied_plan_id),
        download_target_dir: path_to_string(download_target_dir),
        download_target_policy: settings.download_target_policy.clone(),
        download_after_action: settings.download_after_action.clone(),
        mode_label,
    })
}

pub fn read_managed_state(root: &Path) -> Option<ManagedState> {
    read_managed_state_path(&root.join("VAMBoxLibrary").join("managed-state.json"))
}

pub fn write_managed_state(
    root: &Path,
    library_dir: &Path,
    last_applied_plan_id: Option<String>,
) -> Result<ManagedState, String> {
    let state_path = root.join("VAMBoxLibrary").join("managed-state.json");
    if let Some(parent) = state_path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| format!("创建接管状态目录失败 {}: {}", parent.display(), e))?;
    }

    let created_at = read_managed_state(root)
        .map(|state| state.created_at)
        .unwrap_or_else(|| chrono::Utc::now().to_rfc3339());
    let state = ManagedState {
        managed: true,
        version: 1,
        library_path: path_to_string(library_dir.to_path_buf()),
        created_at,
        last_applied_plan_id,
    };
    let contents =
        serde_json::to_string_pretty(&state).map_err(|e| format!("序列化接管状态失败: {}", e))?;
    std::fs::write(&state_path, contents)
        .map_err(|e| format!("写入接管状态失败 {}: {}", state_path.display(), e))?;
    Ok(state)
}

fn read_settings(app_handle: &tauri::AppHandle) -> Result<PersistedSettings, String> {
    let path = settings_file_path(app_handle)?;
    if !path.exists() {
        return Ok(PersistedSettings::default());
    }

    let contents =
        std::fs::read_to_string(&path).map_err(|e| format!("读取设置文件失败: {}", e))?;
    serde_json::from_str::<PersistedSettings>(&contents)
        .map_err(|e| format!("解析设置文件失败: {}", e))
}

fn read_managed_state_path(path: &Path) -> Option<ManagedState> {
    let contents = std::fs::read_to_string(path).ok()?;
    serde_json::from_str::<ManagedState>(&contents).ok()
}

fn path_to_string(path: PathBuf) -> String {
    path.to_string_lossy().to_string()
}
