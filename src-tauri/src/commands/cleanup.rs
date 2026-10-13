use crate::{
    db::Database,
    services::{
        cleanup_indexer::{self, BackgroundIndexer, IndexHost, IndexStatus},
        install_context::resolve_install_context,
        resource_cleanup as cleanup, scene_launch,
    },
};
use cleanup::{Batch, Catalog, Context, Mark, PackageHealth, Plan, Policy, Preview};
use std::{
    collections::HashMap,
    path::PathBuf,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex,
    },
    time::{Duration, Instant},
};
use tauri::{Emitter, Manager, State};

#[derive(Default)]
pub struct CleanupState {
    plans: Mutex<HashMap<String, Plan>>,
    operations: Mutex<HashMap<String, Arc<AtomicBool>>>,
    pub executing: Arc<AtomicBool>,
    /// Last reference catalog, reused when the preview dialog re-plans
    /// (keep / select again / dependency toggle). Execution always rebuilds the
    /// catalog and compares revisions, so a stale entry can only cause a
    /// "please preview again" rejection, never an unsafe move.
    catalog: Mutex<Option<CachedCatalog>>,
    /// Keeps the reference index current in the background.
    indexer: Arc<BackgroundIndexer>,
    /// Problem and usage summary from the last full check, for the cleanup page.
    health: Mutex<Option<StoredHealth>>,
}
struct StoredHealth {
    library_key: String,
    /// Resources changed since; a background check will replace it.
    stale: bool,
    report: Arc<HealthReport>,
}
#[derive(Clone, serde::Serialize)]
pub struct HealthReport {
    pub packages: Vec<PackageHealth>,
    pub checked_at: String,
}
/// Publish the package summary of a full catalog (quick ones miss undeclared uses).
fn remember_health(app: &tauri::AppHandle, ctx: &Context, catalog: &Catalog) -> Arc<HealthReport> {
    let report = Arc::new(HealthReport {
        packages: cleanup::package_health(catalog),
        checked_at: chrono::Utc::now().to_rfc3339(),
    });
    if catalog.is_quick() {
        return report;
    }
    if let Some(state) = app.try_state::<CleanupState>() {
        if let Ok(mut health) = state.health.lock() {
            *health = Some(StoredHealth {
                library_key: ctx.library_key.clone(),
                stale: false,
                report: report.clone(),
            });
        }
    }
    let _ = app.emit("cleanup-health-changed", &ctx.library_key);
    report
}
struct CachedCatalog {
    ctx: Context,
    quick: bool,
    built: Instant,
    catalog: Arc<Catalog>,
}
const CATALOG_REUSE_WINDOW: Duration = Duration::from_secs(10 * 60);
/// Drop the reusable catalog after anything that changes resources or marks.
pub(crate) fn invalidate_catalog(app: &tauri::AppHandle) {
    if let Some(state) = app.try_state::<CleanupState>() {
        if let Ok(mut cached) = state.catalog.lock() {
            *cached = None;
        }
    }
}
pub(crate) struct ExecutionGuard(Arc<AtomicBool>);
impl Drop for ExecutionGuard {
    fn drop(&mut self) {
        self.0.store(false, Ordering::SeqCst);
    }
}
pub(crate) fn execution_guard(app: &tauri::AppHandle) -> Result<ExecutionGuard, String> {
    let state = app.state::<CleanupState>();
    let flag = state.executing.clone();
    if flag.swap(true, Ordering::SeqCst) {
        return Err("已有资源维护正在执行，请稍候".into());
    }
    // Give a file-moving operation the disk; indexing resumes afterwards.
    state.indexer.preempt();
    Ok(ExecutionGuard(flag))
}

