use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
use tauri::{Emitter, State};

use crate::db::Database;
use crate::errors::AppError;
use crate::models::resource::{normalize_resource_types, primary_resource_type};

/// Migration mode
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum MigrationMode {
    ByType,
    ByCreator,
    ByScene,
    Custom,
}

/// Configuration for a migration task
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MigrationConfig {
    pub mode: MigrationMode,
    pub source_dir: String,
    pub target_dir: String,
    pub dry_run: bool,
    pub locale: Option<String>,
}

/// A single file operation in a migration plan
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MigrationOperation {
    pub package_id: String,
    pub source: String,
    pub destination: String,
    pub action: String, // "move" | "copy"
    pub size_bytes: u64,
    pub resource_type: String,
    pub conflict: String, // "none" | "overwrite" | "skip"
}

/// Result of a migration preview
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MigrationPreview {
    pub total_operations: usize,
    pub total_size_bytes: u64,
    pub operations: Vec<MigrationOperation>,
    pub conflicts: Vec<String>,
}

/// Result of a migration execution
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MigrationResult {
    pub task_id: String,
    pub completed: usize,
    pub failed: usize,
    pub total: usize,
    pub errors: Vec<String>,
    pub rollback_available: bool,
}

/// Undo log entry for a completed migration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MigrationUndoEntry {
    pub task_id: String,
    pub source: String,
    pub original_path: String,
    pub new_path: String,
    pub action: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MigrationRollbackResult {
    pub task_id: String,
    pub restored: usize,
    pub failed: usize,
    pub errors: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MigrationProgress {
    pub task_id: String,
    pub phase: String,
    pub completed: usize,
    pub failed: usize,
    pub total: usize,
    pub current_package_id: Option<String>,
}

fn localized_type_folder(resource_type: &str, locale: Option<&str>) -> String {
    let is_zh = locale.unwrap_or_default().to_lowercase().starts_with("zh");
    let folder = match (is_zh, resource_type) {
        (true, "scene") => "场景",
        (true, "appearance") => "外观",
        (true, "morph") => "变形",
        (true, "clothing") => "服装",
        (true, "hair") => "头发",
        (true, "texture") => "纹理",
        (true, "plugin") => "插件",
        (true, "asset") => "资产",
        (true, "sound") => "声音",
        (true, _) => "其他",
        (false, "scene") => "Scenes",
        (false, "appearance") => "Appearances",
        (false, "morph") => "Morphs",
        (false, "clothing") => "Clothing",
        (false, "hair") => "Hair",
        (false, "texture") => "Textures",
        (false, "plugin") => "Plugins",
        (false, "asset") => "Assets",
        (false, "sound") => "Sounds",
        (false, _) => "Other",
    };
    folder.to_string()
}

fn is_under_dir(file_path: &str, dir_path: &str) -> bool {
    if dir_path.trim().is_empty() {
        return true;
    }

    let file = Path::new(file_path);
    let dir = Path::new(dir_path);
    file.starts_with(dir)
}

/// Preview a migration operation without executing it.
/// Generates a list of file operations based on the selected mode.
#[tauri::command]
pub async fn preview_migration(
    db: State<'_, Database>,
    config: MigrationConfig,
) -> Result<MigrationPreview, String> {
    db.with_conn(|conn| {
        // Load all packages
        let mut stmt = conn
            .prepare(
                "SELECT id, creator, name, version, file_path, size_bytes, resource_types
                 FROM packages
                 ORDER BY creator, name",
            )
            .map_err(|e| e.to_string())?;

        let packages: Vec<(String, String, String, i32, String, i64, String)> = stmt
            .query_map([], |row| {
                Ok((
                    row.get(0)?,
                    row.get(1)?,
                    row.get(2)?,
                    row.get(3)?,
                    row.get(4)?,
                    row.get(5)?,
                    row.get(6)?,
                ))
            })
            .map_err(|e| e.to_string())?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| e.to_string())?;

        let mut operations = Vec::new();
        let mut conflicts = Vec::new();

        for (id, creator, name, version, file_path, size_bytes, resource_types_json) in &packages {
            if !is_under_dir(file_path, &config.source_dir) {
                continue;
            }

            let resource_types: Vec<String> =
                serde_json::from_str(resource_types_json).unwrap_or_default();
            let resource_types = normalize_resource_types(resource_types);
            let primary_type = primary_resource_type(resource_types.clone());

            // Determine target subdirectory based on mode
            let target_subdir = match config.mode {
                MigrationMode::ByType => {
                    localized_type_folder(&primary_type, config.locale.as_deref())
                }
                MigrationMode::ByCreator => creator.clone(),
                MigrationMode::ByScene => {
                    if resource_types.contains(&"scene".to_string()) {
                        format!("{}", creator)
                    } else {
                        // Non-scene packages go to "dependencies" folder
                        "dependencies".to_string()
                    }
                }
                MigrationMode::Custom => "packages".to_string(),
            };

            let source = file_path.clone();
            let dest_path = Path::new(&config.target_dir)
                .join(target_subdir)
                .join(format!("{}.{}.{}.var", creator, name, version));
            let dest = dest_path.to_string_lossy().to_string();
            if source == dest {
                continue;
            }
            let action = if config.dry_run { "copy" } else { "move" };
            let conflict = if PathBuf::from(&dest).exists() {
                "overwrite"
            } else {
                "none"
            };

            // Check for destination conflicts
            if conflict == "overwrite" {
                conflicts.push(format!(
                    "Conflict: {} already exists at destination",
                    format!("{}.{}.{}", creator, name, version)
                ));
            }

            operations.push(MigrationOperation {
                package_id: id.clone(),
                source,
                destination: dest,
                action: action.to_string(),
                size_bytes: *size_bytes as u64,
                resource_type: primary_type,
                conflict: conflict.to_string(),
            });
        }

        let total_size = operations.iter().map(|o| o.size_bytes).sum();

        Ok(MigrationPreview {
            total_operations: operations.len(),
            total_size_bytes: total_size,
            operations,
            conflicts,
        })
    })
    .map_err(|e| e.to_string())
}

