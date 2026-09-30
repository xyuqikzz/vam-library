use crate::db::Database;
use crate::errors::AppError;
use crate::models::resource::{normalize_resource_types, primary_resource_type};
use crate::services::resource_files::{self as files, DiskFile, REFERENCED_DIR};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use tauri::{Emitter, Manager};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum MigrationMode {
    ByType,
    ByCreator,
    ByScene,
    Custom,
    Flatten,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MigrationConfig {
    pub mode: MigrationMode,
    pub source_dir: String,
    pub target_dir: String,
    // Kept for IPC compatibility: the old UI called copy mode `dry_run`.
    pub dry_run: bool,
    pub locale: Option<String>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MigrationOperation {
    pub package_id: String,
    pub source: String,
    pub destination: String,
    pub action: String,
    pub size_bytes: u64,
    pub resource_type: String,
    pub conflict: String,
    pub modified_time: String,
}
#[derive(Debug, Clone, Serialize)]
pub struct MigrationPreview {
    pub plan_id: String,
    pub source_dir: String,
    pub target_dir: String,
    pub total_files: usize,
    pub total_operations: usize,
    pub total_size_bytes: u64,
    pub operations: Vec<MigrationOperation>,
    pub conflicts: Vec<String>,
    pub skipped: usize,
    pub warnings: Vec<String>,
    pub cleanup_empty_dirs: bool,
}
#[derive(Debug, Clone)]
struct MigrationPlan {
    validate_directory: bool,
    preview: MigrationPreview,
    files: Vec<DiskFile>,
    excluded: HashSet<String>,
}
#[derive(Default)]
pub struct MigrationState(Mutex<Option<MigrationPlan>>);
#[derive(Debug, Clone, Serialize)]
pub struct MigrationResult {
    pub task_id: String,
    pub completed: usize,
    pub failed: usize,
    pub total: usize,
    pub errors: Vec<String>,
    pub rollback_available: bool,
    pub removed_directories: usize,
}
#[derive(Debug, Clone, Serialize)]
pub struct MigrationRollbackResult {
    pub task_id: String,
    pub restored: usize,
    pub failed: usize,
    pub errors: Vec<String>,
}
#[derive(Debug, Clone, Serialize)]
pub struct MigrationProgress {
    pub task_id: String,
    pub phase: String,
    pub completed: usize,
    pub failed: usize,
    pub total: usize,
    pub current_package_id: Option<String>,
}

pub(super) fn localized_type_folder(resource_type: &str, locale: Option<&str>) -> String {
    let zh = locale.unwrap_or_default().starts_with("zh");
    match (zh, resource_type) {
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
    }
    .into()
}

pub(super) fn target_root(path: &Path) -> Result<PathBuf, String> {
    if !path.is_absolute()
        || path
            .components()
            .any(|c| matches!(c, std::path::Component::ParentDir))
    {
        return Err("目标必须是绝对目录，不能包含 ..".into());
    }
    files::ensure_no_links(path)?;
    if path.exists() {
        return files::canonical_dir(path);
    }
    let parent = path.parent().ok_or("无效目标目录")?;
    Ok(target_root(parent)?.join(path.file_name().ok_or("无效目标目录名")?))
}

fn build_plan(
    config: &MigrationConfig,
    types: &HashMap<String, Vec<String>>,
    mut excluded: HashSet<String>,
) -> Result<MigrationPlan, String> {
    let root = files::canonical_dir(Path::new(&config.source_dir))?;
    let target = target_root(Path::new(&config.target_dir))?;
    // A separate destination inside the source is not itself another input tree.
    if target != root && target.starts_with(&root) {
        excluded.insert(files::path_key(&target.to_string_lossy()));
    }
    let (disk_files, mut warnings) = files::inventory(&root, &excluded)?;
    let mut operations = Vec::new();
    let mut conflicts = Vec::new();
    let mut skipped = 0;
    let mut reserved = HashSet::new();
    for file in &disk_files {
        let source = Path::new(&file.path);
        let relative = source.strip_prefix(&root).map_err(|e| e.to_string())?;
        let package = files::package_name(source);
        let resource_types = if let Some(p) = &package {
            if let Some(known) = types.get(&p.id) {
                known.clone()
            } else if matches!(config.mode, MigrationMode::ByType | MigrationMode::ByScene) {
                match crate::services::var_parser::parse_var_file(source) {
                    Ok(pkg) => pkg.resource_types,
                    Err(e) => {
                        warnings.push(format!("未识别类型，按其他文件处理: {} ({})", file.path, e));
                        vec![]
                    }
                }
            } else {
                vec![]
            }
        } else {
            vec![]
        };
        let resource_types = normalize_resource_types(resource_types);
        let resource_type = primary_resource_type(resource_types.clone());
        // Dependency archives are a deliberate exception to flattening. Moving them back
        // to the root would undo the user's dependency-preservation organization.
        let archived = relative
            .components()
            .next()
            .map(|c| c.as_os_str() == REFERENCED_DIR)
            .unwrap_or(false);
        let folder = if archived {
            REFERENCED_DIR.to_string()
        } else {
            match config.mode {
                MigrationMode::Flatten => String::new(),
                MigrationMode::ByType => {
                    localized_type_folder(&resource_type, config.locale.as_deref())
                }
                MigrationMode::ByCreator => package
                    .as_ref()
                    .map(|p| p.creator.clone())
                    .unwrap_or_else(|| localized_type_folder("other", config.locale.as_deref())),
                MigrationMode::ByScene => {
                    if resource_types.iter().any(|t| t == "scene") {
                        package
                            .as_ref()
                            .map(|p| p.creator.clone())
                            .unwrap_or_else(|| "Scenes".into())
                    } else {
                        "dependencies".into()
                    }
                }
                MigrationMode::Custom => "packages".into(),
            }
        };
        let mut destination = target
            .join(folder)
            .join(source.file_name().ok_or("文件名不存在")?);
        if files::path_key(&file.path) == files::path_key(&destination.to_string_lossy()) {
            skipped += 1;
            continue;
        }
        let mut conflict = "none";
        if destination.exists()
            || reserved.contains(&files::path_key(&destination.to_string_lossy()))
        {
            if source
                .extension()
                .map(|e| e.eq_ignore_ascii_case("var"))
                .unwrap_or(false)
            {
                conflicts.push(format!(
                    "保留源文件，VAR 同名冲突（请先去重）: {} → {}",
                    file.path,
                    destination.display()
                ));
                skipped += 1;
                continue;
            }
            let parent = destination.parent().unwrap().to_path_buf();
            let stem = destination
                .file_stem()
                .unwrap_or_default()
                .to_string_lossy()
                .into_owned();
            let ext = destination
                .extension()
                .map(|e| format!(".{}", e.to_string_lossy()))
                .unwrap_or_default();
            let mut n = 2;
            loop {
                let candidate = parent.join(format!("{} ({}){}", stem, n, ext));
                if !candidate.exists()
                    && !reserved.contains(&files::path_key(&candidate.to_string_lossy()))
                {
                    destination = candidate;
                    break;
                }
                n += 1;
            }
            conflict = "rename";
            conflicts.push(format!(
                "普通文件同名，保留双方并重命名: {} → {}",
                file.path,
                destination.display()
            ));
        }
        files::ensure_no_links(&destination)?;
        reserved.insert(files::path_key(&destination.to_string_lossy()));
        operations.push(MigrationOperation {
            package_id: package
                .map(|p| p.id)
                .unwrap_or_else(|| source.file_name().unwrap().to_string_lossy().into_owned()),
            source: file.path.clone(),
            destination: destination.to_string_lossy().into_owned(),
            action: if config.dry_run { "copy" } else { "move" }.into(),
            size_bytes: file.size,
            resource_type,
            conflict: conflict.into(),
            modified_time: file.modified.clone(),
        });
    }
    let preview = MigrationPreview {
        plan_id: format!(
            "mig_{}",
            chrono::Utc::now().timestamp_nanos_opt().unwrap_or_default()
        ),
        source_dir: root.to_string_lossy().into_owned(),
        target_dir: target.to_string_lossy().into_owned(),
        total_files: disk_files.len(),
        total_operations: operations.len(),
        total_size_bytes: operations.iter().map(|o| o.size_bytes).sum(),
        operations,
        conflicts,
        skipped,
        warnings,
        cleanup_empty_dirs: !config.dry_run,
    };
    Ok(MigrationPlan {
        validate_directory: true,
        preview,
        files: disk_files,
        excluded,
    })
}

#[tauri::command]
pub async fn preview_migration(
    app_handle: tauri::AppHandle,
    config: MigrationConfig,
) -> Result<MigrationPreview, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let _guard = files::maintenance_lock()?;
        let types = app_handle
            .state::<Database>()
            .with_conn(|conn| {
                let mut stmt = conn.prepare("SELECT id,resource_types FROM packages")?;
                let rows =
                    stmt.query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)))?;
                let mut result = HashMap::new();
                for row in rows {
                    let (id, json) = row?;
                    result.insert(id, serde_json::from_str(&json).unwrap_or_default());
                }
                Ok(result)
            })
            .map_err(|e| e.to_string())?;
        let plan = build_plan(
            &config,
            &types,
            crate::commands::deduplication::managed_link_paths(&app_handle),
        )?;
        let preview = plan.preview.clone();
        *app_handle
            .state::<MigrationState>()
            .0
            .lock()
            .map_err(|e| e.to_string())? = Some(plan);
        Ok(preview)
    })
    .await
    .map_err(|e| e.to_string())?
}

