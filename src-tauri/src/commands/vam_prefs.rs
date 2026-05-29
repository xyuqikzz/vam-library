use chrono::Utc;
use serde::Serialize;
use std::path::{Path, PathBuf};

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VamPrefsFile {
    pub path: String,
    pub content: String,
    pub exists: bool,
    pub backup_path: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SaveVamPrefsResult {
    pub path: String,
    pub backup_path: Option<String>,
}

#[tauri::command]
pub async fn read_vam_prefs(vam_root: String) -> Result<VamPrefsFile, String> {
    let prefs_path = resolve_prefs_path(&vam_root)?;
    if !prefs_path.exists() {
        return Ok(VamPrefsFile {
            path: path_to_string(&prefs_path),
            content: "{\n}\n".to_string(),
            exists: false,
            backup_path: None,
        });
    }

    let content =
        std::fs::read_to_string(&prefs_path).map_err(|e| format!("读取 prefs.json 失败: {}", e))?;
    let content = format_json_if_possible(&content);

    Ok(VamPrefsFile {
        path: path_to_string(&prefs_path),
        content,
        exists: true,
        backup_path: None,
    })
}

#[tauri::command]
pub async fn save_vam_prefs(
    vam_root: String,
    content: String,
) -> Result<SaveVamPrefsResult, String> {
    let prefs_path = resolve_prefs_path(&vam_root)?;
    serde_json::from_str::<serde_json::Value>(&content)
        .map_err(|e| format!("prefs.json 不是有效 JSON: {}", e))?;

    let backup_path = if prefs_path.exists() {
        let backup_path = build_backup_path(&prefs_path);
        std::fs::copy(&prefs_path, &backup_path)
            .map_err(|e| format!("备份 prefs.json 失败: {}", e))?;
        Some(path_to_string(&backup_path))
    } else {
        None
    };

    std::fs::write(&prefs_path, ensure_trailing_newline(content))
        .map_err(|e| format!("写入 prefs.json 失败: {}", e))?;

    Ok(SaveVamPrefsResult {
        path: path_to_string(&prefs_path),
        backup_path,
    })
}

fn resolve_prefs_path(vam_root: &str) -> Result<PathBuf, String> {
    let root = PathBuf::from(vam_root.trim());
    if root.as_os_str().is_empty() {
        return Err("请先配置 VAM 目录。".to_string());
    }
    if !root.exists() || !root.is_dir() {
        return Err("VAM 目录不存在或不是文件夹。".to_string());
    }
    Ok(root.join("prefs.json"))
}

fn build_backup_path(prefs_path: &Path) -> PathBuf {
    let timestamp = Utc::now().format("%Y%m%d%H%M%S");
    prefs_path.with_file_name(format!("prefs.json.vambox-backup-{}", timestamp))
}

fn format_json_if_possible(content: &str) -> String {
    match serde_json::from_str::<serde_json::Value>(content) {
        Ok(value) => {
            let mut formatted =
                serde_json::to_string_pretty(&value).unwrap_or_else(|_| content.to_string());
            formatted.push('\n');
            formatted
        }
        Err(_) => content.to_string(),
    }
}

fn ensure_trailing_newline(mut content: String) -> String {
    if !content.ends_with('\n') {
        content.push('\n');
    }
    content
}

fn path_to_string(path: &Path) -> String {
    path.to_string_lossy().to_string()
}