/// Execute a previously-previewed migration.
/// Takes the list of operations and performs the actual file moves/copies.
#[tauri::command]
pub async fn execute_migration(
    app_handle: tauri::AppHandle,
    db: State<'_, Database>,
    operations: Vec<MigrationOperation>,
) -> Result<MigrationResult, String> {
    let total = operations.len();
    let mut completed = 0usize;
    let mut failed = 0usize;
    let mut errors = Vec::new();
    let task_id = format!("mig_{}", chrono::Utc::now().timestamp());
    let mut changed_package_ids = Vec::new();
    let mut changed_paths = Vec::new();

    emit_migration_progress(
        &app_handle,
        MigrationProgress {
            task_id: task_id.clone(),
            phase: "executing".to_string(),
            completed,
            failed,
            total,
            current_package_id: None,
        },
    );

    for op in &operations {
        let source_path = PathBuf::from(&op.source);
        let dest_path = PathBuf::from(&op.destination);

        if let Err(e) = validate_operation_before_execute(op, &source_path, &dest_path) {
            let _ = record_migration_log(&db, &task_id, op, "failed", Some(&e));
            errors.push(e);
            failed += 1;
            emit_migration_progress(
                &app_handle,
                MigrationProgress {
                    task_id: task_id.clone(),
                    phase: "executing".to_string(),
                    completed,
                    failed,
                    total,
                    current_package_id: Some(op.package_id.clone()),
                },
            );
            continue;
        }

        // Create parent directories
        if let Some(parent) = dest_path.parent() {
            if !parent.exists() {
                if let Err(e) = fs::create_dir_all(parent) {
                    let error = format!("Failed to create dir {:?}: {}", parent, e);
                    let _ = record_migration_log(&db, &task_id, op, "failed", Some(&error));
                    errors.push(error);
                    failed += 1;
                    emit_migration_progress(
                        &app_handle,
                        MigrationProgress {
                            task_id: task_id.clone(),
                            phase: "executing".to_string(),
                            completed,
                            failed,
                            total,
                            current_package_id: Some(op.package_id.clone()),
                        },
                    );
                    continue;
                }
            }
        }

        let result = match op.action.as_str() {
            "copy" => fs::copy(&source_path, &dest_path).map(|_| ()),
            "move" => fs::rename(&source_path, &dest_path),
            _ => {
                let error = format!("Unknown action: {}", op.action);
                let _ = record_migration_log(&db, &task_id, op, "failed", Some(&error));
                errors.push(error);
                failed += 1;
                emit_migration_progress(
                    &app_handle,
                    MigrationProgress {
                        task_id: task_id.clone(),
                        phase: "executing".to_string(),
                        completed,
                        failed,
                        total,
                        current_package_id: Some(op.package_id.clone()),
                    },
                );
                continue;
            }
        };

        match result {
            Ok(_) => {
                if op.action == "move" {
                    db.with_conn(|conn| {
                        conn.execute(
                            "UPDATE packages SET file_path = ?1 WHERE id = ?2",
                            rusqlite::params![op.destination, op.package_id],
                        )
                        .map_err(|e| AppError::Database(e.to_string()))?;
                        conn.execute(
                            "UPDATE physical_packages SET file_path = ?1 WHERE file_path = ?2",
                            rusqlite::params![op.destination, op.source],
                        )
                        .map_err(|e| AppError::Database(e.to_string()))?;
                        Ok(())
                    })
                    .map_err(|e| e.to_string())?;
                }
                record_migration_log(&db, &task_id, op, "completed", None)?;
                if op.action == "move" {
                    changed_package_ids.push(op.package_id.clone());
                    changed_paths.push(op.destination.clone());
                }
                completed += 1;
                emit_migration_progress(
                    &app_handle,
                    MigrationProgress {
                        task_id: task_id.clone(),
                        phase: "executing".to_string(),
                        completed,
                        failed,
                        total,
                        current_package_id: Some(op.package_id.clone()),
                    },
                );
            }
            Err(e) => {
                let error = format!(
                    "Failed to {} {} -> {}: {}",
                    op.action, op.source, op.destination, e
                );
                let _ = record_migration_log(&db, &task_id, op, "failed", Some(&error));
                errors.push(error);
                failed += 1;
                emit_migration_progress(
                    &app_handle,
                    MigrationProgress {
                        task_id: task_id.clone(),
                        phase: "executing".to_string(),
                        completed,
                        failed,
                        total,
                        current_package_id: Some(op.package_id.clone()),
                    },
                );
            }
        }
    }

    if !changed_package_ids.is_empty() {
        crate::commands::library_events::emit_library_index_changed(
            &app_handle,
            "package_moved",
            &["packages", "dashboard", "statistics", "folders", "dedupe"],
            changed_package_ids,
            changed_paths,
        );
    }

    emit_migration_progress(
        &app_handle,
        MigrationProgress {
            task_id: task_id.clone(),
            phase: "done".to_string(),
            completed,
            failed,
            total,
            current_package_id: None,
        },
    );

    Ok(MigrationResult {
        task_id,
        completed,
        failed,
        total,
        errors,
        rollback_available: completed > 0,
    })
}

