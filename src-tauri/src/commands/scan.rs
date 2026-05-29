use std::path::Path;
use std::sync::Mutex;

use tauri::State;
use tauri::{AppHandle, Emitter};

use crate::db::Database;
use crate::services::scanner::{self, ScanCacheEntry, ScanProgress, ScanResult};

/// Global watcher state (stored as managed state)
pub struct FileWatcherState {
    pub inner: Mutex<Option<notify::RecommendedWatcher>>,
}

/// Start watching the VAM AddonPackages directory for file changes.
/// Emits "file-change" events to the frontend when .var files change.
#[tauri::command]
pub async fn start_file_watcher(
    app: AppHandle,
    path: String,
    watcher_state: State<'_, FileWatcherState>,
) -> Result<(), String> {
    let root = std::path::PathBuf::from(&path);
    let watcher = crate::services::watcher::start_watching(root, app).map_err(|e| e.to_string())?;

    let mut state = watcher_state.inner.lock().map_err(|e| e.to_string())?;
    *state = Some(watcher);

    Ok(())
}

/// Stop the active file watcher.
#[tauri::command]
pub async fn stop_file_watcher(watcher_state: State<'_, FileWatcherState>) -> Result<(), String> {
    let mut state = watcher_state.inner.lock().map_err(|e| e.to_string())?;
    *state = None; // Dropping the watcher stops it
    Ok(())
}

