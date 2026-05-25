use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use tauri::State;

use crate::db::Database;
use crate::errors::AppError;
use crate::services::install_context::resolve_install_context;

/// A single instance in a duplicate group
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DuplicateInstance {
    pub package_id: String,
    pub file_path: String,
    pub size_bytes: u64,
    pub is_recommended_keep: bool,
    #[serde(default)]
    pub source_type: String,
    #[serde(default)]
    pub link_type: Option<String>,
}

/// A group of duplicate files
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DuplicateGroup {
    pub id: String,
    pub strategy: String,
    pub file_hash: String,
    pub resource_path: String,
    pub total_wasted_bytes: u64,
    pub file_count: usize,
    pub instances: Vec<DuplicateInstance>,
}

/// Summary of deduplication scan results
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DedupSummary {
    pub duplicate_groups: usize,
    pub total_wasted_bytes: u64,
    pub safe_to_clean_count: usize,
    pub total_files: usize,
    pub scanned_files: usize,
}

/// Resolution action for a group
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DuplicateResolution {
    pub group_id: String,
    pub keep_package_ids: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CleanupTrashEntry {
    pub id: i64,
    pub package_id: String,
    pub original_path: String,
    pub trash_path: String,
    pub size_bytes: u64,
    pub created_at: String,
}

/// Helper function to compute the MD5 hash of a physical file on disk using a 64KB buffer.
fn compute_file_md5(path: &str) -> Result<String, String> {
    let mut file =
        std::fs::File::open(path).map_err(|e| format!("Failed to open file '{}': {}", path, e))?;
    let mut context = md5::Context::new();
    let mut buffer = [0u8; 65536]; // 64KB buffer
    loop {
        let n = std::io::Read::read(&mut file, &mut buffer)
            .map_err(|e| format!("Failed to read file '{}': {}", path, e))?;
        if n == 0 {
            break;
        }
        context.consume(&buffer[..n]);
    }
    let digest = context.compute();
    Ok(format!("{:x}", digest))
}

/// Scan all physical packages to find duplicate .var files based on Name and MD5.
/// Computes MD5 incrementally for packages that have multiple physical copies.
#[tauri::command]
pub async fn scan_for_duplicates(
    db: State<'_, Database>,
    _vam_root: String,
) -> Result<DedupSummary, String> {
    db.with_conn(|conn| {
        // 1. Find all package IDs that have multiple copies on disk
        let mut stmt = conn.prepare(
            "SELECT package_id, COUNT(*) as cnt
             FROM physical_packages
             GROUP BY package_id
             HAVING cnt > 1"
        ).map_err(|e| e.to_string())?;

        let duplicate_pkg_ids: Vec<String> = stmt.query_map([], |row| {
            Ok(row.get::<_, String>(0)?)
        })
        .map_err(|e| e.to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())?;

        let mut scanned = 0;

        // 2. Incremental/Lazy MD5 computation for these potential duplicates
        for pkg_id in &duplicate_pkg_ids {
            let mut stmt2 = conn.prepare(
                "SELECT file_path, file_md5 FROM physical_packages WHERE package_id = ?1"
            ).map_err(|e| e.to_string())?;

            let rows: Vec<(String, Option<String>)> = stmt2.query_map(rusqlite::params![pkg_id], |row| {
                Ok((row.get(0)?, row.get(1)?))
            })
            .map_err(|e| e.to_string())?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| e.to_string())?;

            for (file_path, existing_md5) in rows {
                if existing_md5.is_none() || existing_md5.as_ref().unwrap().is_empty() {
                    if Path::new(&file_path).exists() {
                        if let Ok(md5_val) = compute_file_md5(&file_path) {
                            conn.execute(
                                "UPDATE physical_packages SET file_md5 = ?1 WHERE file_path = ?2",
                                rusqlite::params![md5_val, file_path],
                            ).map_err(|e| e.to_string())?;
                        }
                    }
                    scanned += 1;
                }
            }
        }

        // 3. Find true duplicate groups (same package_id and same MD5)
        let mut group_stmt = conn.prepare(
            "SELECT package_id, file_md5, COUNT(*) as cnt, SUM(size_bytes) as total_size, MAX(size_bytes) as single_size
             FROM physical_packages
             WHERE file_md5 IS NOT NULL AND file_md5 != ''
             GROUP BY package_id, file_md5
             HAVING cnt > 1"
        ).map_err(|e| e.to_string())?;

        let groups: Vec<(String, String, i64, i64, i64)> = group_stmt.query_map([], |row| {
            Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?, row.get(4)?))
        })
        .map_err(|e| e.to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())?;

        let duplicate_groups = groups.len();
        let mut total_wasted_bytes = 0u64;

        for (_pkg_id, _md5, _cnt, total_size, single_size) in &groups {
            // Wasted space = total size - size of 1 kept copy
            let wasted = *total_size as u64 - *single_size as u64;
            total_wasted_bytes += wasted;
        }

        let total_files: usize = conn.query_row(
            "SELECT COUNT(*) FROM physical_packages",
            [],
            |row| row.get(0)
        ).unwrap_or(0);

        Ok(DedupSummary {
            duplicate_groups,
            total_wasted_bytes,
            safe_to_clean_count: duplicate_groups,
            scanned_files: scanned,
            total_files,
        })
    })
    .map_err(|e| e.to_string())
}

