use std::path::PathBuf;

use notify::{Event, EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use tauri::{AppHandle, Emitter};

use crate::errors::AppError;

/// Information about a detected file change
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct FileChangeEvent {
    pub path: String,
    pub kind: String, // "added" | "modified" | "removed"
    pub is_var_file: bool,
}

/// Start watching a directory for file system changes.
/// When a .var file is added, modified, or removed, emits a "file-change" event
/// to the frontend so the UI can prompt for a re-scan.
pub fn start_watching(
    path: PathBuf,
    app_handle: AppHandle,
) -> Result<RecommendedWatcher, AppError> {
    let addon_dir = path.join("AddonPackages");

    if !addon_dir.exists() {
        return Err(AppError::Io(format!(
            "Directory does not exist: {:?}",
            addon_dir
        )));
    }

    let app = app_handle.clone();

    let mut watcher =
        notify::recommended_watcher(move |res: Result<Event, notify::Error>| match res {
            Ok(event) => {
                let kind_str = match event.kind {
                    EventKind::Create(_) => "added",
                    EventKind::Modify(_) => "modified",
                    EventKind::Remove(_) => "removed",
                    _ => return,
                };

                for path in &event.paths {
                    if let Some(p) = path.to_str() {
                        let is_var = p.ends_with(".var");
                        let change = FileChangeEvent {
                            path: p.to_string(),
                            kind: kind_str.to_string(),
                            is_var_file: is_var,
                        };

                        let _ = app.emit("file-change", &change);
                    }
                }
            }
            Err(e) => {
                log::warn!("File watch error: {:?}", e);
            }
        })
        .map_err(|e| AppError::Io(format!("Failed to create watcher: {}", e)))?;

    watcher
        .watch(&addon_dir, RecursiveMode::Recursive)
        .map_err(|e| AppError::Io(format!("Failed to watch directory: {}", e)))?;

    Ok(watcher)
}