/// Scan a VAM directory for .var packages.
///
/// Validates the path, scans AddonPackages/ for .var files, parses each one,
/// stores results in the database, emits progress events, and returns a scan summary.
#[tauri::command]
pub async fn scan_vam_directory(
    app: AppHandle,
    path: String,
    db: State<'_, Database>,
) -> Result<ScanResult, String> {
    let root = std::path::PathBuf::from(&path);

    // Validate the path exists
    if !root.exists() {
        return Err(format!("Path does not exist: {}", path));
    }

    // Validate it has an AddonPackages subfolder
    let addon_dir = root.join("AddonPackages");
    if !addon_dir.exists() {
        return Err(format!(
            "AddonPackages directory not found at '{}'. Is this a valid VAM installation?",
            addon_dir.display()
        ));
    }

    // Run the scan (blocking I/O, run on blocking thread) with progress events
    let root_path = root.clone();
    let app_clone = app.clone();
    let scan_cache = load_scan_cache(&db)?;

    // Clear thumbnail cache so that optimized/updated thumbnails are regenerated using the new cover heuristics
    let _ = crate::commands::packages::clear_thumbnail_cache_inner(&app);

    let (packages, scan_result) = tokio::task::spawn_blocking(move || {
        let mut progress_cb = |progress: ScanProgress| {
            let _ = app_clone.emit("scan-progress", &progress);
        };
        scanner::scan_addon_packages_with_cache(&root_path, scan_cache, Some(&mut progress_cb))
    })
    .await
    .map_err(|e| format!("Scan task failed: {}", e))?
    .map_err(|e| e.to_string())?;

    // Store packages in database
    db.with_conn(|conn| {
        let tx = conn.unchecked_transaction().map_err(|e| {
            crate::errors::AppError::Database(format!("Failed to begin transaction: {}", e))
        })?;

        remove_missing_physical_packages(&tx, &scan_result.current_file_paths)?;

        {
            let mut upsert_package = tx
                .prepare_cached(
                    "INSERT INTO packages (
                    id, creator, name, version, file_path, size_bytes,
                    license_type, description, credits, instructions,
                    promotional_link, meta_json, resource_types, file_created_time, scan_time
                ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15)
                ON CONFLICT(id) DO UPDATE SET
                    creator = excluded.creator,
                    name = excluded.name,
                    version = excluded.version,
                    file_path = excluded.file_path,
                    size_bytes = excluded.size_bytes,
                    license_type = excluded.license_type,
                    description = excluded.description,
                    credits = excluded.credits,
                    instructions = excluded.instructions,
                    promotional_link = excluded.promotional_link,
                    meta_json = excluded.meta_json,
                    resource_types = excluded.resource_types,
                    file_created_time = excluded.file_created_time,
                    updated_at = datetime('now')",
                )
                .map_err(|e| crate::errors::AppError::Database(e.to_string()))?;
            let mut upsert_physical = tx
                .prepare_cached(
                    "INSERT OR REPLACE INTO physical_packages (
                    file_path, package_id, size_bytes, modified_time, scan_time, scan_status, last_error
                 ) VALUES (?1, ?2, ?3, ?4, ?5, 'ok', NULL)",
                )
                .map_err(|e| crate::errors::AppError::Database(e.to_string()))?;
            let mut clear_contents = tx
                .prepare_cached("DELETE FROM contents WHERE package_id = ?1")
                .map_err(|e| crate::errors::AppError::Database(e.to_string()))?;
            let mut clear_dependencies = tx
                .prepare_cached("DELETE FROM dependencies WHERE package_id = ?1")
                .map_err(|e| crate::errors::AppError::Database(e.to_string()))?;
            let mut insert_content = tx
                .prepare_cached(
                    "INSERT OR IGNORE INTO contents (package_id, file_path, resource_type, size_bytes)
                     VALUES (?1, ?2, ?3, ?4)",
                )
                .map_err(|e| crate::errors::AppError::Database(e.to_string()))?;
            let mut insert_dependency = tx
                .prepare_cached(
                    "INSERT OR IGNORE INTO dependencies (package_id, depends_on_id, required_version)
                         VALUES (?1, ?2, ?3)",
                )
                .map_err(|e| crate::errors::AppError::Database(e.to_string()))?;
            let mut upsert_failure = tx
                .prepare_cached(
                    "INSERT OR REPLACE INTO physical_packages (
                    file_path, package_id, size_bytes, modified_time, scan_time, scan_status, last_error
                 ) VALUES (?1, ?2, ?3, ?4, ?5, 'failed', ?6)",
                )
                .map_err(|e| crate::errors::AppError::Database(e.to_string()))?;

            for pkg in &packages {
                let resource_types_json =
                    serde_json::to_string(&pkg.resource_types).unwrap_or_else(|_| "[]".to_string());

                let meta_json = pkg
                    .meta
                    .as_ref()
                    .and_then(|m| serde_json::to_string(m).ok());

                let description = pkg.meta.as_ref().and_then(|m| m.description.clone());
                let credits = pkg.meta.as_ref().and_then(|m| m.credits.clone());
                let instructions = pkg.meta.as_ref().and_then(|m| m.instructions.clone());
                let promotional_link = pkg.meta.as_ref().and_then(|m| m.promotional_link.clone());
                let license_type = pkg
                    .meta
                    .as_ref()
                    .map(|m| m.license_type.clone())
                    .unwrap_or_default();

                upsert_package
                    .execute(rusqlite::params![
                        pkg.id,
                        pkg.creator,
                        pkg.name,
                        pkg.version,
                        pkg.file_path,
                        pkg.size_bytes,
                        license_type,
                        description,
                        credits,
                        instructions,
                        promotional_link,
                        meta_json,
                        resource_types_json,
                        pkg.created_time,
                        pkg.scan_time,
                    ])
                    .map_err(|e| {
                        crate::errors::AppError::Database(format!(
                            "Failed to insert package '{}': {}",
                            pkg.id, e
                        ))
                    })?;

                upsert_physical
                    .execute(rusqlite::params![
                        pkg.file_path,
                        pkg.id,
                        pkg.size_bytes as i64,
                        pkg.modified_time,
                        pkg.scan_time,
                    ])
                    .map_err(|e| {
                        crate::errors::AppError::Database(format!(
                            "Failed to insert physical package path: {}",
                            e
                        ))
                    })?;

                clear_contents.execute([&pkg.id]).map_err(|e| {
                    crate::errors::AppError::Database(format!(
                        "Failed to clear content entries for '{}': {}",
                        pkg.id, e
                    ))
                })?;

                clear_dependencies.execute([&pkg.id]).map_err(|e| {
                    crate::errors::AppError::Database(format!(
                        "Failed to clear dependency entries for '{}': {}",
                        pkg.id, e
                    ))
                })?;

                // 重新扫描同一个包时先清空旧明细，避免文件数重复累加。
                for (content_path, size) in &pkg.contents {
                    let rt = crate::models::resource::ResourceType::from_path(content_path);
                    insert_content
                        .execute(rusqlite::params![pkg.id, content_path, rt.as_str(), size])
                        .map_err(|e| {
                            crate::errors::AppError::Database(format!(
                                "Failed to insert content entry: {}",
                                e
                            ))
                        })?;
                }
                if let Some(meta) = &pkg.meta {
                    for (dep_id, _dep_val) in &meta.dependencies {
                        let parts: Vec<&str> = dep_id.splitn(3, '.').collect();
                        let required_version = if parts.len() == 3 {
                            parts[2].to_string()
                        } else {
                            "latest".to_string()
                        };

                        insert_dependency
                            .execute(rusqlite::params![pkg.id, dep_id, required_version])
                            .map_err(|e| {
                                crate::errors::AppError::Database(format!(
                                    "Failed to insert dependency: {}",
                                    e
                                ))
                            })?;
                    }
                }
            }

            for failure in &scan_result.failures {
                let package_id = Path::new(&failure.file_path)
                    .file_stem()
                    .and_then(|name| name.to_str())
                    .unwrap_or(&failure.file_path)
                    .to_string();
                upsert_failure
                    .execute(rusqlite::params![
                        failure.file_path,
                        package_id,
                        failure.size_bytes as i64,
                        failure.modified_time,
                        chrono::Utc::now().to_rfc3339(),
                        failure.error,
                    ])
                    .map_err(|e| {
                        crate::errors::AppError::Database(format!(
                            "Failed to record failed package scan: {}",
                            e
                        ))
                    })?;
            }
        }

        tx.execute(
            "DELETE FROM packages
             WHERE id NOT IN (SELECT DISTINCT package_id FROM physical_packages)",
            [],
        )
        .map_err(|e| {
            crate::errors::AppError::Database(format!(
                "Failed to remove packages missing from current scan: {}",
                e
            ))
        })?;

        tx.commit().map_err(|e| {
            crate::errors::AppError::Database(format!("Failed to commit transaction: {}", e))
        })?;

        Ok(())
    })
    .map_err(|e| e.to_string())?;

    // Emit completion event
    let _ = app.emit(
        "scan-progress",
        &ScanProgress {
            total_files: packages.len(),
            processed_files: packages.len(),
            current_file: String::new(),
            phase: "done".to_string(),
        },
    );
    crate::commands::library_events::emit_library_index_changed(
        &app,
        "scan_completed",
        &[
            "packages",
            "dashboard",
            "dependencies",
            "statistics",
            "folders",
            "tags",
            "dedupe",
        ],
        packages.iter().map(|package| package.id.clone()).collect(),
        scan_result.current_file_paths.clone(),
    );

    Ok(scan_result)
}