/// Retrieve all duplicate physical package groups with their instances.
#[tauri::command]
pub async fn get_duplicate_groups(
    app_handle: tauri::AppHandle,
    db: State<'_, Database>,
) -> Result<Vec<DuplicateGroup>, String> {
    let manifest_link_types = load_manifest_link_types(&app_handle);
    db.with_conn(|conn| {
        let mut group_stmt = conn.prepare(
            "SELECT package_id, file_md5, COUNT(*) as cnt, SUM(size_bytes) as total_size, MAX(size_bytes) as single_size
             FROM physical_packages
             WHERE file_md5 IS NOT NULL AND file_md5 != ''
             GROUP BY package_id, file_md5
             HAVING cnt > 1"
        ).map_err(|e| e.to_string())?;

        let groups: Vec<(String, String, i64, i64, i64)> = group_stmt.query_map([], |row| {
            Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?, row.get(4)?))
        })
        .map_err(|e| e.to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())?;

        let mut dup_groups = Vec::new();

        for (pkg_id, md5, cnt, total_size, single_size) in groups {
            let mut inst_stmt = conn.prepare(
                "SELECT file_path, size_bytes FROM physical_packages WHERE package_id = ?1 AND file_md5 = ?2"
            ).map_err(|e| e.to_string())?;

            let mut instances: Vec<(String, i64)> = inst_stmt.query_map(rusqlite::params![pkg_id, md5], |row| {
                Ok((row.get(0)?, row.get(1)?))
            })
            .map_err(|e| e.to_string())?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| e.to_string())?;

            // Sort instances by length of file path (recommend keeping shortest path, which is typically cleaner/closer to root)
            instances.sort_by(|a, b| a.0.len().cmp(&b.0.len()));

            let mut dup_instances = Vec::new();
            for (idx, (file_path, size)) in instances.iter().enumerate() {
                let path_obj = Path::new(file_path);
                
                // Get parent directory name for display in UI
                let folder_name = path_obj.parent()
                    .and_then(|p| p.file_name())
                    .and_then(|f| f.to_str())
                    .unwrap_or("AddonPackages")
                    .to_string();

                let (source_type, link_type) =
                    classify_physical_file(path_obj, Some(&manifest_link_types));
                dup_instances.push(DuplicateInstance {
                    package_id: folder_name, // Displays folder name (e.g. "Subfolder" or "AddonPackages")
                    file_path: file_path.clone(),
                    size_bytes: *size as u64,
                    is_recommended_keep: idx == 0,
                    source_type,
                    link_type,
                });
            }

            let wasted = total_size as u64 - single_size as u64;

            dup_groups.push(DuplicateGroup {
                id: format!("group_{}", md5.chars().take(8).collect::<String>()),
                strategy: "exact".to_string(),
                file_hash: md5,
                resource_path: pkg_id,
                total_wasted_bytes: wasted,
                file_count: cnt as usize,
                instances: dup_instances,
            });
        }

        Ok(dup_groups)
    })
    .map_err(|e| e.to_string())
}

