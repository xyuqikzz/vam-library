use crate::db::Database;
use crate::errors::AppError;
use crate::services::install_context::resolve_install_context;
use crate::services::resource_dedup::{self, DedupSnapshot, DedupSummary, DuplicateGroup};
use crate::services::resource_files::{self as files};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use tauri::{Manager, State};

#[derive(Default)]
pub struct DedupState(pub Mutex<Option<DedupSnapshot>>);

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CleanupTrashEntry {
    pub id: i64,
    pub package_id: String,
    pub original_path: String,
    pub trash_path: String,
    pub size_bytes: u64,
    pub created_at: String,
}

#[derive(Debug, Serialize)]
pub struct CleanupResult {
    pub cleaned: usize,
    pub archived: usize,
    pub removed_directories: usize,
    pub errors: Vec<String>,
}

#[tauri::command]
pub async fn scan_for_duplicates(
    app_handle: tauri::AppHandle,
    vam_root: String,
) -> Result<DedupSummary, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let _guard = files::maintenance_lock()?;
        let mut root = PathBuf::from(vam_root);
        if root.join("AddonPackages").is_dir() {
            root = root.join("AddonPackages");
        }
        let snapshot = resource_dedup::scan(
            &root,
            load_manifest_link_types(&app_handle).into_keys().collect(),
        )?;
        let summary = snapshot.summary.clone();
        *app_handle
            .state::<DedupState>()
            .0
            .lock()
            .map_err(|e| e.to_string())? = Some(snapshot);
        Ok(summary)
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
pub async fn get_duplicate_groups(
    state: State<'_, DedupState>,
) -> Result<Vec<DuplicateGroup>, String> {
    Ok(state
        .0
        .lock()
        .map_err(|e| e.to_string())?
        .as_ref()
        .map(|s| s.groups.clone())
        .unwrap_or_default())
}

#[tauri::command]
pub async fn execute_cleanup(
    app_handle: tauri::AppHandle,
    scan_id: String,
    file_paths: Vec<String>,
) -> Result<CleanupResult, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let _guard = files::maintenance_lock()?;
        // Consume the preview once. A partial failure must be rescanned before retrying.
        let snapshot = {
            let state = app_handle.state::<DedupState>();
            let mut state = state.0.lock().map_err(|e| e.to_string())?;
            if state.as_ref().map(|s| &s.summary.scan_id) != Some(&scan_id) {
                return Err("扫描已过期，请重新扫描".into());
            }
            state.take().unwrap()
        };
        let context = resolve_install_context(&app_handle)?;
        let trash = PathBuf::from(context.vam_root)
            .join("VAMBoxLibrary")
            .join(".trash");
        let result = organize_snapshot(
            &app_handle.state::<Database>(),
            &snapshot,
            &trash,
            &file_paths,
        )?;
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
            vec![],
            file_paths,
        );
        Ok(result)
    })
    .await
    .map_err(|e| e.to_string())?
}