fn execute_one(db: &Database, task_id: &str, op: &MigrationOperation) -> Result<(), String> {
    files::verify(&DiskFile {
        path: op.source.clone(),
        size: op.size_bytes,
        modified: op.modified_time.clone(),
    })?;
    files::ensure_no_links(Path::new(&op.destination))?;
    db.with_conn(|conn| {
        let tx=conn.unchecked_transaction()?;
        tx.execute("INSERT INTO resource_migration_log (task_id,package_id,source_path,destination_path,action,size_bytes,status,completed_at,file_modified_time) VALUES (?1,?2,?3,?4,?5,?6,'completed',datetime('now'),?7)",rusqlite::params![task_id,op.package_id,op.source,op.destination,op.action,op.size_bytes,op.modified_time])?;
        files::transfer(Path::new(&op.source),Path::new(&op.destination),&op.action).map_err(AppError::Io)?;
        let update=(||->Result<(),AppError>{
            if op.action=="move" {files::update_index_path(&tx,&op.source,&op.destination)?;}
            // Unindexed resources are still migrated. Parsing failure must not discard them.
            if files::package_name(Path::new(&op.destination)).is_some() {
                if let Ok(pkg)=crate::services::var_parser::parse_var_file(Path::new(&op.destination)) {
                    files::index_parsed_package(&tx,&pkg)?;
                }
            }
            tx.commit()?; Ok(())
        })();
        if let Err(e)=update {
            let rollback=if op.action=="move" {files::transfer(Path::new(&op.destination),Path::new(&op.source),"move")}else{fs::remove_file(&op.destination).map_err(|e|e.to_string())};
            return Err(AppError::Io(format!("迁移索引失败: {}; 文件恢复: {:?}",e,rollback)));
        }
        Ok(())
    }).map_err(|e|e.to_string())
}