#[tauri::command]
pub async fn rollback_migration(
    app_handle: tauri::AppHandle,
    db: State<'_, Database>,
    task_id: String,
) -> Result<MigrationRollbackResult, String> {
    let entries = db
        .with_conn(|conn| {
            let mut stmt = conn
                .prepare(
                    "SELECT id, package_id, source_path, destination_path, action
                     FROM resource_migration_log
                     WHERE task_id = ?1 AND status = 'completed' AND rolled_back_at IS NULL
                     ORDER BY id DESC",
                )
                .map_err(|e| AppError::Database(e.to_string()))?;
            let rows = stmt
                .query_map(rusqlite::params![&task_id], |row| {
                    Ok((
                        row.get::<_, i64>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, String>(2)?,
                        row.get::<_, String>(3)?,
                        row.get::<_, String>(4)?,
                    ))
                })
                .map_err(|e| AppError::Database(e.to_string()))?
                .collect::<Result<Vec<_>, _>>()
                .map_err(|e| AppError::Database(e.to_string()))?;
            Ok(rows)
        })
        .map_err(|e| e.to_string())?;

    let mut restored = 0usize;
    let mut failed = 0usize;
    let mut errors = Vec::new();
    let mut changed_package_ids = Vec::new();
    let mut changed_paths = Vec::new();
    let total = entries.len();

    emit_migration_progress(
        &app_handle,
        MigrationProgress {
            task_id: task_id.clone(),
            phase: "rollback".to_string(),
            completed: restored,
            failed,
            total,
            current_package_id: None,
        },
    );

    for (log_id, package_id, source, destination, action) in entries {
        let source_path = PathBuf::from(&source);
        let destination_path = PathBuf::from(&destination);
        let result = match action.as_str() {
            "move" => rollback_move(&source_path, &destination_path),
            "copy" => rollback_copy(&destination_path),
            _ => Err(format!("未知迁移动作，无法回滚: {}", action)),
        };

        match result {
            Ok(()) => {
                db.with_conn(|conn| {
                    conn.execute(
                        "UPDATE resource_migration_log SET rolled_back_at = datetime('now') WHERE id = ?1",
                        rusqlite::params![log_id],
                    )
                    .map_err(|e| AppError::Database(e.to_string()))?;
                    if action == "move" {
                        conn.execute(
                            "UPDATE packages SET file_path = ?1, updated_at = datetime('now') WHERE id = ?2",
                            rusqlite::params![source, package_id],
                        )
                        .map_err(|e| AppError::Database(e.to_string()))?;
                        conn.execute(
                            "UPDATE physical_packages SET file_path = ?1 WHERE file_path = ?2",
                            rusqlite::params![source, destination],
                        )
                        .map_err(|e| AppError::Database(e.to_string()))?;
                    }
                    Ok(())
                })
                .map_err(|e| e.to_string())?;
                if action == "move" {
                    changed_package_ids.push(package_id.clone());
                    changed_paths.push(source.clone());
                }
                restored += 1;
                emit_migration_progress(
                    &app_handle,
                    MigrationProgress {
                        task_id: task_id.clone(),
                        phase: "rollback".to_string(),
                        completed: restored,
                        failed,
                        total,
                        current_package_id: Some(package_id),
                    },
                );
            }
            Err(e) => {
                failed += 1;
                errors.push(e);
                emit_migration_progress(
                    &app_handle,
                    MigrationProgress {
                        task_id: task_id.clone(),
                        phase: "rollback".to_string(),
                        completed: restored,
                        failed,
                        total,
                        current_package_id: Some(package_id),
                    },
                );
            }
        }
    }

    if !changed_package_ids.is_empty() {
        crate::commands::library_events::emit_library_index_changed(
            &app_handle,
            "package_moved",
            &["packages", "dashboard", "statistics", "folders", "dedupe"],
            changed_package_ids,
            changed_paths,
        );
    }

    emit_migration_progress(
        &app_handle,
        MigrationProgress {
            task_id: task_id.clone(),
            phase: "done".to_string(),
            completed: restored,
            failed,
            total,
            current_package_id: None,
        },
    );

    Ok(MigrationRollbackResult {
        task_id,
        restored,
        failed,
        errors,
    })
}