fn organize_snapshot(
    db: &Database,
    snapshot: &DedupSnapshot,
    trash: &Path,
    file_paths: &[String],
) -> Result<CleanupResult, String> {
    let scan_id = &snapshot.summary.scan_id;
    let deletions = resource_dedup::validate_selection(&snapshot, &file_paths)?;
    let selected: HashSet<_> = file_paths.iter().map(|p| files::path_key(p)).collect();
    let mut archives = Vec::new();
    for item in snapshot.groups.iter().flat_map(|g| &g.instances) {
        if let Some(destination) = &item.archive_destination {
            files::ensure_no_links(Path::new(destination))?;
            if Path::new(destination).exists() && !selected.contains(&files::path_key(destination))
            {
                return Err(format!(
                    "依赖旧版本目标已存在且未列入清理，已停止，请先处理冲突: {}",
                    destination
                ));
            }
            archives.push((item.clone(), destination.clone()));
        }
    }
    files::ensure_no_links(&trash)?;
    let mut result = CleanupResult {
        cleaned: 0,
        archived: 0,
        removed_directories: 0,
        errors: vec![],
    };
    for (index, item) in deletions.iter().enumerate() {
        if let Some(file) = snapshot.files.iter().find(|f| f.path == item.file_path) {
            if let Err(e) = files::verify(file) {
                result.errors.push(e);
                break;
            }
        }
        let group = snapshot
            .groups
            .iter()
            .find(|g| g.instances.iter().any(|i| i.file_path == item.file_path))
            .ok_or("清理分组已失效")?;
        if let Some(error) = group
            .instances
            .iter()
            .filter(|i| i.is_recommended_keep)
            .filter_map(|i| snapshot.files.iter().find(|f| f.path == i.file_path))
            .find_map(|f| files::verify(f).err())
        {
            result.errors.push(error);
            break;
        }
        let destination = trash.join(format!(
            "{}_{}_{}",
            scan_id,
            index,
            Path::new(&item.file_path)
                .file_name()
                .unwrap()
                .to_string_lossy()
        ));
        let action = db.with_conn(|conn| {
                let tx = conn.unchecked_transaction()?;
                tx.execute("INSERT INTO cleanup_trash (package_id, original_path, trash_path, size_bytes) VALUES (?1,?2,?3,?4)",
                    rusqlite::params![item.package_id, item.file_path, destination.to_string_lossy(), item.size_bytes])?;
                files::transfer(Path::new(&item.file_path), &destination, "move").map_err(AppError::Io)?;
                let update = (|| -> Result<(), AppError> {
                    tx.execute("DELETE FROM physical_packages WHERE lower(replace(file_path, '\\', '/')) = ?1", [files::path_key(&item.file_path)])?;
                    let replacement = snapshot.groups.iter().flat_map(|g| &g.instances).find(|i| i.package_id.eq_ignore_ascii_case(&item.package_id) && !selected.contains(&files::path_key(&i.file_path)) && Path::new(&i.file_path).exists());
                    if let Some(keep) = replacement {
                        files::index_package(&tx, Path::new(&keep.file_path))?;
                    tx.execute("DELETE FROM packages WHERE lower(replace(file_path, '\\', '/')) = ?1", [files::path_key(&item.file_path)])?;
                    } else {
                        tx.execute("DELETE FROM packages WHERE lower(replace(file_path, '\\', '/')) = ?1", [files::path_key(&item.file_path)])?;
                    }
                    tx.commit()?;
                    Ok(())
                })();
                if let Err(e) = update {
                    let rollback = files::transfer(&destination, Path::new(&item.file_path), "move");
                    return Err(AppError::Io(format!("索引更新失败: {}; 文件恢复: {:?}", e, rollback)));
                }
                Ok(())
            });
        match action {
            Ok(()) => result.cleaned += 1,
            Err(e) => {
                result.errors.push(format!("{}: {}", item.file_path, e));
                break;
            }
        }
    }
    if result.errors.is_empty() {
        for (item, destination) in archives {
            let action = db.with_conn(|conn| {
                    let tx = conn.unchecked_transaction()?;
                    // Reuse the migration log so the existing restore action can undo the archive.
                    tx.execute("INSERT INTO resource_migration_log (task_id,package_id,source_path,destination_path,action,size_bytes,status,completed_at,file_modified_time) VALUES (?1,?2,?3,?4,'move',?5,'completed',datetime('now'),?6)",
                        rusqlite::params![scan_id,item.package_id,item.file_path,destination,item.size_bytes,files::fingerprint(Path::new(&item.file_path)).map_err(AppError::Io)?.modified])?;
                    files::transfer(Path::new(&item.file_path),Path::new(&destination),"move").map_err(AppError::Io)?;
                    let update = (|| -> Result<(), AppError> {
                        files::update_index_path(&tx,&item.file_path,&destination)?;
                        files::index_package(&tx,Path::new(&destination))?;
                        tx.commit()?; Ok(())
                    })();
                    if let Err(e) = update {
                        let rollback = files::transfer(Path::new(&destination),Path::new(&item.file_path),"move");
                        return Err(AppError::Io(format!("归档索引失败: {}; 文件恢复: {:?}",e,rollback)));
                    }
                    Ok(())
                });
            match action {
                Ok(()) => result.archived += 1,
                Err(e) => {
                    result.errors.push(e.to_string());
                    break;
                }
            }
        }
    }
    // Newly found survivors may never have been indexed before this disk scan.
    for group in &snapshot.groups {
        if !group.instances.iter().any(|i| {
            selected.contains(&files::path_key(&i.file_path)) || i.archive_destination.is_some()
        }) {
            continue;
        }
        for item in group.instances.iter().filter(|i| i.is_recommended_keep) {
            let path = item
                .archive_destination
                .as_deref()
                .filter(|p| Path::new(p).exists())
                .unwrap_or(&item.file_path);
            if let Ok(pkg) = crate::services::var_parser::parse_var_file(Path::new(path)) {
                if let Err(e) = db.with_conn(|conn| {
                    let tx = conn.unchecked_transaction()?;
                    files::index_parsed_package(&tx, &pkg)?;
                    tx.commit()?;
                    Ok(())
                }) {
                    result
                        .errors
                        .push(format!("文件已整理，索引更新失败，请重新扫描资源库: {}", e));
                }
            }
        }
    }
    match files::remove_empty_dirs_excluding(Path::new(&snapshot.summary.root), &snapshot.excluded)
    {
        Ok((count, errors)) => {
            result.removed_directories = count;
            result.errors.extend(errors);
        }
        Err(e) => result.errors.push(e),
    }
    Ok(result)
}