#[tauri::command]
pub async fn execute_migration(
    app_handle: tauri::AppHandle,
    plan_id: String,
) -> Result<MigrationResult, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let _guard = files::maintenance_lock()?;
        let plan = {
            let state = app_handle.state::<MigrationState>();
            let mut guard = state.0.lock().map_err(|e| e.to_string())?;
            if guard.as_ref().map(|p| &p.preview.plan_id) != Some(&plan_id) {
                return Err("迁移预览已失效，请重新预览".into());
            }
            guard.take().unwrap()
        };
        let root = Path::new(&plan.preview.source_dir);
        if plan.validate_directory {
            let (current, _) = files::inventory(root, &plan.excluded)?;
            if current != plan.files {
                return Err("源目录已变化，请重新预览".into());
            }
        } else {
            for file in &plan.files {
                files::verify(file)?;
            }
        }
        // Preflight all destinations before moving the first file.
        for op in &plan.preview.operations {
            files::ensure_no_links(Path::new(&op.destination))?;
            if fs::symlink_metadata(&op.destination).is_ok() {
                return Err(format!("目标已变化，请重新预览: {}", op.destination));
            }
        }
        let db = app_handle.state::<Database>();
        let mut result = MigrationResult {
            task_id: plan_id.clone(),
            completed: 0,
            failed: 0,
            total: plan.preview.operations.len(),
            errors: vec![],
            rollback_available: false,
            removed_directories: 0,
        };
        for op in &plan.preview.operations {
            match execute_one(&db, &plan_id, op) {
                Ok(()) => result.completed += 1,
                Err(e) => {
                    result.failed += 1;
                    result.errors.push(format!("{}: {}", op.source, e));
                }
            }
            let _ = app_handle.emit(
                "migration-progress",
                MigrationProgress {
                    task_id: plan_id.clone(),
                    phase: "executing".into(),
                    completed: result.completed,
                    failed: result.failed,
                    total: result.total,
                    current_package_id: Some(op.source.clone()),
                },
            );
        }
        if plan.preview.cleanup_empty_dirs {
            match files::remove_empty_dirs_excluding(root, &plan.excluded) {
                Ok((count, errors)) => {
                    result.removed_directories = count;
                    result.errors.extend(errors);
                }
                Err(e) => result.errors.push(e),
            }
        }
        result.rollback_available = result.completed > 0;
        crate::commands::library_events::emit_library_index_changed(
            &app_handle,
            "package_moved",
            &["packages", "dashboard", "dependencies", "folders", "dedupe"],
            vec![],
            plan.preview
                .operations
                .iter()
                .map(|o| o.destination.clone())
                .collect(),
        );
        Ok(result)
    })
    .await
    .map_err(|e| e.to_string())?
}