/// 将重复 .var 文件移入 VAM Library 回收站，并同步更新数据库引用。
#[tauri::command]
pub async fn execute_cleanup(
    app_handle: tauri::AppHandle,
    db: State<'_, Database>,
    instances: Vec<DuplicateInstance>,
) -> Result<usize, String> {
    let install_context = resolve_install_context(&app_handle)?;
    let trash_dir = PathBuf::from(install_context.vam_root)
        .join("VAMBoxLibrary")
        .join(".trash");
    std::fs::create_dir_all(&trash_dir)
        .map_err(|e| format!("创建回收站目录失败 {}: {}", trash_dir.display(), e))?;

    let cleaned_count = db
        .with_conn(|conn| {
            let mut cleaned_count = 0;

            for instance in &instances {
                // 1. Query the package_id associated with this file_path before deleting
                let package_id_opt: Option<String> = conn
                    .query_row(
                        "SELECT package_id FROM physical_packages WHERE file_path = ?1",
                        rusqlite::params![instance.file_path],
                        |row| row.get(0),
                    )
                    .ok();

                let package_id = match package_id_opt {
                    Some(id) => id,
                    None => continue, // If not found in physical_packages, skip
                };

                // 2. 移入回收站，不做永久删除。
                let path = Path::new(&instance.file_path);
                if path.exists() {
                    let trash_path = build_trash_path(&trash_dir, path);
                    if let Err(e) = std::fs::rename(path, &trash_path) {
                        log::error!("Failed to move duplicate file {}: {}", path.display(), e);
                        continue;
                    }
                    conn.execute(
                    "INSERT INTO cleanup_trash (package_id, original_path, trash_path, size_bytes)
                     VALUES (?1, ?2, ?3, ?4)",
                    rusqlite::params![
                        package_id,
                        instance.file_path,
                        trash_path.to_string_lossy().to_string(),
                        instance.size_bytes as i64
                    ],
                )
                .map_err(|e| e.to_string())?;
                    cleaned_count += 1;
                }

                // 3. Delete from physical_packages
                let _ = conn.execute(
                    "DELETE FROM physical_packages WHERE file_path = ?1",
                    rusqlite::params![instance.file_path],
                );

                // 4. Update or delete the main packages table record
                let remaining_path: Option<String> = conn
                    .query_row(
                        "SELECT file_path FROM physical_packages WHERE package_id = ?1 LIMIT 1",
                        rusqlite::params![package_id],
                        |row| row.get(0),
                    )
                    .ok();

                if let Some(rem_path) = remaining_path {
                    // If there's still another physical copy left, update packages to point to it
                    let _ = conn.execute(
                        "UPDATE packages SET file_path = ?1 WHERE id = ?2",
                        rusqlite::params![rem_path, package_id],
                    );
                } else {
                    // If NO physical copies left, delete from the packages table entirely
                    let _ = conn.execute(
                        "DELETE FROM packages WHERE id = ?1",
                        rusqlite::params![package_id],
                    );
                }
            }
            Ok(cleaned_count)
        })
        .map_err(|e| e.to_string())?;

    if cleaned_count > 0 {
        crate::commands::library_events::emit_library_index_changed(
            &app_handle,
            "dedupe_changed",
            &[
                "packages",
                "dashboard",
                "dependencies",
                "statistics",
                "folders",
                "dedupe",
            ],
            Vec::new(),
            instances
                .iter()
                .map(|instance| instance.file_path.clone())
                .collect(),
        );
    }

    Ok(cleaned_count)
}

pub fn purge_expired_trash_internal(conn: &rusqlite::Connection) -> Result<usize, AppError> {
    let mut stmt = conn.prepare(
        "SELECT id, trash_path FROM cleanup_trash
         WHERE restored_at IS NULL AND created_at < datetime('now', '-1 day')"
    )?;
    let expired_items = stmt.query_map([], |row| {
        Ok((row.get::<_, i64>(0)?, row.get::<_, String>(1)?))
    })?.collect::<Result<Vec<_>, _>>()?;

    let mut purged_count = 0;
    for (id, trash_path) in expired_items {
        let path = Path::new(&trash_path);
        if path.exists() {
            let _ = std::fs::remove_file(path);
        }
        conn.execute("DELETE FROM cleanup_trash WHERE id = ?1", [id])?;
        purged_count += 1;
    }
    Ok(purged_count)
}