pub fn managed_link_paths(app_handle: &tauri::AppHandle) -> HashSet<String> {
    load_manifest_link_types(app_handle).into_keys().collect()
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
        .and_then(|v| v.as_array())
        .map(|links| {
            links
                .iter()
                .filter_map(|v| {
                    Some((
                        files::path_key(v.get("link")?.as_str()?),
                        v.get("link_type")?.as_str()?.to_string(),
                    ))
                })
                .collect()
        })
        .unwrap_or_default()
}

pub fn purge_expired_trash_internal(conn: &rusqlite::Connection) -> Result<usize, AppError> {
    let mut stmt = conn.prepare(
        "SELECT id, trash_path FROM cleanup_trash
         WHERE restored_at IS NULL AND created_at < datetime('now', '-1 day')",
    )?;
    let expired_items = stmt
        .query_map([], |row| {
            Ok((row.get::<_, i64>(0)?, row.get::<_, String>(1)?))
        })?
        .collect::<Result<Vec<_>, _>>()?;

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
    let _ = db.with_conn(|conn| purge_expired_trash_internal(conn));

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
        let trash_path: String = conn
            .query_row(
                "SELECT trash_path FROM cleanup_trash WHERE id = ?1",
                [trash_id],
                |row| row.get(0),
            )
            .map_err(|e| AppError::Database(format!("未找到回收站项: {}", e)))?;

        let path = Path::new(&trash_path);
        if path.exists() {
            std::fs::remove_file(path)
                .map_err(|e| AppError::Io(format!("永久删除物理文件失败: {}", e)))?;
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
        let mut stmt = conn
            .prepare("SELECT id, trash_path FROM cleanup_trash WHERE restored_at IS NULL")
            .map_err(|e| AppError::Database(e.to_string()))?;

        let items = stmt
            .query_map([], |row| {
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
    let _guard = files::maintenance_lock()?;
    let package_id = restore_trash_item(&db, trash_id)?;

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

fn restore_trash_item(db: &Database, trash_id: i64) -> Result<String, String> {
    db.with_conn(|conn| {
        let tx = conn.unchecked_transaction()?;
        let (package_id, original, trash, size): (String,String,String,u64) = tx.query_row(
            "SELECT package_id,original_path,trash_path,size_bytes FROM cleanup_trash WHERE id=?1 AND restored_at IS NULL",[trash_id],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?,r.get(3)?)))?;
        let snapshot=files::fingerprint(Path::new(&trash)).map_err(AppError::Io)?;
        if snapshot.size!=size {return Err(AppError::Validation("回收站文件已变化".into()));}
        files::transfer(Path::new(&trash),Path::new(&original),"move").map_err(AppError::Io)?;
        let update=(||->Result<(),AppError>{
            if let Ok(pkg)=crate::services::var_parser::parse_var_file(Path::new(&original)) {
                files::index_parsed_package(&tx,&pkg)?;
            }
            tx.execute("UPDATE cleanup_trash SET restored_at=datetime('now') WHERE id=?1",[trash_id])?;
            tx.commit()?;Ok(())
        })();
        if let Err(e)=update {
            let rollback=files::transfer(Path::new(&original),Path::new(&trash),"move");
            return Err(AppError::Io(format!("恢复索引失败: {}; 文件恢复: {:?}",e,rollback)));
        }
        Ok(package_id)
    }).map_err(|e|e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use files::tests::TestDir;
    use std::io::Write;
    fn var(d: &TestDir, name: &str, deps: serde_json::Value) {
        let path = d.0.join(name);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        let mut zip = zip::ZipWriter::new(std::fs::File::create(path).unwrap());
        zip.start_file("meta.json", zip::write::SimpleFileOptions::default())
            .unwrap();
        zip.write_all(
            serde_json::json!({"dependencies":deps})
                .to_string()
                .as_bytes(),
        )
        .unwrap();
        zip.finish().unwrap();
    }
    #[test]
    fn recycle_unindexed_files_archive_pinned_and_restore() {
        let d = TestDir::new();
        let data = TestDir::new();
        let db = Database::new(&data.0.join("db.sqlite")).unwrap();
        var(&d, "old/A.Asset.1.var", serde_json::json!({}));
        var(&d, "old/A.Asset.2.var", serde_json::json!({}));
        var(&d, "A.Asset.3.var", serde_json::json!({}));
        var(&d, "B.Scene.1.var", serde_json::json!({"A.Asset.1":{}}));
        d.write("a/readme.txt", b"same");
        d.write("b/readme.txt", b"same");
        let snapshot = resource_dedup::scan(&d.0, HashSet::new()).unwrap();
        let selected: Vec<_> = snapshot
            .groups
            .iter()
            .flat_map(|g| &g.instances)
            .filter(|i| !i.is_recommended_keep)
            .map(|i| i.file_path.clone())
            .collect();
        let result = organize_snapshot(&db, &snapshot, &data.0.join("trash"), &selected).unwrap();
        assert!(result.errors.is_empty(), "{:?}", result.errors);
        assert_eq!(result.cleaned, 2);
        assert_eq!(result.archived, 1);
        assert!(d
            .0
            .join(files::REFERENCED_DIR)
            .join("A.Asset.1.var")
            .exists());
        assert!(d.0.join("A.Asset.3.var").exists());
        let ids = db
            .with_conn(|conn| {
                let mut stmt = conn.prepare("SELECT id FROM cleanup_trash")?;
                let result = stmt
                    .query_map([], |r| r.get::<_, i64>(0))?
                    .collect::<Result<Vec<_>, _>>()?;
                Ok(result)
            })
            .unwrap();
        for id in ids {
            restore_trash_item(&db, id).unwrap();
        }
        assert!(d.0.join("old/A.Asset.2.var").exists());
        assert!(d.0.join("a/readme.txt").exists());
        assert!(d.0.join("b/readme.txt").exists());
        let count = db
            .with_conn(|c| {
                Ok(c.query_row(
                    "SELECT COUNT(*) FROM packages WHERE id='A.Asset.2'",
                    [],
                    |r| r.get::<_, i64>(0),
                )?)
            })
            .unwrap();
        assert_eq!(count, 1);
    }
    #[test]
    fn failed_database_write_leaves_originals_untouched() {
        let d = TestDir::new();
        let data = TestDir::new();
        let db = Database::new(&data.0.join("db.sqlite")).unwrap();
        d.write("a/readme.txt", b"same");
        d.write("b/readme.txt", b"same");
        let snapshot = resource_dedup::scan(&d.0, HashSet::new()).unwrap();
        let selected: Vec<_> = snapshot.groups[0]
            .instances
            .iter()
            .filter(|i| !i.is_recommended_keep)
            .map(|i| i.file_path.clone())
            .collect();
        db.with_conn(|c|{c.execute_batch("CREATE TRIGGER fail_trash BEFORE INSERT ON cleanup_trash BEGIN SELECT RAISE(ABORT,'test failure'); END;")?;Ok(())}).unwrap();
        let result = organize_snapshot(&db, &snapshot, &data.0.join("trash"), &selected).unwrap();
        assert_eq!(result.cleaned, 0);
        assert_eq!(result.errors.len(), 1);
        assert!(selected.iter().all(|p| Path::new(p).exists()));
    }
}
