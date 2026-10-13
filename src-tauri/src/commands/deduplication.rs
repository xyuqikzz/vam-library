use crate::db::Database;
use crate::errors::AppError;
use crate::services::install_context::resolve_install_context;
use crate::services::resource_dedup::{self, DedupSnapshot, DedupSummary, DuplicateGroup};
use crate::services::resource_files::{self as files};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::{Mutex, Arc, atomic::{AtomicBool, Ordering}};
use tauri::Emitter;
use crate::services::{dedup_analysis, resource_cleanup};
use tauri::{Manager, State};

#[derive(Default)]
pub struct DedupState(pub Mutex<Option<DedupSnapshot>>, pub Mutex<Option<(String, Arc<AtomicBool>)>>);

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
pub async fn scan_for_duplicates(app_handle: tauri::AppHandle, vam_root: String,
    request_id: String, mode: Option<dedup_analysis::Mode>) -> Result<DedupSummary, String> {
    let cancel = Arc::new(AtomicBool::new(false));
    {
        let state = app_handle.state::<DedupState>();
        let mut running = state.1.lock().map_err(|e|e.to_string())?;
        if running.is_some() { return Err("去重分析正在进行，请先停止当前分析".into()); }
        *running = Some((request_id.clone(),cancel.clone()));
        *state.0.lock().map_err(|e|e.to_string())? = None;
    }
    let app = app_handle.clone(); let request = request_id.clone();
    let result = tauri::async_runtime::spawn_blocking(move || {
        let _guard = files::maintenance_lock_cancellable(&cancel)?;
        let started=std::time::Instant::now();
        let mut last=std::time::Instant::now()-std::time::Duration::from_secs(1);
        let mut last_stage=String::new();
        let mut progress=|stage:&str,current:usize,total:usize,bytes:u64| {
            if stage!=last_stage || last.elapsed().as_millis()>=100 || (total>0 && current==total) {
                let _=app.emit("dedup-progress",serde_json::json!({"request_id":request,"stage":stage,"current":current,"total":total,"bytes":bytes}));
                last=std::time::Instant::now();last_stage=stage.into();
            }
        };
        let mut root=PathBuf::from(vam_root);
        if root.join("AddonPackages").is_dir() {root=root.join("AddonPackages");}
        let ctx=resource_cleanup::Context::from_install(&resolve_install_context(&app)?)?;
        let db=app.state::<Database>();
        progress("references",0,0,0);
        let catalog=resource_cleanup::build_catalog(&db,&ctx,false,&cancel,&mut |_,n,t|progress("references",n,t,0))?;
        let protection=resource_cleanup::dedup_protection(&catalog);
        let mut snapshot=dedup_analysis::analyze(&root,load_manifest_link_types(&app).into_keys().collect(),mode.unwrap_or_default(),Some(&db),&protection,&cancel,&mut progress)?;
        snapshot.summary.elapsed_ms=started.elapsed().as_millis() as u64;
        crate::services::dedup_cache::check(&cancel)?;
        let summary=snapshot.summary.clone();
        let state=app.state::<DedupState>();
        // Pair publication with the cancellation lock so a late stop cannot publish a plan.
        let _running=state.1.lock().map_err(|e|e.to_string())?;
        crate::services::dedup_cache::check(&cancel)?;
        *state.0.lock().map_err(|e|e.to_string())?=Some(snapshot);
        Ok(summary)
    }).await.map_err(|e|e.to_string()).and_then(|r|r);
    let state=app_handle.state::<DedupState>();
    *state.1.lock().map_err(|e|e.to_string())?=None;
    result
}

#[tauri::command]
pub fn cancel_dedup_scan(state: State<'_,DedupState>, request_id: String) -> Result<(),String> {
    let running=state.1.lock().map_err(|e|e.to_string())?;
    if let Some((id,cancel))=running.as_ref().filter(|(id,_)|id==&request_id) {
        let _=id; cancel.store(true,Ordering::Relaxed);
        *state.0.lock().map_err(|e|e.to_string())?=None;
    }
    Ok(())
}