#[tauri::command]
pub async fn rollback_all_migrations(
    app_handle: tauri::AppHandle,
    db: State<'_, Database>,
) -> Result<MigrationRollbackResult, String> {
    let entries = db
        .with_conn(|conn| {
            let mut stmt = conn
                .prepare(
                    "SELECT id, package_id, source_path, destination_path, action
                     FROM resource_migration_log
                     WHERE status = 'completed' AND rolled_back_at IS NULL
                     ORDER BY id DESC",
                )
                .map_err(|e| AppError::Database(e.to_string()))?;
            let rows = stmt
                .query_map([], |row| {
                    Ok((
                        row.get::<_, i64>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, String>(2)?,
                        row.get::<_, String>(3)?,
                        row.get::<_, String>(4)?,
                    ))
                })
                .map_err(|e| AppError::Database(e.to_string()))?
                .collect::<Result<Vec<_>, _>>()
                .map_err(|e| AppError::Database(e.to_string()))?;
            Ok(rows)
        })
        .map_err(|e| e.to_string())?;

    let task_id = "all".to_string();
    let total = entries.len();
    let mut restored = 0usize;
    let mut failed = 0usize;
    let mut errors = Vec::new();
    let mut changed_package_ids = Vec::new();
    let mut changed_paths = Vec::new();

    emit_migration_progress(
        &app_handle,
        MigrationProgress {
            task_id: task_id.clone(),
            phase: "rollback_all".to_string(),
            completed: restored,
            failed,
            total,
            current_package_id: None,
        },
    );

    for (log_id, package_id, source, destination, action) in entries {
        let source_path = PathBuf::from(&source);
        let destination_path = PathBuf::from(&destination);
        let result = match action.as_str() {
            "move" => rollback_move(&source_path, &destination_path),
            "copy" => rollback_copy(&destination_path),
            _ => Err(format!("未知迁移动作，无法恢复: {}", action)),
        };

        match result {
            Ok(()) => {
                db.with_conn(|conn| {
                    conn.execute(
                        "UPDATE resource_migration_log SET rolled_back_at = datetime('now') WHERE id = ?1",
                        rusqlite::params![log_id],
                    )
                    .map_err(|e| AppError::Database(e.to_string()))?;
                    if action == "move" {
                        conn.execute(
                            "UPDATE packages SET file_path = ?1, updated_at = datetime('now') WHERE id = ?2",
                            rusqlite::params![source, package_id],
                        )
                        .map_err(|e| AppError::Database(e.to_string()))?;
                        conn.execute(
                            "UPDATE physical_packages SET file_path = ?1 WHERE file_path = ?2",
                            rusqlite::params![source, destination],
                        )
                        .map_err(|e| AppError::Database(e.to_string()))?;
                    }
                    Ok(())
                })
                .map_err(|e| e.to_string())?;
                if action == "move" {
                    changed_package_ids.push(package_id.clone());
                    changed_paths.push(source.clone());
                }
                restored += 1;
            }
            Err(e) => {
                failed += 1;
                errors.push(e);
            }
        }

        emit_migration_progress(
            &app_handle,
            MigrationProgress {
                task_id: task_id.clone(),
                phase: "rollback_all".to_string(),
                completed: restored,
                failed,
                total,
                current_package_id: Some(package_id),
            },
        );
    }

    if !changed_package_ids.is_empty() {
        crate::commands::library_events::emit_library_index_changed(
            &app_handle,
            "package_moved",
            &["packages", "dashboard", "statistics", "folders", "dedupe"],
            changed_package_ids,
            changed_paths,
        );
    }

    emit_migration_progress(
        &app_handle,
        MigrationProgress {
            task_id: task_id.clone(),
            phase: "done".to_string(),
            completed: restored,
            failed,
            total,
            current_package_id: None,
        },
    );

    Ok(MigrationRollbackResult {
        task_id,
        restored,
        failed,
        errors,
    })
}