fn rollback_entries(
    db: &Database,
    task_id: Option<&str>,
) -> Result<MigrationRollbackResult, String> {
    let entries=db.with_conn(|conn|{
        let mut stmt=conn.prepare("SELECT id,package_id,source_path,destination_path,action,size_bytes,file_modified_time FROM resource_migration_log WHERE status='completed' AND rolled_back_at IS NULL AND (?1 IS NULL OR task_id=?1) ORDER BY id DESC")?;
        let rows=stmt.query_map([task_id],|r|Ok((r.get::<_,i64>(0)?,r.get::<_,String>(1)?,r.get::<_,String>(2)?,r.get::<_,String>(3)?,r.get::<_,String>(4)?,r.get::<_,u64>(5)?,r.get::<_,Option<String>>(6)?)))?.collect::<Result<Vec<_>,_>>()?;Ok(rows)
    }).map_err(|e|e.to_string())?;
    let mut result = MigrationRollbackResult {
        task_id: task_id.unwrap_or("all").into(),
        restored: 0,
        failed: 0,
        errors: vec![],
    };
    for (id, _package_id, source, destination, action, size, modified) in entries {
        let run = (|| -> Result<(), String> {
            let current = files::fingerprint(Path::new(&destination))?;
            if current.size != size
                || modified
                    .as_ref()
                    .map(|m| m != &current.modified)
                    .unwrap_or(false)
            {
                return Err(format!("目标文件已变化，不能回滚: {}", destination));
            }
            if action == "copy" {
                let source_snapshot = files::fingerprint(Path::new(&source))?;
                if source_snapshot.size != current.size
                    || source_snapshot.modified != current.modified
                {
                    return Err("原文件已变化或丢失，保留复制文件".into());
                }
            }
            if action != "move" && action != "copy" {
                return Err("未知迁移动作".into());
            }
            db.with_conn(|conn|{
                let tx=conn.unchecked_transaction()?;
                if action=="move" {files::update_index_path(&tx,&destination,&source)?;}
                else {
                    tx.execute("DELETE FROM physical_packages WHERE lower(replace(file_path,'\\','/'))=?1",[files::path_key(&destination)])?;
                    tx.execute("UPDATE packages SET file_path=?1 WHERE lower(replace(file_path,'\\','/'))=?2",rusqlite::params![source,files::path_key(&destination)])?;
                }
                tx.execute("UPDATE resource_migration_log SET rolled_back_at=datetime('now') WHERE id=?1",[id])?;
                if action=="move" {files::transfer(Path::new(&destination),Path::new(&source),"move").map_err(AppError::Io)?;}
                else {fs::remove_file(&destination)?;}
                if let Err(e)=tx.commit() {
                    let undo=files::transfer(Path::new(&source),Path::new(&destination),&action);
                    return Err(AppError::Io(format!("回滚索引提交失败: {}; 文件恢复: {:?}",e,undo)));
                }
                Ok(())
            }).map_err(|e|e.to_string())?;
            Ok(())
        })();
        match run {
            Ok(()) => result.restored += 1,
            Err(e) => {
                result.failed += 1;
                result.errors.push(e);
            }
        }
    }
    Ok(result)
}