#[tauri::command]
pub async fn list_cleanup_trash(db: State<'_, Database>) -> Result<Vec<CleanupTrashEntry>, String> {
    // Proactively purge expired trash first!
    let _ = db.with_conn(|conn| {
        purge_expired_trash_internal(conn)
    });

    db.with_conn(|conn| {
        let mut stmt = conn
            .prepare(
                "SELECT id, package_id, original_path, trash_path, size_bytes, created_at
                 FROM cleanup_trash
                 WHERE restored_at IS NULL
                 ORDER BY created_at DESC, id DESC",
            )
            .map_err(|e| e.to_string())?;

        let rows = stmt
            .query_map([], |row| {
                Ok(CleanupTrashEntry {
                    id: row.get(0)?,
                    package_id: row.get(1)?,
                    original_path: row.get(2)?,
                    trash_path: row.get(3)?,
                    size_bytes: row.get::<_, i64>(4)? as u64,
                    created_at: row.get(5)?,
                })
            })
            .map_err(|e| e.to_string())?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| e.to_string())?;
        Ok(rows)
    })
    .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn delete_cleanup_trash_item(
    db: State<'_, Database>,
    trash_id: i64,
) -> Result<(), String> {
    db.with_conn(|conn| {
        let trash_path: String = conn.query_row(
            "SELECT trash_path FROM cleanup_trash WHERE id = ?1",
            [trash_id],
            |row| row.get(0),
        ).map_err(|e| AppError::Database(format!("未找到回收站项: {}", e)))?;

        let path = Path::new(&trash_path);
        if path.exists() {
            std::fs::remove_file(path).map_err(|e| {
                AppError::Io(format!("永久删除物理文件失败: {}", e))
            })?;
        }

        conn.execute("DELETE FROM cleanup_trash WHERE id = ?1", [trash_id])
            .map_err(|e| AppError::Database(e.to_string()))?;

        Ok(())
    })
    .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn empty_cleanup_trash(db: State<'_, Database>) -> Result<usize, String> {
    db.with_conn(|conn| {
        let mut stmt = conn.prepare(
            "SELECT id, trash_path FROM cleanup_trash WHERE restored_at IS NULL"
        ).map_err(|e| AppError::Database(e.to_string()))?;

        let items = stmt.query_map([], |row| {
            Ok((row.get::<_, i64>(0)?, row.get::<_, String>(1)?))
        })
        .map_err(|e| AppError::Database(e.to_string()))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| AppError::Database(e.to_string()))?;

        let mut deleted_count = 0;
        for (id, trash_path) in items {
            let path = Path::new(&trash_path);
            if path.exists() {
                let _ = std::fs::remove_file(path);
            }
            conn.execute("DELETE FROM cleanup_trash WHERE id = ?1", [id])
                .map_err(|e| AppError::Database(e.to_string()))?;
            deleted_count += 1;
        }

        Ok(deleted_count)
    })
    .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn restore_cleanup_trash_item(
    app_handle: tauri::AppHandle,
    db: State<'_, Database>,
    trash_id: i64,
) -> Result<(), String> {
    let package_id = db
        .with_conn(|conn| {
            let (package_id, original_path, trash_path, size_bytes): (String, String, String, i64) =
                conn.query_row(
                    "SELECT package_id, original_path, trash_path, size_bytes
                 FROM cleanup_trash
                 WHERE id = ?1 AND restored_at IS NULL",
                    rusqlite::params![trash_id],
                    |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
                )
                .map_err(|e| AppError::Database(e.to_string()))?;

            let trash_path_buf = PathBuf::from(&trash_path);
            let original_path_buf = PathBuf::from(&original_path);
            if !trash_path_buf.exists() {
                return Err(AppError::Io(format!(
                    "回收站文件不存在: {}",
                    trash_path_buf.display()
                )));
            }
            if original_path_buf.exists() {
                return Err(AppError::Io(format!(
                    "原路径已存在，无法恢复: {}",
                    original_path_buf.display()
                )));
            }
            if let Some(parent) = original_path_buf.parent() {
                std::fs::create_dir_all(parent).map_err(|e| {
                    AppError::Io(format!("创建恢复目录失败 {}: {}", parent.display(), e))
                })?;
            }
            std::fs::rename(&trash_path_buf, &original_path_buf)
                .map_err(|e| AppError::Io(format!("恢复文件失败: {}", e)))?;

            conn.execute(
            "INSERT OR REPLACE INTO physical_packages (file_path, package_id, size_bytes, scan_time)
             VALUES (?1, ?2, ?3, datetime('now'))",
            rusqlite::params![original_path, package_id, size_bytes],
        )
        .map_err(|e| AppError::Database(e.to_string()))?;
            conn.execute(
                "UPDATE packages SET file_path = ?1, updated_at = datetime('now') WHERE id = ?2",
                rusqlite::params![original_path, package_id],
            )
            .map_err(|e| AppError::Database(e.to_string()))?;
            conn.execute(
                "UPDATE cleanup_trash SET restored_at = datetime('now') WHERE id = ?1",
                rusqlite::params![trash_id],
            )
            .map_err(|e| AppError::Database(e.to_string()))?;

            Ok(package_id)
        })
        .map_err(|e| e.to_string())?;

    crate::commands::library_events::emit_library_index_changed(
        &app_handle,
        "dedupe_changed",
        &[
            "packages",
            "dashboard",
            "dependencies",
            "statistics",
            "folders",
            "dedupe",
        ],
        vec![package_id],
        Vec::new(),
    );

    Ok(())
}

/// Legacy command for previewing cleanup (kept for backward compatibility, though UI calls execute_cleanup)
#[tauri::command]
pub async fn preview_cleanup(
    db: State<'_, Database>,
    resolutions: Vec<DuplicateResolution>,
) -> Result<Vec<DuplicateInstance>, String> {
    db.with_conn(|conn| {
        let mut to_delete = Vec::new();

        for resolution in &resolutions {
            let hash_prefix = resolution.group_id.strip_prefix("group_").unwrap_or("");

            let mut stmt = conn
                .prepare(
                    "SELECT file_path, package_id, size_bytes
                 FROM physical_packages
                 WHERE file_md5 LIKE ?1",
                )
                .map_err(|e| e.to_string())?;

            let pattern = format!("{}%", hash_prefix);
            let instances: Vec<(String, String, i64)> = stmt
                .query_map(rusqlite::params![&pattern], |row| {
                    Ok((row.get(0)?, row.get(1)?, row.get(2)?))
                })
                .map_err(|e| e.to_string())?
                .collect::<Result<Vec<_>, _>>()
                .map_err(|e| e.to_string())?;

            for (file_path, pkg_id, size_bytes) in instances {
                if resolution.keep_package_ids.contains(&pkg_id) {
                    continue;
                }

                to_delete.push(DuplicateInstance {
                    package_id: pkg_id,
                    source_type: classify_physical_file(Path::new(&file_path), None).0,
                    link_type: classify_physical_file(Path::new(&file_path), None).1,
                    file_path,
                    size_bytes: size_bytes as u64,
                    is_recommended_keep: false,
                });
            }
        }

        Ok(to_delete)
    })
    .map_err(|e| e.to_string())
}

fn build_trash_path(trash_dir: &Path, original_path: &Path) -> PathBuf {
    let file_name = original_path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("package.var");
    let stamp = chrono::Utc::now().timestamp_millis();
    trash_dir.join(format!("{}_{}", stamp, file_name))
}

fn load_manifest_link_types(app_handle: &tauri::AppHandle) -> HashMap<String, String> {
    let Ok(context) = resolve_install_context(app_handle) else {
        return HashMap::new();
    };
    let Ok(content) = std::fs::read_to_string(context.manifest_path) else {
        return HashMap::new();
    };
    let Ok(value) = serde_json::from_str::<serde_json::Value>(&content) else {
        return HashMap::new();
    };

    value
        .get("links")
        .and_then(|links| links.as_array())
        .map(|links| {
            links
                .iter()
                .filter_map(|entry| {
                    let link = entry.get("link")?.as_str()?.to_string();
                    let link_type = entry.get("link_type")?.as_str()?.to_string();
                    Some((normalize_path_key(&link), link_type))
                })
                .collect()
        })
        .unwrap_or_default()
}

fn classify_physical_file(
    path: &Path,
    manifest_link_types: Option<&HashMap<String, String>>,
) -> (String, Option<String>) {
    let path_key = normalize_path_key(&path.to_string_lossy());
    if let Some(link_type) = manifest_link_types.and_then(|links| links.get(&path_key)) {
        return (link_type.clone(), Some(link_type.clone()));
    }

    if std::fs::symlink_metadata(path)
        .map(|metadata| metadata.file_type().is_symlink())
        .unwrap_or(false)
    {
        return ("symlink".to_string(), Some("symlink".to_string()));
    }

    if has_multiple_links(path) {
        return ("hard_link".to_string(), Some("hard_link".to_string()));
    }

    let normalized = path.to_string_lossy().replace('\\', "/").to_lowercase();
    if normalized.contains("/VAMBoxLibrary/addonpackages/") {
        ("managed_library".to_string(), None)
    } else if normalized.contains("/addonpackages/") {
        ("real_file".to_string(), None)
    } else {
        ("external".to_string(), None)
    }
}

fn normalize_path_key(path: &str) -> String {
    path.replace('\\', "/").to_lowercase()
}

#[cfg(windows)]
fn has_multiple_links(path: &Path) -> bool {
    let _ = path;
    false
}

#[cfg(unix)]
fn has_multiple_links(path: &Path) -> bool {
    use std::os::unix::fs::MetadataExt;
    std::fs::metadata(path)
        .map(|metadata| metadata.nlink() > 1)
        .unwrap_or(false)
}

#[cfg(not(any(windows, unix)))]
fn has_multiple_links(_path: &Path) -> bool {
    false
}