fn emit_migration_progress(app_handle: &tauri::AppHandle, progress: MigrationProgress) {
    let _ = app_handle.emit("migration-progress", progress);
}

fn record_migration_log(
    db: &Database,
    task_id: &str,
    op: &MigrationOperation,
    status: &str,
    error: Option<&str>,
) -> Result<(), String> {
    db.with_conn(|conn| {
        conn.execute(
            "INSERT INTO resource_migration_log (
                task_id, package_id, source_path, destination_path, action,
                size_bytes, status, error, completed_at
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8,
                CASE WHEN ?7 = 'completed' THEN datetime('now') ELSE NULL END)",
            rusqlite::params![
                task_id,
                op.package_id,
                op.source,
                op.destination,
                op.action,
                op.size_bytes as i64,
                status,
                error,
            ],
        )
        .map_err(|e| AppError::Database(e.to_string()))?;
        Ok(())
    })
    .map_err(|e| e.to_string())
}

fn rollback_move(source_path: &Path, destination_path: &Path) -> Result<(), String> {
    if !destination_path.exists() {
        return Err(format!(
            "迁移目标不存在，无法回滚: {}",
            destination_path.display()
        ));
    }
    if source_path.exists() {
        return Err(format!(
            "原路径已存在，无法回滚覆盖: {}",
            source_path.display()
        ));
    }
    if let Some(parent) = source_path.parent() {
        fs::create_dir_all(parent)
            .map_err(|e| format!("创建回滚目录失败 {}: {}", parent.display(), e))?;
    }
    fs::rename(destination_path, source_path).map_err(|e| {
        format!(
            "回滚移动失败 {} -> {}: {}",
            destination_path.display(),
            source_path.display(),
            e
        )
    })
}

fn rollback_copy(destination_path: &Path) -> Result<(), String> {
    if !destination_path.exists() {
        return Ok(());
    }
    fs::remove_file(destination_path).map_err(|e| {
        format!(
            "删除复制产生的文件失败 {}: {}",
            destination_path.display(),
            e
        )
    })
}

fn validate_operation_before_execute(
    op: &MigrationOperation,
    source_path: &Path,
    dest_path: &Path,
) -> Result<(), String> {
    if !source_path.exists() {
        return Err(format!("源文件不存在，已跳过: {}", source_path.display()));
    }

    let metadata = fs::metadata(source_path)
        .map_err(|e| format!("读取源文件状态失败 {}: {}", source_path.display(), e))?;
    if metadata.len() != op.size_bytes {
        return Err(format!(
            "源文件大小已变化，已跳过: {}",
            source_path.display()
        ));
    }

    if dest_path.exists() {
        return Err(format!(
            "目标文件已存在，执行前二次校验阻止覆盖: {}",
            dest_path.display()
        ));
    }

    if let Some(parent) = dest_path.parent() {
        if parent.exists() && !parent.is_dir() {
            return Err(format!("目标父路径不是目录: {}", parent.display()));
        }
    }

    if !has_available_space(dest_path, op.size_bytes)? {
        return Err(format!("目标磁盘空间不足，已跳过: {}", dest_path.display()));
    }

    Ok(())
}

