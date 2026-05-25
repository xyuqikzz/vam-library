use std::sync::atomic::{AtomicU64, Ordering};

use serde::Serialize;
use tauri::{AppHandle, Emitter};

static LIBRARY_REVISION: AtomicU64 = AtomicU64::new(1);

#[derive(Debug, Clone, Serialize)]
pub struct LibraryIndexChangedPayload {
    pub revision: u64,
    pub reason: String,
    pub changed_package_ids: Vec<String>,
    pub changed_paths: Vec<String>,
    pub invalidates: Vec<String>,
}

/// 统一通知前端本地资源索引已发生变化。
pub fn emit_library_index_changed(
    app_handle: &AppHandle,
    reason: &str,
    invalidates: &[&str],
    changed_package_ids: Vec<String>,
    changed_paths: Vec<String>,
) {
    let payload = LibraryIndexChangedPayload {
        revision: LIBRARY_REVISION.fetch_add(1, Ordering::SeqCst),
        reason: reason.to_string(),
        changed_package_ids,
        changed_paths,
        invalidates: invalidates.iter().map(|item| item.to_string()).collect(),
    };

    let _ = app_handle.emit("library-index-changed", payload);
}