#[tauri::command]
pub async fn get_duplicate_groups(
    state: State<'_, DedupState>,
    scan_id: Option<String>,
) -> Result<Vec<DuplicateGroup>, String> {
    Ok(state
        .0
        .lock()
        .map_err(|e| e.to_string())?
        .as_ref()
        .filter(|s| scan_id.as_ref().map_or(true, |id| id == &s.summary.scan_id))
        .map(|s| s.groups.clone())
        .unwrap_or_default())
}

#[derive(Clone, Serialize)]
struct CleanupProgress {
    scan_id: String,
    stage: String,
    current: usize,
    total: usize,
    file_name: String,
}

#[tauri::command]
pub async fn execute_cleanup(
    app_handle: tauri::AppHandle,
    scan_id: String,
    file_paths: Vec<String>,
) -> Result<CleanupResult, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let mut last = std::time::Instant::now();
        let mut last_stage = String::new();
        let mut last_count = usize::MAX;
        let mut progress = |stage: &str, current: usize, total: usize, path: &str| {
            if stage != last_stage || current != last_count || last.elapsed().as_millis() >= 100 {
                let _ = app_handle.emit("dedup-cleanup-progress", CleanupProgress {
                    scan_id: scan_id.clone(), stage: stage.into(), current, total,
                    file_name: Path::new(path).file_name().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default(),
                });
                last = std::time::Instant::now(); last_stage = stage.into(); last_count = current;
            }
        };
        progress("waiting", 0, 0, "");
        let _execution = crate::commands::cleanup::execution_guard(&app_handle)?;
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
        crate::services::scene_launch::ensure_game_stopped(Path::new(&context.vam_root))?;
        let cleanup_context = crate::services::resource_cleanup::Context::from_install(&context)?;
        if !file_paths.iter().all(|path| cleanup_context.roots.iter().any(|root| Path::new(path).starts_with(root))) {return Err("清理文件不属于当前游戏实例".into());}
        let pins = resource_dedup::pin_selection(&snapshot, &file_paths)?;
        let deletions = resource_dedup::validate_pinned_selection_with_progress(&snapshot, &file_paths, &pins, &mut progress)?;
        progress("references", 0, 0, "");
        resource_cleanup::validate_dedup_deletions(&app_handle.state::<Database>(), &cleanup_context, &file_paths, &snapshot.witnesses)?;
        let trash = PathBuf::from(context.vam_root)
            .join("VAMBoxLibrary")
            .join(".trash");
        let result = organize_verified_with_progress(
            &app_handle.state::<Database>(), &snapshot, &trash, &file_paths, deletions, &mut progress,
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

#[cfg(test)]
fn organize_snapshot(
    db: &Database,
    snapshot: &DedupSnapshot,
    trash: &Path,
    file_paths: &[String],
) -> Result<CleanupResult, String> {
    let pins = resource_dedup::pin_selection(snapshot, file_paths)?;
    let deletions = resource_dedup::validate_pinned_selection(snapshot, file_paths, &pins)?;
    organize_verified(db, snapshot, trash, file_paths, deletions)
}

#[cfg(test)]
fn organize_verified(db: &Database, snapshot: &DedupSnapshot, trash: &Path, file_paths: &[String],
    deletions: Vec<crate::services::resource_dedup::DuplicateInstance>) -> Result<CleanupResult,String> {
    organize_verified_with_progress(db, snapshot, trash, file_paths, deletions, &mut |_, _, _, _| {})
}

fn organize_verified_with_progress(db: &Database, snapshot: &DedupSnapshot, trash: &Path, file_paths: &[String],
    deletions: Vec<crate::services::resource_dedup::DuplicateInstance>,
    progress: &mut impl FnMut(&str, usize, usize, &str)) -> Result<CleanupResult,String> {
    let scan_id = &snapshot.summary.scan_id;
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
    files::ensure_no_links(trash)?;
    let mut result = CleanupResult {
        cleaned: 0,
        archived: 0,
        removed_directories: 0,
        errors: vec![],
    };
    let files_by_path: HashMap<_, _> = snapshot
        .files
        .iter()
        .map(|file| (file.path.as_str(), file))
        .collect();
    let groups_by_path: HashMap<_, _> = snapshot
        .groups
        .iter()
        .flat_map(|group| {
            group
                .instances
                .iter()
                .map(move |item| (item.file_path.as_str(), group))
        })
        .collect();
    let mut replacements = HashMap::<String, Vec<_>>::new();
    for item in snapshot.groups.iter().flat_map(|group| &group.instances) {
        replacements
            .entry(item.package_id.to_ascii_lowercase())
            .or_default()
            .push(item);
    }
    let total = deletions.len() + archives.len();
    for (index, item) in deletions.iter().enumerate() {
        progress("moving", result.cleaned + result.archived, total, &item.file_path);
        if let Some(file) = files_by_path.get(item.file_path.as_str()) {
            if let Err(e) = files::verify(file) {
                result.errors.push(e);
                break;
            }
        }
        let group = groups_by_path
            .get(item.file_path.as_str())
            .ok_or("清理分组已失效")?;
        if let Some(error) = group
            .instances
            .iter()
            .filter(|i| i.is_recommended_keep)
            .filter_map(|i| files_by_path.get(i.file_path.as_str()))
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
                    let replacement = replacements.get(&item.package_id.to_ascii_lowercase()).into_iter().flatten().find(|i| {
                        !selected.contains(&files::path_key(&i.file_path)) && Path::new(&i.file_path).exists()
                            && (snapshot.hashes.get(&item.file_path).map_or(true, |hash| snapshot.hashes.get(&i.file_path) == Some(hash))
                                || snapshot.content_hashes.get(&item.file_path).is_some_and(|hash| snapshot.content_hashes.get(&i.file_path) == Some(hash)))
                    });
                    if let Some(keep) = replacement {
                        let exact_copy = snapshot.hashes.get(&item.file_path)
                            .zip(snapshot.hashes.get(&keep.file_path))
                            .is_some_and(|(removed, retained)| removed == retained)
                            || snapshot.content_hashes.get(&item.file_path).zip(snapshot.content_hashes.get(&keep.file_path))
                                .is_some_and(|(removed, retained)| removed == retained);
                        if exact_copy {
                            // The identical survivor can have unreadable metadata. Keep
                            // existing logical metadata and redirect only the removed path.
                            tx.execute("UPDATE packages SET file_path=?1, updated_at=datetime('now') WHERE lower(replace(file_path, '\\', '/'))=?2",
                                rusqlite::params![keep.file_path, files::path_key(&item.file_path)])?;
                        } else {
                            files::index_package(&tx, Path::new(&keep.file_path))?;
                        }
                    }
                    tx.execute("DELETE FROM packages WHERE lower(replace(file_path, '\\', '/')) = ?1", [files::path_key(&item.file_path)])?;
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
            progress("moving", result.cleaned + result.archived, total, &item.file_path);
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
    progress("finishing", 0, 0, "");
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
    if result.errors.is_empty() { progress("complete", result.cleaned + result.archived, total, ""); }
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

pub fn purge_expired_trash_internal(_conn: &rusqlite::Connection) -> Result<usize, AppError> {
    // Legacy entries have neither an instance binding nor a content fingerprint.
    // They remain available for explicit restore/deletion, never automatic purge.
    Ok(0)
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
    app_handle: tauri::AppHandle,
    db: State<'_, Database>,
    trash_id: i64,
) -> Result<(), String> {
    let db = db.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        let _execution = crate::commands::cleanup::execution_guard(&app_handle)?;
        purge_trash_items(&db, Some(trash_id)).map(|_| ())
    }).await.map_err(|e| e.to_string())?
}

#[tauri::command]
pub async fn empty_cleanup_trash(
    app_handle: tauri::AppHandle,
    db: State<'_, Database>,
) -> Result<usize, String> {
    let db = db.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        let _execution = crate::commands::cleanup::execution_guard(&app_handle)?;
        purge_trash_items(&db, None)
    }).await.map_err(|e| e.to_string())?
}