#[cfg(windows)]
fn has_available_space(dest_path: &Path, required_bytes: u64) -> Result<bool, String> {
    use std::os::windows::ffi::OsStrExt;

    // Find the nearest existing parent directory
    let mut check_path = dest_path.to_path_buf();
    while !check_path.exists() {
        if let Some(parent) = check_path.parent() {
            check_path = parent.to_path_buf();
        } else {
            break;
        }
    }

    let wide_path: Vec<u16> = check_path
        .as_os_str()
        .encode_wide()
        .chain(std::iter::once(0))
        .collect();

    extern "system" {
        fn GetDiskFreeSpaceExW(
            lpDirectoryName: *const u16,
            lpFreeBytesAvailableToCaller: *mut u64,
            lpTotalNumberOfBytes: *mut u64,
            lpTotalNumberOfFreeBytes: *mut u64,
        ) -> i32;
    }

    let mut free_bytes: u64 = 0;
    let mut total_bytes: u64 = 0;
    let mut total_free_bytes: u64 = 0;

    let res = unsafe {
        GetDiskFreeSpaceExW(
            wide_path.as_ptr(),
            &mut free_bytes,
            &mut total_bytes,
            &mut total_free_bytes,
        )
    };

    if res == 0 {
        let err = std::io::Error::last_os_error();
        return Err(format!(
            "获取磁盘空间失败 (路径: {}): {}",
            check_path.display(),
            err
        ));
    }

    Ok(free_bytes >= required_bytes)
}

#[cfg(not(windows))]
fn has_available_space(_dest_path: &Path, _required_bytes: u64) -> Result<bool, String> {
    Ok(true)
}

/// Collect all dependencies for a scene package into a target directory
#[tauri::command]
pub async fn collect_scene_dependencies(
    db: State<'_, Database>,
    scene_package_id: String,
    target_dir: String,
) -> Result<MigrationPreview, String> {
    db.with_conn(|conn| {
        // Get all dependencies of the scene package
        let mut dep_stmt = conn
            .prepare(
                "SELECT DISTINCT p.id, p.creator, p.name, p.version, p.file_path, p.size_bytes, p.resource_types
                 FROM dependencies d
                 JOIN packages p ON p.id = d.depends_on_id
                 WHERE d.package_id = ?1",
            )
            .map_err(|e| e.to_string())?;

        let deps: Vec<(String, String, String, i32, String, i64, String)> = dep_stmt
            .query_map(rusqlite::params![&scene_package_id], |row| {
                Ok((
                    row.get(0)?,
                    row.get(1)?,
                    row.get(2)?,
                    row.get(3)?,
                    row.get(4)?,
                    row.get(5)?,
                    row.get(6)?,
                ))
            })
            .map_err(|e| e.to_string())?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| e.to_string())?;

        let operations: Vec<MigrationOperation> = deps
            .iter()
            .map(|(id, creator, name, version, file_path, size_bytes, resource_types_json)| {
                let resource_types: Vec<String> =
                    serde_json::from_str(resource_types_json).unwrap_or_default();
                let primary_type = primary_resource_type(resource_types);

                let dest = format!(
                    "{}/{}.{}.{}.var",
                    target_dir, creator, name, version
                );

                MigrationOperation {
                    package_id: id.clone(),
                    source: file_path.clone(),
                    destination: dest,
                    action: "copy".to_string(),
                    size_bytes: *size_bytes as u64,
                    resource_type: primary_type,
                    conflict: if PathBuf::from(&file_path).exists() {
                        "none".to_string()
                    } else {
                        "skip".to_string()
                    },
                }
            })
            .collect();

        let total_size = operations.iter().map(|o| o.size_bytes).sum();

        Ok(MigrationPreview {
            total_operations: operations.len(),
            total_size_bytes: total_size,
            operations,
            conflicts: Vec::new(),
        })
    })
    .map_err(|e| e.to_string())
}