async fn rollback_command(
    app: tauri::AppHandle,
    id: Option<String>,
) -> Result<MigrationRollbackResult, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let _guard = files::maintenance_lock()?;
        let result = rollback_entries(&app.state::<Database>(), id.as_deref())?;
        crate::commands::library_events::emit_library_index_changed(
            &app,
            "package_moved",
            &["packages", "dashboard", "folders"],
            vec![],
            vec![],
        );
        Ok(result)
    })
    .await
    .map_err(|e| e.to_string())?
}
#[tauri::command]
pub async fn rollback_migration(
    app_handle: tauri::AppHandle,
    task_id: String,
) -> Result<MigrationRollbackResult, String> {
    rollback_command(app_handle, Some(task_id)).await
}
#[tauri::command]
pub async fn rollback_all_migrations(
    app_handle: tauri::AppHandle,
) -> Result<MigrationRollbackResult, String> {
    rollback_command(app_handle, None).await
}

#[tauri::command]
pub async fn collect_scene_dependencies(
    app_handle: tauri::AppHandle,
    scene_package_id: String,
    target_dir: String,
) -> Result<MigrationPreview, String> {
    tauri::async_runtime::spawn_blocking(move||{
        let _guard=files::maintenance_lock()?;
        let target=target_root(Path::new(&target_dir))?;
        let rows=app_handle.state::<Database>().with_conn(|conn|{
            let mut stmt=conn.prepare("SELECT DISTINCT p.id,p.file_path,p.resource_types FROM dependencies d JOIN packages p ON p.id=d.depends_on_id WHERE d.package_id=?1")?;
            let rows=stmt.query_map([scene_package_id],|r|Ok((r.get::<_,String>(0)?,r.get::<_,String>(1)?,r.get::<_,String>(2)?)))?.collect::<Result<Vec<_>,_>>()?;Ok(rows)
        }).map_err(|e|e.to_string())?;
        let mut disk_files=Vec::new();let mut operations=Vec::new();let mut conflicts=Vec::new();let mut reserved=HashSet::new();
        for (id,path,types) in rows {
            let file=files::fingerprint(Path::new(&path))?;
            let destination=target.join(Path::new(&path).file_name().ok_or("无效文件名")?);
            if destination.exists() || !reserved.insert(files::path_key(&destination.to_string_lossy())) {
                conflicts.push(format!("目标已存在，保留源文件: {}",destination.display()));continue;
            }
            operations.push(MigrationOperation {package_id:id,source:path,destination:destination.to_string_lossy().into_owned(),action:"copy".into(),size_bytes:file.size,resource_type:primary_resource_type(serde_json::from_str::<Vec<String>>(&types).unwrap_or_default()),conflict:"none".into(),modified_time:file.modified.clone()});
            disk_files.push(file);
        }
        let preview=MigrationPreview {plan_id:format!("mig_{}",chrono::Utc::now().timestamp_nanos_opt().unwrap_or_default()),source_dir:String::new(),target_dir:target.to_string_lossy().into_owned(),total_files:disk_files.len()+conflicts.len(),total_operations:operations.len(),total_size_bytes:operations.iter().map(|o|o.size_bytes).sum(),skipped:conflicts.len(),operations,conflicts,warnings:vec![],cleanup_empty_dirs:false};
        *app_handle.state::<MigrationState>().0.lock().map_err(|e|e.to_string())?=Some(MigrationPlan {validate_directory:false,preview:preview.clone(),files:disk_files,excluded:HashSet::new()});Ok(preview)
    }).await.map_err(|e|e.to_string())?
}

