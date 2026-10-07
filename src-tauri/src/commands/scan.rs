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

        for pkg in &packages {
            crate::services::resource_files::index_parsed_package(&tx, pkg).map_err(|e| {
                crate::errors::AppError::Database(format!("Failed to index package '{}': {}", pkg.id, e))
            })?;
        }
        {
            let mut upsert_failure = tx.prepare_cached(
                "INSERT INTO physical_packages (file_path, package_id, size_bytes, modified_time, scan_time, scan_status, last_error)
                 VALUES (?1, ?2, ?3, ?4, ?5, 'failed', ?6)
                 ON CONFLICT(file_path) DO UPDATE SET package_id=excluded.package_id,
                    size_bytes=excluded.size_bytes, modified_time=excluded.modified_time,
                    scan_status='failed', last_error=excluded.last_error, file_md5=NULL",
            )?;

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

    // A large library can exceed SQLite's bound-parameter limit. Compare paths
    // in memory, then reuse a single indexed DELETE inside the scan transaction.
    let current: std::collections::HashSet<&str> =
        current_file_paths.iter().map(String::as_str).collect();
    let mut query = tx.prepare("SELECT file_path FROM physical_packages")?;
    let paths = query
        .query_map([], |row| row.get::<_, String>(0))?
        .collect::<Result<Vec<_>, _>>()?;
    let mut delete = tx.prepare("DELETE FROM physical_packages WHERE file_path = ?1")?;
    for path in paths {
        if !current.contains(path.as_str()) {
            delete.execute([path])?;
        }
    }
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn large_scan_removes_only_missing_paths_without_sql_parameter_limit() {
        let mut conn = rusqlite::Connection::open_in_memory().unwrap();
        crate::db::schema::create_tables(&conn).unwrap();
        let paths: Vec<_> = (0..40_000)
            .map(|i| format!("AddonPackages/A.P.{i}.var"))
            .collect();
        for path in [&paths[0], &paths[39_999], &"stale.var".to_string()] {
            conn.execute("INSERT INTO physical_packages (file_path,package_id,size_bytes,scan_time) VALUES (?1,'A.P.1',1,'original')", [path]).unwrap();
        }
        let tx = conn.transaction().unwrap();
        remove_missing_physical_packages(&tx, &paths).unwrap();
        assert_eq!(
            tx.query_row("SELECT count(*) FROM physical_packages", [], |r| r
                .get::<_, i64>(0))
                .unwrap(),
            2
        );
        assert_eq!(
            tx.query_row(
                "SELECT count(*) FROM physical_packages WHERE scan_time='original'",
                [],
                |r| r.get::<_, i64>(0)
            )
            .unwrap(),
            2
        );
        remove_missing_physical_packages(&tx, &[]).unwrap();
        assert_eq!(
            tx.query_row("SELECT count(*) FROM physical_packages", [], |r| r
                .get::<_, i64>(0))
                .unwrap(),
            0
        );
    }
}