/// Background indexing as seen by the app.
struct AppIndexHost(tauri::AppHandle);
impl IndexHost for AppIndexHost {
    fn enabled(&self) -> bool {
        self.0
            .try_state::<Database>()
            .and_then(|db| cleanup::policy(&db).ok())
            .is_some_and(|policy| policy.background_index)
    }
    fn busy(&self) -> bool {
        self.0.try_state::<CleanupState>().map_or(true, |state| {
            state.executing.load(Ordering::SeqCst)
                || state.operations.lock().map_or(true, |ops| !ops.is_empty())
        })
    }
    fn game_running(&self) -> bool {
        context(&self.0)
            .is_ok_and(|ctx| scene_launch::game_may_be_running(std::path::Path::new(&ctx.vam_root)))
    }
    fn run(
        &self,
        cancel: &AtomicBool,
        report: &mut dyn FnMut(&str, usize, usize),
    ) -> Result<(), String> {
        let ctx = context(&self.0)?;
        let db = self.0.state::<Database>();
        let pool = cleanup_indexer::background_pool()?;
        let catalog = cleanup::with_worker_pool(pool, || {
            cleanup::build_catalog(&db, &ctx, false, cancel, &mut |stage, n, t| {
                report(stage, n, t)
            })
        })?;
        remember_health(&self.0, &ctx, &catalog);
        Ok(())
    }
    fn publish(&self, status: &IndexStatus) {
        let _ = self.0.emit("cleanup-index-status", status);
    }
}
fn schedule_index_after(app: &tauri::AppHandle, delay: Duration) {
    if let Some(state) = app.try_state::<CleanupState>() {
        state
            .indexer
            .schedule(Arc::new(AppIndexHost(app.clone())), delay);
    }
}
/// Called whenever the local library index changes (scans, downloads,
/// cleanups, migrations): drop the reusable catalog and refresh the reference
/// index in the background once things are quiet.
pub fn on_library_changed(app: &tauri::AppHandle) {
    invalidate_catalog(app);
    if let Some(state) = app.try_state::<CleanupState>() {
        if let Ok(mut health) = state.health.lock() {
            if let Some(health) = health.as_mut() {
                health.stale = true;
            }
        }
    }
    schedule_index_after(app, Duration::from_secs(15));
}
fn preempt_indexer(app: &tauri::AppHandle) {
    if let Some(state) = app.try_state::<CleanupState>() {
        state.indexer.preempt();
    }
}
#[tauri::command]
pub async fn get_cleanup_index_status(app: tauri::AppHandle) -> Result<IndexStatus, String> {
    Ok(app.state::<CleanupState>().indexer.status())
}
pub fn ensure_idle(app: &tauri::AppHandle) -> Result<(), String> {
    if app
        .try_state::<CleanupState>()
        .is_some_and(|s| s.executing.load(Ordering::SeqCst))
    {
        return Err("资源维护执行中，请完成后再切换设置或游戏实例".into());
    }
    Ok(())
}
fn context(app: &tauri::AppHandle) -> Result<Context, String> {
    let mut ctx = Context::from_install(&resolve_install_context(app)?)?;
    #[derive(serde::Deserialize)]
    struct Instances {
        #[serde(default)]
        vam_instances: Vec<crate::commands::settings::VamInstance>,
    }
    let path = crate::services::install_context::settings_file_path(app)?;
    let instances: Instances =
        serde_json::from_reader(std::fs::File::open(path).map_err(|e| e.to_string())?)
            .map_err(|e| e.to_string())?;
    for instance in instances.vam_instances {
        let root = std::path::Path::new(&instance.root_path);
        if let Ok(canonical) = std::fs::canonicalize(root) {
            let key = crate::services::resource_files::path_key(
                canonical.to_string_lossy().trim_start_matches(r"\\?\"),
            );
            if key == ctx.library_key {
                continue;
            }
        }
        let library = instance
            .managed_library_path
            .map(PathBuf::from)
            .unwrap_or_else(|| root.join("VAMBoxLibrary/AddonPackages"));
        for other in [root.join("AddonPackages"), library] {
            if let Ok(other) = std::fs::canonicalize(other) {
                let other = crate::services::resource_files::path_key(
                    other.to_string_lossy().trim_start_matches(r"\\?\"),
                );
                if ctx.roots.iter().any(|r| {
                    let r = crate::services::resource_files::path_key(r);
                    r == other
                        || r.starts_with(&format!("{other}/"))
                        || other.starts_with(&format!("{r}/"))
                }) {
                    ctx.shared = true;
                }
            }
        }
    }
    Ok(ctx)
}
fn expected_context(app: &tauri::AppHandle, root: &str) -> Result<Context, String> {
    let ctx = context(app)?;
    let root = crate::services::resource_files::canonical_dir(std::path::Path::new(root))?;
    if crate::services::resource_files::path_key(&root.to_string_lossy()) != ctx.library_key {
        return Err("游戏实例已变化，请刷新页面".into());
    }
    Ok(ctx)
}
fn data(app: &tauri::AppHandle) -> Result<PathBuf, String> {
    app.path().app_data_dir().map_err(|e| e.to_string())
}
fn changed(app: &tauri::AppHandle, batch: &Batch) {
    crate::commands::library_events::emit_library_index_changed(
        app,
        "cleanup_changed",
        &[
            "packages",
            "dashboard",
            "dependencies",
            "statistics",
            "folders",
            "dedupe",
        ],
        batch.metadata.keys().cloned().collect(),
        vec![],
    );
}
fn progress(app: &tauri::AppHandle, id: &str, stage: &str, current: usize, total: usize) {
    let _ = app.emit(
        "cleanup-progress",
        serde_json::json!({"request_id":id,"stage":stage,"current":current,"total":total}),
    );
}
#[tauri::command]
pub async fn list_cleanup_marks(
    app: tauri::AppHandle,
    db: State<'_, Database>,
    vam_root: String,
) -> Result<Vec<Mark>, String> {
    cleanup::marks(&db, &expected_context(&app, &vam_root)?)
}
#[tauri::command]
pub async fn update_cleanup_marks(
    app: tauri::AppHandle,
    _db: State<'_, Database>,
    vam_root: String,
    package_ids: Vec<String>,
    action: String,
) -> Result<Vec<Mark>, String> {
    let ctx = expected_context(&app, &vam_root)?;
    tauri::async_runtime::spawn_blocking(move || {
        let _guard = crate::services::resource_files::maintenance_lock()?;
        let db = app.state::<Database>();
        cleanup::set_marks(&db, &ctx, &package_ids, &action)?;
        invalidate_catalog(&app);
        let _ = app.emit("cleanup-marks-changed", &ctx.library_key);
        cleanup::marks(&db, &ctx)
    })
    .await
    .map_err(|e| e.to_string())?
}
#[tauri::command]
pub async fn preview_resource_cleanup(
    app: tauri::AppHandle,
    vam_root: String,
    request_id: String,
    package_ids: Vec<String>,
    include_dependencies: bool,
    excluded_package_ids: Vec<String>,
    reuse_index: Option<bool>,
    quick: Option<bool>,
    force_package_ids: Option<Vec<String>>,
) -> Result<Preview, String> {
    let quick = quick.unwrap_or(false);
    if request_id.is_empty()
        || request_id.len() > 80
        || !request_id
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'-')
    {
        return Err("无效请求标识".into());
    }
    cleanup::validate_request(&package_ids)?;
    let ctx = expected_context(&app, &vam_root)?;
    let state = app.state::<CleanupState>();
    let cancel = Arc::new(AtomicBool::new(false));
    {
        let mut ops = state.operations.lock().map_err(|e| e.to_string())?;
        if ops.len() >= 8 || ops.contains_key(&request_id) {
            return Err("清理分析任务过多或请求已存在".into());
        }
        ops.insert(request_id.clone(), cancel.clone());
    }
    // A preview reads the same files; let it have the disk.
    preempt_indexer(&app);
    let worker = app.clone();
    let id = request_id.clone();
    let result = tauri::async_runtime::spawn_blocking(move || {
        let db = worker.state::<Database>();
        let cleanup_state = worker.state::<CleanupState>();
        let reusable = if reuse_index.unwrap_or(false) {
            cleanup_state
                .catalog
                .lock()
                .map_err(|e| e.to_string())?
                .as_ref()
                .filter(|c| {
                    c.ctx == ctx && c.quick == quick && c.built.elapsed() < CATALOG_REUSE_WINDOW
                })
                .map(|c| c.catalog.clone())
        } else {
            None
        };
        let catalog = match reusable {
            Some(catalog) => {
                progress(&worker, &id, "analyzing", 0, 0);
                catalog
            }
            None => {
                let built = Instant::now();
                let catalog = Arc::new(cleanup::build_catalog(
                    &db,
                    &ctx,
                    quick,
                    &cancel,
                    &mut |stage, n, t| progress(&worker, &id, stage, n, t),
                )?);
                *cleanup_state.catalog.lock().map_err(|e| e.to_string())? = Some(CachedCatalog {
                    ctx: ctx.clone(),
                    quick,
                    built,
                    catalog: catalog.clone(),
                });
                remember_health(&worker, &ctx, &catalog);
                catalog
            }
        };
        cleanup::make_plan_with_removal_choice(
            &db,
            ctx,
            &catalog,
            id.clone(),
            package_ids,
            include_dependencies,
            excluded_package_ids,
            &force_package_ids.unwrap_or_default(),
            &cancel,
        )
    })
    .await
    .map_err(|e| e.to_string())
    .and_then(|r| r);
    match result {
        Ok(plan) => {
            let preview = plan.preview.clone();
            let mut plans = state.plans.lock().map_err(|e| e.to_string())?;
            let mut operations = state.operations.lock().map_err(|e| e.to_string())?;
            if operations
                .get(&request_id)
                .map_or(true, |cancel| cancel.load(Ordering::Relaxed))
            {
                operations.remove(&request_id);
                return Err("操作已取消".into());
            }
            if plans.len() >= 8 {
                operations.remove(&request_id);
                return Err("请关闭旧清理预览后重试".into());
            }
            plans.insert(request_id, plan);
            Ok(preview)
        }
        Err(e) => {
            state
                .operations
                .lock()
                .map_err(|e| e.to_string())?
                .remove(&request_id);
            Err(e)
        }
    }
}
/// Problem packages and packages nothing uses, from the latest full check.
/// Runs a check when there is none yet, or when resources changed and no
/// background check is going to replace the old result.
#[tauri::command]
pub async fn get_cleanup_health(
    app: tauri::AppHandle,
    vam_root: String,
) -> Result<Arc<HealthReport>, String> {
    let ctx = expected_context(&app, &vam_root)?;
    let state = app.state::<CleanupState>();
    let background = cleanup::policy(&app.state::<Database>())?.background_index;
    if let Some(stored) = state.health.lock().map_err(|e| e.to_string())?.as_ref() {
        if stored.library_key == ctx.library_key && (!stored.stale || background) {
            return Ok(stored.report.clone());
        }
    }
    let id = format!("health-{}", chrono::Utc::now().timestamp_nanos_opt().unwrap_or_default());
    let cancel = Arc::new(AtomicBool::new(false));
    {
        let mut ops = state.operations.lock().map_err(|e| e.to_string())?;
        if ops.len() >= 8 {
            return Err("清理分析任务过多，请稍后再试".into());
        }
        ops.insert(id.clone(), cancel.clone());
    }
    preempt_indexer(&app);
    let worker = app.clone();
    let result = tauri::async_runtime::spawn_blocking(move || {
        let db = worker.state::<Database>();
        let catalog = cleanup::build_catalog(&db, &ctx, false, &cancel, &mut |_, _, _| {})?;
        Ok::<_, String>(remember_health(&worker, &ctx, &catalog))
    })
    .await
    .map_err(|e| e.to_string())
    .and_then(|r| r);
    state
        .operations
        .lock()
        .map_err(|e| e.to_string())?
        .remove(&id);
    result
}
#[tauri::command]
pub async fn cancel_resource_cleanup(
    app: tauri::AppHandle,
    request_id: String,
) -> Result<(), String> {
    let state = app.state::<CleanupState>();
    if let Some(cancel) = state
        .operations
        .lock()
        .map_err(|e| e.to_string())?
        .get(&request_id)
    {
        cancel.store(true, Ordering::Relaxed);
    }
    if state
        .plans
        .lock()
        .map_err(|e| e.to_string())?
        .remove(&request_id)
        .is_some()
    {
        state
            .operations
            .lock()
            .map_err(|e| e.to_string())?
            .remove(&request_id);
    }
    Ok(())
}
#[tauri::command]
pub async fn execute_resource_cleanup(
    app: tauri::AppHandle,
    plan_id: String,
    confirm_risks: Option<bool>,
) -> Result<Batch, String> {
    let state = app.state::<CleanupState>();
    let _execution = execution_guard(&app)?;
    let confirm_risks = confirm_risks.unwrap_or(false);
    let plan = {
        let mut plans = state.plans.lock().map_err(|e| e.to_string())?;
        let plan = plans.get(&plan_id).ok_or("清理预览已失效，请重新分析")?;
        if plan.preview.requires_confirmation && !confirm_risks {
            return Err("请先确认清理对其他资源的影响".into());
        }
        plans.remove(&plan_id).unwrap()
    };
    let cancel = state
        .operations
        .lock()
        .map_err(|e| e.to_string())?
        .get(&plan_id)
        .cloned()
        .ok_or("清理任务已取消")?;
    invalidate_catalog(&app);
    let worker = app.clone();
    let id = plan_id.clone();
    let result = tauri::async_runtime::spawn_blocking(move || {
        if context(&worker)? != plan.ctx {
            return Err("游戏实例或资源目录已变化，请重新预览".into());
        }
        scene_launch::ensure_game_stopped(std::path::Path::new(&plan.ctx.vam_root))?;
        let db = worker.state::<Database>();
        let batch = cleanup::execute_with_progress(
            &db,
            &data(&worker)?,
            plan,
            confirm_risks,
            &cancel,
            &mut |stage, n, t| progress(&worker, &id, stage, n, t),
        )?;
        changed(&worker, &batch);
        Ok(batch.for_display())
    })
    .await
    .map_err(|e| e.to_string())
    .and_then(|r| r);
    state
        .operations
        .lock()
        .map_err(|e| e.to_string())?
        .remove(&plan_id);
    result
}
#[tauri::command]
pub async fn list_resource_cleanup_batches(
    app: tauri::AppHandle,
    vam_root: String,
) -> Result<Vec<Batch>, String> {
    let ctx = expected_context(&app, &vam_root)?;
    tauri::async_runtime::spawn_blocking(move || cleanup::list_batches(&data(&app)?, &ctx)
        .map(|batches| batches.into_iter().map(Batch::for_display).collect()))
        .await
        .map_err(|e| e.to_string())?
}
#[tauri::command]
pub async fn restore_resource_cleanup_batch(
    app: tauri::AppHandle,
    vam_root: String,
    batch_id: String,
) -> Result<Batch, String> {
    let _execution = execution_guard(&app)?;
    let ctx = expected_context(&app, &vam_root)?;
    invalidate_catalog(&app);
    tauri::async_runtime::spawn_blocking(move || {
        scene_launch::ensure_game_stopped(std::path::Path::new(&ctx.vam_root))?;
        let batch = cleanup::restore(&app.state::<Database>(), &data(&app)?, &ctx, &batch_id)?;
        changed(&app, &batch);
        Ok(batch.for_display())
    })
    .await
    .map_err(|e| e.to_string())?
}
#[tauri::command]
pub async fn delete_resource_cleanup_batch(
    app: tauri::AppHandle,
    vam_root: String,
    batch_id: String,
) -> Result<Batch, String> {
    let _execution = execution_guard(&app)?;
    let ctx = expected_context(&app, &vam_root)?;
    tauri::async_runtime::spawn_blocking(move || cleanup::purge_with_progress(&data(&app)?, &ctx, &batch_id,
        &mut |stage, current, total| progress(&app, &batch_id, stage, current, total)).map(Batch::for_display))
        .await
        .map_err(|e| e.to_string())?
}
#[tauri::command]
pub async fn extend_resource_cleanup_batch(
    app: tauri::AppHandle,
    vam_root: String,
    batch_id: String,
    days: i64,
) -> Result<Batch, String> {
    let ctx = expected_context(&app, &vam_root)?;
    tauri::async_runtime::spawn_blocking(move || {
        cleanup::extend(&data(&app)?, &ctx, &batch_id, days).map(Batch::for_display)
    })
    .await
    .map_err(|e| e.to_string())?
}
pub async fn quick_delete(
    app: tauri::AppHandle,
    package_id: String,
    include_dependencies: bool,
) -> Result<crate::commands::packages::QuickDeleteResult, String> {
    let _execution = execution_guard(&app)?;
    let ctx = context(&app)?;
    invalidate_catalog(&app);
    tauri::async_runtime::spawn_blocking(move || {
        scene_launch::ensure_game_stopped(std::path::Path::new(&ctx.vam_root))?;
        let db = app.state::<Database>();
        let cancel = AtomicBool::new(false);
        let id = format!(
            "quick-{}-{}",
            std::process::id(),
            chrono::Utc::now().timestamp_nanos_opt().unwrap_or_default()
        );
        let plan = cleanup::make_plan(
            &db,
            ctx,
            id,
            vec![package_id],
            include_dependencies,
            vec![],
            &cancel,
            &mut |_, _| {},
        )?;
        if !plan.preview.blocked.is_empty() {
            return Err("资源包受到保护、仍被引用或检查不完整，请在资源清理页查看预览".into());
        }
        let skipped_package_ids = plan
            .preview
            .retained
            .iter()
            .map(|p| p.package_id.clone())
            .collect::<Vec<_>>();
        let deleted_package_ids = plan
            .preview
            .items
            .iter()
            .map(|p| p.package_id.clone())
            .collect::<Vec<_>>();
        let deleted_file_count = plan.preview.total_files;
        let freed_bytes = plan.preview.total_bytes;
        let batch = cleanup::execute(&db, &data(&app)?, plan, &cancel, &mut |_, _| {})?;
        changed(&app, &batch);
        if batch.status != "completed" {
            return Err(batch.errors.join("; "));
        }
        Ok(crate::commands::packages::QuickDeleteResult {
            deleted_count: deleted_package_ids.len(),
            skipped_count: skipped_package_ids.len(),
            deleted_package_ids,
            skipped_package_ids,
            deleted_file_count,
            freed_bytes,
        })
    })
    .await
    .map_err(|e| e.to_string())?
}
#[tauri::command]
pub async fn get_cleanup_policy(db: State<'_, Database>) -> Result<Policy, String> {
    cleanup::policy(&db)
}
#[tauri::command]
pub async fn save_cleanup_policy(
    app: tauri::AppHandle,
    db: State<'_, Database>,
    retention_days: i64,
    auto_purge: bool,
    background_index: Option<bool>,
) -> Result<Policy, String> {
    match background_index {
        Some(enabled) => {
            cleanup::save_policy_with_index(&db, retention_days, auto_purge, enabled)?;
            if !enabled {
                preempt_indexer(&app);
            }
            // Starts indexing when enabled; reports "disabled" otherwise.
            schedule_index_after(&app, Duration::from_secs(1));
        }
        None => cleanup::save_policy(&db, retention_days, auto_purge)?,
    }
    cleanup::policy(&db)
}

pub fn startup_maintenance(app: tauri::AppHandle) {
    tauri::async_runtime::spawn_blocking(move || {
        let Ok(ctx) = context(&app) else {
            return;
        };
        let result = (|| {
            let _execution = execution_guard(&app)?;
            let db = app.state::<Database>();
            let data = data(&app)?;
            cleanup::reconcile(&db, &data, &ctx)?;
            cleanup::auto_purge(&db, &data, &ctx)?;
            Ok::<_, String>(())
        })();
        if let Err(e) = result {
            log::warn!("清理记录维护未完成: {e}");
        }
        // Bring the reference index up to date once startup has settled.
        schedule_index_after(&app, Duration::from_secs(60));
    });
}