#[cfg(test)]
mod tests {
    use super::*;
    use files::tests::TestDir;
    fn config(d: &TestDir) -> MigrationConfig {
        MigrationConfig {
            mode: MigrationMode::Flatten,
            source_dir: d.0.to_string_lossy().into_owned(),
            target_dir: d.0.to_string_lossy().into_owned(),
            dry_run: false,
            locale: Some("zh-CN".into()),
        }
    }
    #[test]
    fn flatten_includes_unindexed_and_ordinary_files_and_resolves_collisions() {
        let d = TestDir::new();
        d.write("sub/A.X.1.var", b"package");
        d.write("sub/readme.txt", b"one");
        d.write("other/readme.txt", b"two");
        d.write("readme.txt", b"root");
        d.write("other/A.X.1.var", b"another");
        d.write("依赖旧版本/A.X.0.var", b"pinned");
        let p = build_plan(&config(&d), &HashMap::new(), HashSet::new()).unwrap();
        assert_eq!(p.preview.total_files, 6);
        assert_eq!(p.preview.operations.len(), 3);
        assert_eq!(p.preview.skipped, 3);
        let dest: HashSet<_> = p
            .preview
            .operations
            .iter()
            .map(|o| &o.destination)
            .collect();
        assert_eq!(dest.len(), 3);
        assert!(p
            .preview
            .operations
            .iter()
            .all(|o| Path::new(&o.destination).parent() == Some(d.0.as_path())));
        assert_eq!(fs::read(d.0.join("readme.txt")).unwrap(), b"root");
    }
    #[test]
    fn move_then_cleanup_and_rollback_recreates_directories() {
        let d = TestDir::new();
        let a = d.write("nested/deep/readme.txt", b"one");
        let plan = build_plan(&config(&d), &HashMap::new(), HashSet::new()).unwrap();
        let dbdir = TestDir::new();
        let db = Database::new(&dbdir.0.join("test.db")).unwrap();
        execute_one(&db, &plan.preview.plan_id, &plan.preview.operations[0]).unwrap();
        assert!(!a.exists());
        assert_eq!(files::remove_empty_dirs(&d.0).unwrap().0, 2);
        let result = rollback_entries(&db, Some(&plan.preview.plan_id)).unwrap();
        assert_eq!(result.restored, 1);
        assert_eq!(fs::read(a).unwrap(), b"one");
    }
    #[test]
    fn stale_source_and_existing_destination_do_not_overwrite() {
        let d = TestDir::new();
        let a = d.write("sub/readme.txt", b"one");
        let p = build_plan(&config(&d), &HashMap::new(), HashSet::new()).unwrap();
        let dbdir = TestDir::new();
        let db = Database::new(&dbdir.0.join("test.db")).unwrap();
        d.write("readme.txt", b"keep");
        assert!(execute_one(&db, "task", &p.preview.operations[0]).is_err());
        assert!(a.exists());
        assert_eq!(fs::read(d.0.join("readme.txt")).unwrap(), b"keep");
        fs::write(&a, b"changed").unwrap();
        assert!(execute_one(&db, "task", &p.preview.operations[0]).is_err());
    }
    #[test]
    fn rollback_refuses_changed_copy() {
        let d = TestDir::new();
        d.write("sub/readme.txt", b"one");
        let mut c = config(&d);
        c.dry_run = true;
        let p = build_plan(&c, &HashMap::new(), HashSet::new()).unwrap();
        let dbdir = TestDir::new();
        let db = Database::new(&dbdir.0.join("test.db")).unwrap();
        execute_one(&db, "task", &p.preview.operations[0]).unwrap();
        d.write("readme.txt", b"user edits");
        let result = rollback_entries(&db, Some("task")).unwrap();
        assert_eq!(result.failed, 1);
        assert!(d.0.join("readme.txt").exists());
    }
}