fn purge_trash_items(db: &Database, selected: Option<i64>) -> Result<usize, String> {
    let _guard = files::maintenance_lock()?;
    let items: Vec<(i64, String)> = db.with_conn(|conn| {
        let mut stmt = conn.prepare("SELECT id,trash_path FROM cleanup_trash WHERE restored_at IS NULL AND (?1 IS NULL OR id=?1)")?;
        let items = stmt.query_map([selected], |r| Ok((r.get(0)?,r.get(1)?)))?.collect::<rusqlite::Result<_>>()?;
        Ok(items)
    }).map_err(|e| e.to_string())?;
    if selected.is_some() && items.is_empty() { return Err("未找到回收站项，可能已恢复或删除".into()); }
    let mut deleted = 0;
    let mut errors = Vec::new();
    for chunk in items.chunks(128) {
        let mut removed = Vec::new();
        // Filesystem work must not monopolize the shared SQLite connection.
        for (id, trash) in chunk {
            let path = Path::new(trash);
            let result = (|| {
                if !path.is_absolute() || path.components().any(|c| matches!(c, std::path::Component::ParentDir))
                    || !path.components().any(|c| c.as_os_str().to_string_lossy().eq_ignore_ascii_case(".trash")) {
                    return Err("无效回收文件路径".to_string());
                }
                files::ensure_no_links(path)?;
                match std::fs::remove_file(path) {
                    Ok(()) => Ok(()),
                    Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
                    Err(e) => Err(e.to_string()),
                }
            })();
            match result {
                Ok(()) => removed.push((*id, trash)),
                Err(e) => errors.push(format!("{trash}: {e}")),
            }
        }
        db.with_conn(|conn| {
            let tx = conn.unchecked_transaction()?;
            {
                let mut stmt = tx.prepare_cached("DELETE FROM cleanup_trash WHERE id=?1 AND trash_path=?2 AND restored_at IS NULL")?;
                for (id, path) in &removed { deleted += stmt.execute(rusqlite::params![id,path])?; }
            }
            tx.commit()?; Ok(())
        }).map_err(|e| e.to_string())?;
    }
    if !errors.is_empty() {
        return Err(format!("已删除 {deleted} 个文件，另有 {} 个没能删除，记录已保留：{}", errors.len(), errors.into_iter().take(10).collect::<Vec<_>>().join("; ")));
    }
    Ok(deleted)
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
    fn trash_record(db: &Database, path: &Path, restored: bool) -> i64 {
        db.with_conn(|c| {
            c.execute("INSERT INTO cleanup_trash(package_id,original_path,trash_path,size_bytes,restored_at) VALUES('A.Test.1','original',?1,4,?2)",
                rusqlite::params![path.to_string_lossy(),restored.then_some("restored")])?;
            Ok(c.last_insert_rowid())
        }).unwrap()
    }
    #[test]
    fn empty_legacy_trash_keeps_failed_and_restored_records() {
        let d = TestDir::new();
        let db = Database::new(&d.0.join("index.db")).unwrap();
        let good = d.write(".trash/good.var", b"test");
        trash_record(&db, &good, false);
        trash_record(&db, &d.0.join(".trash/missing.var"), false);
        let blocked = d.0.join(".trash/directory.var");
        std::fs::create_dir_all(&blocked).unwrap();
        trash_record(&db, &blocked, false);
        let outside = d.write("outside.var", b"keep");
        trash_record(&db, &outside, false);
        let restored = d.write(".trash/restored.var", b"keep");
        let restored_id = trash_record(&db, &restored, true);
        assert!(purge_trash_items(&db, Some(restored_id)).is_err());
        let error = purge_trash_items(&db, None).unwrap_err();
        assert!(error.contains("已删除 2 个文件"));
        assert!(!good.exists());
        assert!(outside.exists() && restored.exists() && blocked.exists());
        db.with_conn(|c| { assert_eq!(c.query_row("SELECT COUNT(*) FROM cleanup_trash",[],|r|r.get::<_,i64>(0))?,3); Ok(()) }).unwrap();
    }
    #[test]
    fn legacy_trash_deletion_crosses_transaction_chunks_and_supports_single_selection() {
        let d = TestDir::new();
        let db = Database::new(&d.0.join("index.db")).unwrap();
        let mut ids = Vec::new();
        for i in 0..260 { ids.push(trash_record(&db, &d.write(&format!(".trash/{i}.var"),b"test"), false)); }
        assert_eq!(purge_trash_items(&db, Some(ids[100])).unwrap(),1);
        assert!(d.0.join(".trash/99.var").exists());
        assert_eq!(purge_trash_items(&db, None).unwrap(),259);
        assert_eq!(purge_trash_items(&db, None).unwrap(),0);
    }
    #[test]
    fn normalized_deletion_paths_use_indexes() {
        let d = TestDir::new();
        let db = Database::new(&d.0.join("index.db")).unwrap();
        db.with_conn(|c| {
            for (table,index) in [("physical_packages","idx_phys_normalized_path"),("packages","idx_packages_normalized_path")] {
                let sql = format!("EXPLAIN QUERY PLAN DELETE FROM {table} WHERE lower(replace(file_path, '\\', '/')) = ?1");
                let mut stmt=c.prepare(&sql)?;
                let details=stmt.query_map(["/fixture/a.var"],|r|r.get::<_,String>(3))?.collect::<rusqlite::Result<Vec<_>>>()?;
                assert!(details.iter().any(|d|d.contains(index)),"{details:?}");
            }
            Ok(())
        }).unwrap();
    }
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
    fn new_analysis_recycles_and_restores_cross_name_and_repacked_copies() {
        use crate::services::{dedup_analysis::{self,Mode,Protection},resource_cleanup::{self,Context}};
        use std::io::Write;
        for deep in [false,true] {
            let d=TestDir::new();let data=TestDir::new();let db=Database::new(&data.0.join("db.sqlite")).unwrap();
            let root=d.0.join("AddonPackages");std::fs::create_dir(&root).unwrap();
            let first=root.join("A.Asset.1.var");
            let second=if deep {root.join("sub/A.Asset.1.var")} else {root.join("A.Asset.1_2.var")};
            for (i,path) in [&first,&second].iter().enumerate() {
                std::fs::create_dir_all(path.parent().unwrap()).unwrap();
                let mut zip=zip::ZipWriter::new(std::fs::File::create(path).unwrap());
                let options=zip::write::SimpleFileOptions::default().compression_method(if deep && i==1 {zip::CompressionMethod::Deflated} else {zip::CompressionMethod::Stored});
                zip.start_file("meta.json",options).unwrap();zip.write_all(b"{}").unwrap();
                zip.start_file("Custom/test.txt",options).unwrap();zip.write_all(&vec![b'x';1024]).unwrap();zip.finish().unwrap();
                db.with_conn(|c| files::index_package(c,path)).unwrap();
            }
            let ctx=Context{library_key:"test".into(),vam_root:d.0.to_string_lossy().into_owned(),roots:vec![root.to_string_lossy().into_owned()],managed:false,shared:false};
            let snapshot=dedup_analysis::analyze(&root,HashSet::new(),if deep {Mode::Deep} else {Mode::Standard},Some(&db),&Protection{complete:true,..Default::default()},&AtomicBool::new(false),&mut |_,_,_,_|{}).unwrap();
            let selected:Vec<_>=snapshot.groups.iter().flat_map(|g|&g.instances).filter(|i|!i.is_recommended_keep).map(|i|i.file_path.clone()).collect();
            assert_eq!(selected.len(),1);
            let kept=snapshot.witnesses[&selected[0]].clone();let removed_bytes=std::fs::read(&selected[0]).unwrap();
            {
                let pins=resource_dedup::pin_selection(&snapshot,&selected).unwrap();
                let deletions=resource_dedup::validate_pinned_selection(&snapshot,&selected,&pins).unwrap();
                resource_cleanup::validate_dedup_deletions(&db,&ctx,&selected,&snapshot.witnesses).unwrap();
                let mut events = Vec::new();
                let result=organize_verified_with_progress(&db,&snapshot,&data.0.join("trash"),&selected,deletions,
                    &mut |stage, current, total, path| events.push((stage.to_string(), current, total, path.to_string()))).unwrap();
                assert_eq!(events.first().map(|e| (&*e.0,e.1,e.2)), Some(("moving",0,1)));
                assert_eq!(events.last().map(|e| (&*e.0,e.1,e.2)), Some(("complete",1,1)));
                assert_eq!(events[0].3, selected[0]);
                assert!(result.errors.is_empty(),"{:?}",result.errors);assert_eq!(result.cleaned,1);
            }
            assert!(Path::new(&kept).exists());assert!(!Path::new(&selected[0]).exists());
            let trash_id=db.with_conn(|c|Ok(c.query_row("SELECT id FROM cleanup_trash",[],|r|r.get::<_,i64>(0))?)).unwrap();
            restore_trash_item(&db,trash_id).unwrap();
            assert_eq!(std::fs::read(&selected[0]).unwrap(),removed_bytes);
        }
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

    fn check_malformed_duplicate_cleanup(d: &TestDir, indexed: bool) {
        let data = TestDir::new();
        let db = Database::new(&data.0.join("db.sqlite")).unwrap();
        var(
            d,
            "B.Scene.1.var",
            serde_json::json!({"Amaimon.Bhaviour.2":{}}),
        );
        let snapshot = resource_dedup::scan(&d.0, HashSet::new()).unwrap();
        let group = snapshot
            .groups
            .iter()
            .find(|g| g.resource_path == "amaimon.bhaviour")
            .unwrap();
        let kept = group
            .instances
            .iter()
            .find(|i| i.is_recommended_keep)
            .unwrap();
        let recycled = group
            .instances
            .iter()
            .find(|i| !i.is_recommended_keep)
            .unwrap();
        assert_eq!(
            group
                .instances
                .iter()
                .filter(|i| i.is_recommended_keep)
                .count(),
            1
        );
        if indexed {
            db.with_conn(|conn| {
                    conn.execute("INSERT INTO packages (id,creator,name,version,file_path,scan_time,description) VALUES ('Amaimon.Bhaviour.2','Amaimon','Bhaviour',2,?1,'preserved time','preserved metadata')", [&recycled.file_path])?;
                    for item in &group.instances {
                        conn.execute("INSERT INTO physical_packages (file_path,package_id,scan_time,scan_status) VALUES (?1,?2,'preserved time','error')", rusqlite::params![item.file_path,item.package_id])?;
                    }
                    Ok(())
                }).unwrap();
        }
        let context = crate::services::resource_cleanup::Context {
            library_key: files::path_key(&d.0.to_string_lossy()),
            vam_root: d.0.to_string_lossy().into_owned(),
            roots: vec![d.0.to_string_lossy().into_owned()],
            managed: false,
            shared: false,
        };
        let selected = vec![recycled.file_path.clone()];
        crate::services::resource_cleanup::validate_deletions(&db, &context, &selected).unwrap();
        let result = organize_snapshot(&db, &snapshot, &data.0.join("trash"), &selected).unwrap();
        assert!(result.errors.is_empty(), "{:?}", result.errors);
        assert_eq!(result.cleaned, 1);
        assert_eq!(result.archived, 0);
        assert!(Path::new(&kept.file_path).is_file());
        assert!(!Path::new(&recycled.file_path).exists());
        if indexed {
            let row: (String, String, String) = db.with_conn(|c| Ok(c.query_row("SELECT file_path,scan_time,description FROM packages WHERE id='Amaimon.Bhaviour.2'", [], |r| Ok((r.get(0)?,r.get(1)?,r.get(2)?)))?)).unwrap();
            assert_eq!(
                row,
                (
                    kept.file_path.clone(),
                    "preserved time".into(),
                    "preserved metadata".into()
                )
            );
            let physical: Vec<String> = db
                .with_conn(|c| {
                    let mut stmt = c.prepare(
                        "SELECT file_path FROM physical_packages WHERE package_id='Amaimon.Bhaviour.2'",
                    )?;
                    let rows = stmt
                        .query_map([], |r| r.get(0))?
                        .collect::<Result<Vec<_>, _>>()?;
                    Ok(rows)
                })
                .unwrap();
            assert_eq!(physical, [kept.file_path.clone()]);
        }
        let trash_id = db
            .with_conn(|c| {
                Ok(c.query_row(
                    "SELECT id FROM cleanup_trash WHERE restored_at IS NULL",
                    [],
                    |r| r.get::<_, i64>(0),
                )?)
            })
            .unwrap();
        restore_trash_item(&db, trash_id).unwrap();
        assert!(Path::new(&recycled.file_path).is_file());
        assert_eq!(
            files::hash_file(&files::fingerprint(Path::new(&recycled.file_path)).unwrap()).unwrap(),
            snapshot.hashes[&kept.file_path]
        );
    }

    #[test]
    fn malformed_duplicate_copies_recycle_and_restore_without_reparsing() {
        for indexed in [false, true] {
            let d = TestDir::new();
            let source = d.0.join("场景/Amaimon.Bhaviour.2.var");
            std::fs::create_dir_all(source.parent().unwrap()).unwrap();
            let mut zip = zip::ZipWriter::new(std::fs::File::create(&source).unwrap());
            zip.start_file("meta.json", zip::write::SimpleFileOptions::default())
                .unwrap();
            zip.write_all(br#"{"description":"bad "quote","dependencies":{}}"#)
                .unwrap();
            zip.finish()
                .unwrap()
                .set_times(std::fs::FileTimes::new().set_modified(
                    std::time::SystemTime::UNIX_EPOCH + std::time::Duration::from_secs(200),
                ))
                .unwrap();
            let copy = d.write(
                "其他/Amaimon.Bhaviour.2.var",
                &std::fs::read(&source).unwrap(),
            );
            std::fs::OpenOptions::new()
                .write(true)
                .open(copy)
                .unwrap()
                .set_times(std::fs::FileTimes::new().set_modified(
                    std::time::SystemTime::UNIX_EPOCH + std::time::Duration::from_secs(100),
                ))
                .unwrap();
            check_malformed_duplicate_cleanup(&d, indexed);
        }
    }

    #[test]
    #[ignore = "requires VAM_DEDUP_FIXTURE_ROOT with the reported Amaimon copies; source files are read only"]
    fn reported_malformed_package_copies_recycle_and_restore() {
        let source_root = PathBuf::from(std::env::var("VAM_DEDUP_FIXTURE_ROOT").unwrap());
        let d = TestDir::new();
        for folder in ["场景", "其他"] {
            let relative = format!("{folder}/Amaimon.Bhaviour.2.var");
            let source = source_root.join(&relative);
            let modified = std::fs::metadata(&source).unwrap().modified().unwrap();
            let copy = d.write(&relative, &std::fs::read(source).unwrap());
            std::fs::OpenOptions::new()
                .write(true)
                .open(copy)
                .unwrap()
                .set_times(std::fs::FileTimes::new().set_modified(modified))
                .unwrap();
        }
        check_malformed_duplicate_cleanup(&d, true);
    }
}