fn load_scan_cache(
    db: &State<'_, Database>,
) -> Result<std::collections::HashMap<String, ScanCacheEntry>, String> {
    db.with_conn(|conn| {
        let mut stmt = conn
            .prepare(
                "SELECT file_path, size_bytes, COALESCE(modified_time, ''), COALESCE(scan_status, 'ok')
                 FROM physical_packages",
            )
            .map_err(|e| crate::errors::AppError::Database(e.to_string()))?;

        let rows = stmt
            .query_map([], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    ScanCacheEntry {
                        size_bytes: row.get::<_, i64>(1)? as u64,
                        modified_time: row.get(2)?,
                        scan_status: row.get(3)?,
                    },
                ))
            })
            .map_err(|e| crate::errors::AppError::Database(e.to_string()))?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| crate::errors::AppError::Database(e.to_string()))?;

        Ok(rows.into_iter().collect())
    })
    .map_err(|e| e.to_string())
}

fn remove_missing_physical_packages(
    tx: &rusqlite::Transaction<'_>,
    current_file_paths: &[String],
) -> Result<(), crate::errors::AppError> {
    if current_file_paths.is_empty() {
        tx.execute("DELETE FROM physical_packages", [])
            .map_err(|e| crate::errors::AppError::Database(e.to_string()))?;
        return Ok(());
    }

    let placeholders = std::iter::repeat("?")
        .take(current_file_paths.len())
        .collect::<Vec<_>>()
        .join(",");
    let sql = format!(
        "DELETE FROM physical_packages WHERE file_path NOT IN ({})",
        placeholders
    );
    let params = rusqlite::params_from_iter(current_file_paths.iter());
    tx.execute(&sql, params)
        .map_err(|e| crate::errors::AppError::Database(e.to_string()))?;
    Ok(())
}

/// Validate whether a path looks like a valid VAM installation.
///
/// Checks for the presence of AddonPackages/, Custom/, and Saves/ directories.
#[tauri::command]
pub async fn validate_vam_directory(path: String) -> Result<bool, String> {
    let root = Path::new(&path);

    if !root.exists() {
        return Ok(false);
    }

    let has_addon_packages = root.join("AddonPackages").exists();
    let has_custom = root.join("Custom").exists();
    let has_saves = root.join("Saves").exists();

    Ok(has_addon_packages && has_custom && has_saves)
}
