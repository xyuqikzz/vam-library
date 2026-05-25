use serde::{Deserialize, Serialize};
use std::collections::{HashSet, VecDeque};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use tauri::{Emitter, State};
use walkdir::WalkDir;

use crate::db::Database;
use crate::errors::AppError;
use crate::services::install_context::write_managed_state;
use rusqlite::{params, OptionalExtension};

const MANIFEST_FILE: &str = "on-demand-links.json";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OnDemandOperationResult {
    pub completed: usize,
    pub failed: usize,
    pub skipped: usize,
    pub total: usize,
    pub errors: Vec<String>,
    pub launched: bool,
    pub launch_path: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OnDemandProgress {
    pub phase: String,
    pub completed: usize,
    pub failed: usize,
    pub skipped: usize,
    pub total: usize,
    pub current_file: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OnDemandPlan {
    pub id: String,
    pub name: String,
    pub package_ids: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OnDemandPlansState {
    pub plans: Vec<OnDemandPlan>,
    pub active_plan_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct LinkManifest {
    links: Vec<LinkManifestEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct LinkManifestEntry {
    source: String,
    link: String,
    package_id: String,
    link_type: String,
}

#[derive(Debug, Clone)]
struct PackagePath {
    id: String,
    file_path: String,
}

#[derive(Debug, Clone)]
struct InstalledPackage {
    id: String,
    creator: String,
    name: String,
    version: i32,
}

#[tauri::command]
pub async fn list_on_demand_plans(db: State<'_, Database>) -> Result<OnDemandPlansState, String> {
    db.with_conn(|conn| {
        let mut stmt = conn
            .prepare("SELECT id, name, package_ids FROM on_demand_plans ORDER BY created_at, id")
            .map_err(|e| {
                AppError::Database(format!("Failed to prepare on-demand plans query: {}", e))
            })?;

        let plans = stmt
            .query_map([], |row| {
                let package_ids_json: String = row.get(2)?;
                let package_ids = serde_json::from_str(&package_ids_json).unwrap_or_default();
                Ok(OnDemandPlan {
                    id: row.get(0)?,
                    name: row.get(1)?,
                    package_ids,
                })
            })
            .map_err(|e| AppError::Database(format!("Failed to query on-demand plans: {}", e)))?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| AppError::Database(format!("Failed to collect on-demand plans: {}", e)))?;

        let active_plan_id = conn
            .query_row(
                "SELECT value FROM on_demand_state WHERE key = 'active_plan_id'",
                [],
                |row| row.get(0),
            )
            .optional()
            .map_err(|e| {
                AppError::Database(format!("Failed to load active on-demand plan: {}", e))
            })?;

        Ok(OnDemandPlansState {
            plans,
            active_plan_id,
        })
    })
    .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn save_on_demand_plans(
    db: State<'_, Database>,
    plans: Vec<OnDemandPlan>,
    active_plan_id: Option<String>,
) -> Result<(), String> {
    db.with_conn(|conn| {
        let tx = conn.unchecked_transaction().map_err(|e| {
            AppError::Database(format!("Failed to begin on-demand plans save: {}", e))
        })?;

        tx.execute("DELETE FROM on_demand_plans", [])
            .map_err(|e| AppError::Database(format!("Failed to clear on-demand plans: {}", e)))?;

        for plan in plans {
            let package_ids_json = serde_json::to_string(&plan.package_ids).map_err(|e| {
                AppError::Database(format!(
                    "Failed to serialize on-demand plan packages: {}",
                    e
                ))
            })?;
            tx.execute(
                "INSERT INTO on_demand_plans (id, name, package_ids, updated_at)
                 VALUES (?1, ?2, ?3, datetime('now'))",
                params![plan.id, plan.name, package_ids_json],
            )
            .map_err(|e| AppError::Database(format!("Failed to save on-demand plan: {}", e)))?;
        }

        match active_plan_id {
            Some(active_plan_id) => {
                tx.execute(
                    "INSERT OR REPLACE INTO on_demand_state (key, value)
                     VALUES ('active_plan_id', ?1)",
                    [active_plan_id],
                )
                .map_err(|e| {
                    AppError::Database(format!("Failed to save active on-demand plan: {}", e))
                })?;
            }
            None => {
                tx.execute(
                    "DELETE FROM on_demand_state WHERE key = 'active_plan_id'",
                    [],
                )
                .map_err(|e| {
                    AppError::Database(format!("Failed to clear active on-demand plan: {}", e))
                })?;
            }
        }

        tx.commit().map_err(|e| {
            AppError::Database(format!("Failed to commit on-demand plans save: {}", e))
        })?;
        Ok(())
    })
    .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn migrate_on_demand_library(
    app_handle: tauri::AppHandle,
    db: State<'_, Database>,
    vam_root: String,
) -> Result<OnDemandOperationResult, String> {
    let root = PathBuf::from(vam_root);
    let addon_dir = root.join("AddonPackages");
    let library_dir = on_demand_library_dir(&root);

    if !addon_dir.exists() {
        return Err(format!("AddonPackages 目录不存在: {}", addon_dir.display()));
    }

    fs::create_dir_all(&library_dir)
        .map_err(|e| format!("创建托管库失败 {}: {}", library_dir.display(), e))?;

    let mut files = Vec::new();
    for entry in WalkDir::new(&addon_dir)
        .follow_links(false)
        .into_iter()
        .filter_map(|entry| entry.ok())
    {
        let path = entry.path();
        if !entry.file_type().is_file() || !is_var_file(path) {
            continue;
        }
        if path.starts_with(&library_dir) {
            continue;
        }
        if fs::symlink_metadata(path)
            .map(|m| m.file_type().is_symlink())
            .unwrap_or(false)
        {
            continue;
        }
        files.push(path.to_path_buf());
    }

    let total = files.len();
    let mut completed = 0usize;
    let mut failed = 0usize;
    let mut skipped = 0usize;
    let mut errors = Vec::new();
    let mut db_updates: Vec<(String, String)> = Vec::new();

    emit_on_demand_progress(
        &app_handle,
        OnDemandProgress {
            phase: "migrate".to_string(),
            completed,
            failed,
            skipped,
            total,
            current_file: None,
        },
    );

    for source in files {
        let relative = match source.strip_prefix(&addon_dir) {
            Ok(path) => path,
            Err(e) => {
                failed += 1;
                errors.push(format!("计算相对路径失败 {}: {}", source.display(), e));
                emit_on_demand_progress(
                    &app_handle,
                    OnDemandProgress {
                        phase: "migrate".to_string(),
                        completed,
                        failed,
                        skipped,
                        total,
                        current_file: Some(source.to_string_lossy().to_string()),
                    },
                );
                continue;
            }
        };
        let destination = library_dir.join(relative);

        if destination.exists() {
            skipped += 1;
            errors.push(format!("目标已存在，已跳过: {}", destination.display()));
            emit_on_demand_progress(
                &app_handle,
                OnDemandProgress {
                    phase: "migrate".to_string(),
                    completed,
                    failed,
                    skipped,
                    total,
                    current_file: Some(source.to_string_lossy().to_string()),
                },
            );
            continue;
        }

        if let Some(parent) = destination.parent() {
            if let Err(e) = fs::create_dir_all(parent) {
                failed += 1;
                errors.push(format!("创建目录失败 {}: {}", parent.display(), e));
                emit_on_demand_progress(
                    &app_handle,
                    OnDemandProgress {
                        phase: "migrate".to_string(),
                        completed,
                        failed,
                        skipped,
                        total,
                        current_file: Some(source.to_string_lossy().to_string()),
                    },
                );
                continue;
            }
        }

        match fs::rename(&source, &destination) {
            Ok(_) => {
                db_updates.push((
                    source.to_string_lossy().to_string(),
                    destination.to_string_lossy().to_string(),
                ));
                completed += 1;
            }
            Err(e) => {
                failed += 1;
                errors.push(format!(
                    "迁移失败 {} -> {}: {}",
                    source.display(),
                    destination.display(),
                    e
                ));
            }
        }

        emit_on_demand_progress(
            &app_handle,
            OnDemandProgress {
                phase: "migrate".to_string(),
                completed,
                failed,
                skipped,
                total,
                current_file: Some(source.to_string_lossy().to_string()),
            },
        );
    }

    update_package_paths(&db, &db_updates)?;
    write_managed_state(&root, &library_dir, None)?;
    crate::commands::settings::mark_managed_enabled(
        &app_handle,
        Some(library_dir.to_string_lossy().to_string()),
    )?;
    if !db_updates.is_empty() {
        crate::commands::library_events::emit_library_index_changed(
            &app_handle,
            "package_moved",
            &["packages", "dashboard", "statistics", "folders", "dedupe"],
            Vec::new(),
            db_updates
                .iter()
                .map(|(_, destination)| destination.clone())
                .collect(),
        );
    }

    emit_on_demand_progress(
        &app_handle,
        OnDemandProgress {
            phase: "done".to_string(),
            completed,
            failed,
            skipped,
            total,
            current_file: None,
        },
    );

    Ok(OnDemandOperationResult {
        completed,
        failed,
        skipped,
        total,
        errors,
        launched: false,
        launch_path: None,
    })
}

#[tauri::command]
pub async fn restore_on_demand_library(
    app_handle: tauri::AppHandle,
    db: State<'_, Database>,
    vam_root: String,
) -> Result<OnDemandOperationResult, String> {
    let root = PathBuf::from(vam_root);
    let addon_dir = root.join("AddonPackages");
    let library_dir = on_demand_library_dir(&root);
    let manifest_path = manifest_path(&root);

    fs::create_dir_all(&addon_dir)
        .map_err(|e| format!("创建 AddonPackages 目录失败 {}: {}", addon_dir.display(), e))?;

    let clear_result = clear_manifest_links(&manifest_path, &addon_dir);
    let mut errors = clear_result.errors;

    let mut files = Vec::new();
    if library_dir.exists() {
        for entry in WalkDir::new(&library_dir)
            .follow_links(false)
            .into_iter()
            .filter_map(|entry| entry.ok())
        {
            let path = entry.path();
            if entry.file_type().is_file() && is_var_file(path) {
                files.push(path.to_path_buf());
            }
        }
    }

    let total = files.len();
    let mut completed = 0usize;
    let mut failed = 0usize;
    let mut skipped = clear_result.skipped;
    let mut db_updates: Vec<(String, String)> = Vec::new();

    emit_on_demand_progress(
        &app_handle,
        OnDemandProgress {
            phase: "restore".to_string(),
            completed,
            failed,
            skipped,
            total,
            current_file: None,
        },
    );

    for source in files {
        let relative = match source.strip_prefix(&library_dir) {
            Ok(path) => path,
            Err(e) => {
                failed += 1;
                errors.push(format!("计算恢复路径失败 {}: {}", source.display(), e));
                emit_on_demand_progress(
                    &app_handle,
                    OnDemandProgress {
                        phase: "restore".to_string(),
                        completed,
                        failed,
                        skipped,
                        total,
                        current_file: Some(source.to_string_lossy().to_string()),
                    },
                );
                continue;
            }
        };
        let destination = addon_dir.join(relative);

        if destination.exists() {
            skipped += 1;
            errors.push(format!(
                "原目录已存在同名文件，已跳过: {}",
                destination.display()
            ));
            emit_on_demand_progress(
                &app_handle,
                OnDemandProgress {
                    phase: "restore".to_string(),
                    completed,
                    failed,
                    skipped,
                    total,
                    current_file: Some(source.to_string_lossy().to_string()),
                },
            );
            continue;
        }

        if let Some(parent) = destination.parent() {
            if let Err(e) = fs::create_dir_all(parent) {
                failed += 1;
                errors.push(format!("创建恢复目录失败 {}: {}", parent.display(), e));
                emit_on_demand_progress(
                    &app_handle,
                    OnDemandProgress {
                        phase: "restore".to_string(),
                        completed,
                        failed,
                        skipped,
                        total,
                        current_file: Some(source.to_string_lossy().to_string()),
                    },
                );
                continue;
            }
        }

        match fs::rename(&source, &destination) {
            Ok(_) => {
                db_updates.push((
                    source.to_string_lossy().to_string(),
                    destination.to_string_lossy().to_string(),
                ));
                completed += 1;
            }
            Err(e) => {
                failed += 1;
                errors.push(format!(
                    "恢复失败 {} -> {}: {}",
                    source.display(),
                    destination.display(),
                    e
                ));
            }
        }

        emit_on_demand_progress(
            &app_handle,
            OnDemandProgress {
                phase: "restore".to_string(),
                completed,
                failed,
                skipped,
                total,
                current_file: Some(source.to_string_lossy().to_string()),
            },
        );
    }

    update_package_paths(&db, &db_updates)?;
    mark_migration_logs_restored(&db, &db_updates)?;
    let _ = fs::remove_file(root.join("VAMBoxLibrary").join("managed-state.json"));
    crate::commands::settings::mark_managed_disabled(&app_handle)?;

    if !db_updates.is_empty() {
        crate::commands::library_events::emit_library_index_changed(
            &app_handle,
            "package_moved",
            &["packages", "dashboard", "statistics", "folders", "dedupe"],
            Vec::new(),
            db_updates
                .iter()
                .map(|(_, destination)| destination.clone())
                .collect(),
        );
    }

    emit_on_demand_progress(
        &app_handle,
        OnDemandProgress {
            phase: "done".to_string(),
            completed,
            failed,
            skipped,
            total,
            current_file: None,
        },
    );

    Ok(OnDemandOperationResult {
        completed,
        failed,
        skipped,
        total,
        errors,
        launched: false,
        launch_path: None,
    })
}

#[tauri::command]
pub async fn apply_on_demand_plan(
    app_handle: tauri::AppHandle,
    db: State<'_, Database>,
    vam_root: String,
    package_ids: Vec<String>,
    include_dependencies: bool,
) -> Result<OnDemandOperationResult, String> {
    let root = PathBuf::from(vam_root);
    let addon_dir = root.join("AddonPackages");
    let library_dir = on_demand_library_dir(&root);
    let manifest_path = manifest_path(&root);

    if !addon_dir.exists() {
        return Err(format!("AddonPackages 目录不存在: {}", addon_dir.display()));
    }
    fs::create_dir_all(&addon_dir)
        .map_err(|e| format!("创建 AddonPackages 目录失败 {}: {}", addon_dir.display(), e))?;

    let mut errors = Vec::new();
    let package_ids = collect_plan_package_ids(&db, package_ids, include_dependencies)?;
    let package_paths = load_package_paths(&db, &package_ids)?;

    let conflict_errors =
        validate_plan_targets(&package_paths, &library_dir, &addon_dir, &manifest_path);
    if !conflict_errors.is_empty() {
        let total = package_paths.len();
        return Ok(OnDemandOperationResult {
            completed: 0,
            failed: conflict_errors.len(),
            skipped: 0,
            total,
            errors: conflict_errors,
            launched: false,
            launch_path: None,
        });
    }

    let clear_result = clear_manifest_links(&manifest_path, &addon_dir);
    errors.extend(clear_result.errors);

    let total = package_paths.len();
    let mut completed = 0usize;
    let mut failed = 0usize;
    let mut skipped = 0usize;
    let mut manifest = LinkManifest { links: Vec::new() };

    emit_on_demand_progress(
        &app_handle,
        OnDemandProgress {
            phase: "apply".to_string(),
            completed,
            failed,
            skipped,
            total,
            current_file: None,
        },
    );

    for package in package_paths {
        let source = PathBuf::from(&package.file_path);
        if !source.exists() {
            failed += 1;
            errors.push(format!("源文件不存在: {}", source.display()));
            emit_on_demand_progress(
                &app_handle,
                OnDemandProgress {
                    phase: "apply".to_string(),
                    completed,
                    failed,
                    skipped,
                    total,
                    current_file: Some(package.file_path.clone()),
                },
            );
            continue;
        }

        let target = target_link_path(&source, &library_dir, &addon_dir);
        if source == target {
            skipped += 1;
            emit_on_demand_progress(
                &app_handle,
                OnDemandProgress {
                    phase: "apply".to_string(),
                    completed,
                    failed,
                    skipped,
                    total,
                    current_file: Some(package.file_path.clone()),
                },
            );
            continue;
        }

        if target.exists() {
            skipped += 1;
            errors.push(format!(
                "目标已有真实文件或未纳管链接，已跳过: {}",
                target.display()
            ));
            emit_on_demand_progress(
                &app_handle,
                OnDemandProgress {
                    phase: "apply".to_string(),
                    completed,
                    failed,
                    skipped,
                    total,
                    current_file: Some(package.file_path.clone()),
                },
            );
            continue;
        }

        if let Some(parent) = target.parent() {
            if let Err(e) = fs::create_dir_all(parent) {
                failed += 1;
                errors.push(format!("创建目录失败 {}: {}", parent.display(), e));
                emit_on_demand_progress(
                    &app_handle,
                    OnDemandProgress {
                        phase: "apply".to_string(),
                        completed,
                        failed,
                        skipped,
                        total,
                        current_file: Some(package.file_path.clone()),
                    },
                );
                continue;
            }
        }

        match create_file_link(&source, &target) {
            Ok(link_type) => {
                manifest.links.push(LinkManifestEntry {
                    source: source.to_string_lossy().to_string(),
                    link: target.to_string_lossy().to_string(),
                    package_id: package.id.clone(),
                    link_type,
                });
                completed += 1;
            }
            Err(e) => {
                failed += 1;
                errors.push(format!(
                    "创建映射失败 {} -> {}: {}",
                    source.display(),
                    target.display(),
                    e
                ));
            }
        }

        emit_on_demand_progress(
            &app_handle,
            OnDemandProgress {
                phase: "apply".to_string(),
                completed,
                failed,
                skipped,
                total,
                current_file: Some(package.file_path.clone()),
            },
        );
    }

    if let Some(parent) = manifest_path.parent() {
        fs::create_dir_all(parent)
            .map_err(|e| format!("创建映射清单目录失败 {}: {}", parent.display(), e))?;
    }
    let manifest_json = serde_json::to_string_pretty(&manifest)
        .map_err(|e| format!("序列化映射清单失败: {}", e))?;
    fs::write(&manifest_path, manifest_json)
        .map_err(|e| format!("写入映射清单失败 {}: {}", manifest_path.display(), e))?;
    write_managed_state(&root, &library_dir, None)?;

    let mut launched = false;
    let mut launch_path = None;
    if failed == 0 {
        match launch_vam_game(&root) {
            Ok(path) => {
                launched = true;
                launch_path = Some(path.to_string_lossy().to_string());
            }
            Err(e) => {
                failed += 1;
                errors.push(e);
            }
        }
    }

    emit_on_demand_progress(
        &app_handle,
        OnDemandProgress {
            phase: "done".to_string(),
            completed,
            failed,
            skipped: skipped + clear_result.skipped,
            total,
            current_file: None,
        },
    );

    Ok(OnDemandOperationResult {
        completed,
        failed,
        skipped: skipped + clear_result.skipped,
        total,
        errors,
        launched,
        launch_path,
    })
}

fn emit_on_demand_progress(app_handle: &tauri::AppHandle, progress: OnDemandProgress) {
    let _ = app_handle.emit("on-demand-operation-progress", progress);
}

fn update_package_paths(db: &Database, updates: &[(String, String)]) -> Result<(), String> {
    if updates.is_empty() {
        return Ok(());
    }

    db.with_conn(|conn| {
        let tx = conn.unchecked_transaction().map_err(|e| {
            AppError::Database(format!("Failed to begin on-demand path update: {}", e))
        })?;

        for (source, destination) in updates {
            // 恢复/迁移前目标文件已确认不存在，这里清掉可能残留的旧索引，避免唯一路径约束冲突。
            tx.execute(
                "DELETE FROM physical_packages WHERE file_path = ?1 AND file_path <> ?2",
                rusqlite::params![destination, source],
            )
            .map_err(|e| {
                AppError::Database(format!("Failed to clear stale physical package path: {}", e))
            })?;
            tx.execute(
                "UPDATE packages SET file_path = ?1, updated_at = datetime('now') WHERE file_path = ?2",
                rusqlite::params![destination, source],
            )
            .map_err(|e| AppError::Database(format!("Failed to update package path: {}", e)))?;
            tx.execute(
                "UPDATE physical_packages SET file_path = ?1 WHERE file_path = ?2",
                rusqlite::params![destination, source],
            )
            .map_err(|e| {
                AppError::Database(format!("Failed to update physical package path: {}", e))
            })?;
        }

        tx.commit().map_err(|e| {
            AppError::Database(format!("Failed to commit on-demand path update: {}", e))
        })?;
        Ok(())
    })
    .map_err(|e| e.to_string())
}

fn mark_migration_logs_restored(db: &Database, updates: &[(String, String)]) -> Result<(), String> {
    if updates.is_empty() {
        return Ok(());
    }

    db.with_conn(|conn| {
        for (source, destination) in updates {
            conn.execute(
                "UPDATE resource_migration_log
                 SET rolled_back_at = datetime('now')
                 WHERE destination_path = ?1
                   AND source_path = ?2
                   AND status = 'completed'
                   AND rolled_back_at IS NULL",
                rusqlite::params![source, destination],
            )
            .map_err(|e| {
                AppError::Database(format!("Failed to mark migration log restored: {}", e))
            })?;
        }
        Ok(())
    })
    .map_err(|e| e.to_string())
}

fn collect_plan_package_ids(
    db: &Database,
    package_ids: Vec<String>,
    include_dependencies: bool,
) -> Result<Vec<String>, String> {
    let mut selected: HashSet<String> = package_ids.into_iter().collect();
    if !include_dependencies {
        let mut list: Vec<String> = selected.into_iter().collect();
        list.sort();
        return Ok(list);
    }

    let (dependency_pairs, installed_packages): (Vec<(String, String)>, Vec<InstalledPackage>) = db
        .with_conn(|conn| {
            let mut dep_stmt = conn
                .prepare("SELECT package_id, depends_on_id FROM dependencies")
                .map_err(|e| AppError::Database(e.to_string()))?;
            let pairs = dep_stmt
                .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))
                .map_err(|e| AppError::Database(e.to_string()))?
                .collect::<Result<Vec<_>, _>>()
                .map_err(|e| AppError::Database(e.to_string()))?;

            let mut pkg_stmt = conn
                .prepare("SELECT id, creator, name, version FROM packages")
                .map_err(|e| AppError::Database(e.to_string()))?;
            let installed = pkg_stmt
                .query_map([], |row| {
                    Ok(InstalledPackage {
                        id: row.get(0)?,
                        creator: row.get(1)?,
                        name: row.get(2)?,
                        version: row.get(3)?,
                    })
                })
                .map_err(|e| AppError::Database(e.to_string()))?
                .collect::<Result<Vec<_>, _>>()
                .map_err(|e| AppError::Database(e.to_string()))?;

            Ok((pairs, installed))
        })
        .map_err(|e| e.to_string())?;

    let mut queue: VecDeque<String> = selected.iter().cloned().collect();
    while let Some(current) = queue.pop_front() {
        for (_, dep_id) in dependency_pairs
            .iter()
            .filter(|(source, _)| source == &current)
        {
            let Some(resolved_dep_id) =
                resolve_installed_dependency_id(dep_id, &installed_packages)
            else {
                continue;
            };
            if selected.insert(resolved_dep_id.clone()) {
                queue.push_back(resolved_dep_id);
            }
        }
    }

    let mut list: Vec<String> = selected.into_iter().collect();
    list.sort();
    Ok(list)
}

fn resolve_installed_dependency_id(
    depends_on_id: &str,
    installed_packages: &[InstalledPackage],
) -> Option<String> {
    if installed_packages.iter().any(|pkg| pkg.id == depends_on_id) {
        return Some(depends_on_id.to_string());
    }

    let parts: Vec<&str> = depends_on_id.split('.').collect();
    if parts.len() < 2 {
        return None;
    }

    let creator = parts[0];
    let (name, required_version) = if parts.len() >= 3 {
        let version_part = parts.last().copied().unwrap_or_default();
        (
            parts[1..parts.len() - 1].join("."),
            parse_required_version(version_part),
        )
    } else {
        (parts[1].to_string(), None)
    };

    let mut candidates: Vec<&InstalledPackage> = installed_packages
        .iter()
        .filter(|pkg| pkg.creator == creator && pkg.name == name)
        .collect();

    if candidates.is_empty() {
        return None;
    }

    candidates.sort_by_key(|pkg| pkg.version);

    if let Some(required) = required_version {
        candidates
            .into_iter()
            .filter(|pkg| pkg.version >= required)
            .max_by_key(|pkg| pkg.version)
            .map(|pkg| pkg.id.clone())
    } else {
        candidates
            .into_iter()
            .max_by_key(|pkg| pkg.version)
            .map(|pkg| pkg.id.clone())
    }
}

fn parse_required_version(version_part: &str) -> Option<i32> {
    if version_part.eq_ignore_ascii_case("latest") {
        None
    } else {
        version_part.parse::<i32>().ok()
    }
}

fn load_package_paths(db: &Database, package_ids: &[String]) -> Result<Vec<PackagePath>, String> {
    if package_ids.is_empty() {
        return Ok(Vec::new());
    }

    db.with_conn(|conn| {
        let mut packages = Vec::new();
        let mut stmt = conn
            .prepare("SELECT id, file_path FROM packages WHERE id = ?1")
            .map_err(|e| AppError::Database(e.to_string()))?;

        for package_id in package_ids {
            match stmt.query_row([package_id], |row| {
                Ok(PackagePath {
                    id: row.get(0)?,
                    file_path: row.get(1)?,
                })
            }) {
                Ok(package) => packages.push(package),
                Err(rusqlite::Error::QueryReturnedNoRows) => {}
                Err(e) => return Err(AppError::Database(e.to_string())),
            }
        }

        Ok(packages)
    })
    .map_err(|e| e.to_string())
}

fn clear_manifest_links(manifest_path: &Path, addon_dir: &Path) -> OnDemandOperationResult {
    let mut result = OnDemandOperationResult {
        completed: 0,
        failed: 0,
        skipped: 0,
        total: 0,
        errors: Vec::new(),
        launched: false,
        launch_path: None,
    };

    if !manifest_path.exists() {
        return result;
    }

    let content = match fs::read_to_string(manifest_path) {
        Ok(content) => content,
        Err(e) => {
            result.failed += 1;
            result.errors.push(format!("读取旧映射清单失败: {}", e));
            return result;
        }
    };

    let manifest: LinkManifest = match serde_json::from_str(&content) {
        Ok(manifest) => manifest,
        Err(e) => {
            result.failed += 1;
            result.errors.push(format!("解析旧映射清单失败: {}", e));
            return result;
        }
    };

    result.total = manifest.links.len();
    for entry in manifest.links {
        let link = PathBuf::from(entry.link);
        let source = PathBuf::from(entry.source);
        if !link.starts_with(addon_dir) || link.extension().and_then(|e| e.to_str()) != Some("var")
        {
            result.skipped += 1;
            continue;
        }
        if !link.exists() {
            result.skipped += 1;
            continue;
        }
        if !is_manifest_owned_link(&source, &link, &entry.link_type) {
            result.failed += 1;
            result.errors.push(format!(
                "旧映射目标不是 VAM Library 清单对应的链接，已跳过: {}",
                link.display()
            ));
            continue;
        }
        match fs::remove_file(&link) {
            Ok(_) => result.completed += 1,
            Err(e) => {
                result.failed += 1;
                result
                    .errors
                    .push(format!("清理旧映射失败 {}: {}", link.display(), e));
            }
        }
    }

    let _ = fs::remove_file(manifest_path);
    result
}

fn validate_plan_targets(
    package_paths: &[PackagePath],
    library_dir: &Path,
    addon_dir: &Path,
    manifest_path: &Path,
) -> Vec<String> {
    let existing_manifest_links = load_manifest_link_paths(manifest_path);
    let mut errors = Vec::new();

    for package in package_paths {
        let source = PathBuf::from(&package.file_path);
        if !source.exists() {
            errors.push(format!("源文件不存在: {}", source.display()));
            continue;
        }

        let target = target_link_path(&source, library_dir, addon_dir);
        if source == target {
            continue;
        }

        if target.exists() && !existing_manifest_links.contains(&target) {
            errors.push(format!(
                "目标存在真实文件或未知链接，应用方案前请先处理: {}",
                target.display()
            ));
        }
    }

    errors
}

fn load_manifest_link_paths(manifest_path: &Path) -> HashSet<PathBuf> {
    let Ok(content) = fs::read_to_string(manifest_path) else {
        return HashSet::new();
    };
    let Ok(manifest) = serde_json::from_str::<LinkManifest>(&content) else {
        return HashSet::new();
    };
    manifest
        .links
        .into_iter()
        .map(|entry| PathBuf::from(entry.link))
        .collect()
}

fn is_manifest_owned_link(source: &Path, link: &Path, link_type: &str) -> bool {
    if fs::symlink_metadata(link)
        .map(|metadata| metadata.file_type().is_symlink())
        .unwrap_or(false)
    {
        return link_type == "symlink";
    }

    if link_type != "hard_link" {
        return false;
    }

    match (fs::metadata(source), fs::metadata(link)) {
        (Ok(source_meta), Ok(link_meta)) => source_meta.len() == link_meta.len(),
        _ => false,
    }
}

fn target_link_path(source: &Path, library_dir: &Path, addon_dir: &Path) -> PathBuf {
    if let Ok(relative) = source.strip_prefix(library_dir) {
        addon_dir.join(relative)
    } else {
        addon_dir.join(source.file_name().unwrap_or_default())
    }
}

fn on_demand_library_dir(root: &Path) -> PathBuf {
    root.join("VAMBoxLibrary").join("AddonPackages")
}

fn manifest_path(root: &Path) -> PathBuf {
    root.join("VAMBoxLibrary").join(MANIFEST_FILE)
}

fn launch_vam_game(root: &Path) -> Result<PathBuf, String> {
    let launcher = find_vam_launcher(root).ok_or_else(|| {
        format!(
            "未找到 VAM 启动器，请确认目录中存在 VaM.exe 或常见启动脚本: {}",
            root.display()
        )
    })?;

    launch_process(root, &launcher)?;
    Ok(launcher)
}

fn find_vam_launcher(root: &Path) -> Option<PathBuf> {
    let candidates = [
        "VaM.exe",
        "VAM.exe",
        "VaM (Desktop Mode).bat",
        "VaM (OpenVR).bat",
        "VaM (Desktop Mode).cmd",
        "VaM (OpenVR).cmd",
    ];

    for candidate in candidates {
        let path = root.join(candidate);
        if path.exists() {
            return Some(path);
        }
    }

    let entries = fs::read_dir(root).ok()?;
    for entry in entries.flatten() {
        let path = entry.path();
        let Some(file_name) = path.file_name().and_then(|name| name.to_str()) else {
            continue;
        };
        if file_name.eq_ignore_ascii_case("vam.exe") {
            return Some(path);
        }
    }

    None
}

#[cfg(windows)]
fn launch_process(root: &Path, launcher: &Path) -> Result<(), String> {
    let extension = launcher
        .extension()
        .and_then(|ext| ext.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase();

    if extension == "bat" || extension == "cmd" {
        Command::new("cmd")
            .current_dir(root)
            .args(["/C", "start", ""])
            .arg(launcher)
            .spawn()
            .map_err(|e| format!("启动脚本失败 {}: {}", launcher.display(), e))?;
    } else {
        Command::new(launcher)
            .current_dir(root)
            .spawn()
            .map_err(|e| format!("启动游戏失败 {}: {}", launcher.display(), e))?;
    }

    Ok(())
}

#[cfg(not(windows))]
fn launch_process(root: &Path, launcher: &Path) -> Result<(), String> {
    Command::new(launcher)
        .current_dir(root)
        .spawn()
        .map_err(|e| format!("启动游戏失败 {}: {}", launcher.display(), e))?;
    Ok(())
}

fn is_var_file(path: &Path) -> bool {
    path.extension()
        .and_then(|ext| ext.to_str())
        .map(|ext| ext.eq_ignore_ascii_case("var"))
        .unwrap_or(false)
}

fn create_file_link(source: &Path, target: &Path) -> Result<String, String> {
    match fs::hard_link(source, target) {
        Ok(_) => return Ok("hard_link".to_string()),
        Err(hard_link_error) => {
            if let Err(symlink_error) = create_symlink_file(source, target) {
                return Err(format!(
                    "硬链接失败: {}; 符号链接失败: {}",
                    hard_link_error, symlink_error
                ));
            }
        }
    }

    Ok("symlink".to_string())
}

#[cfg(windows)]
fn create_symlink_file(source: &Path, target: &Path) -> std::io::Result<()> {
    std::os::windows::fs::symlink_file(source, target)
}

#[cfg(unix)]
fn create_symlink_file(source: &Path, target: &Path) -> std::io::Result<()> {
    std::os::unix::fs::symlink(source, target)
}

#[tauri::command]
pub async fn launch_vam_direct(vam_root: String) -> Result<String, String> {
    let root = PathBuf::from(vam_root);
    let launcher = launch_vam_game(&root)?;
    Ok(launcher.to_string_lossy().to_string())
}
