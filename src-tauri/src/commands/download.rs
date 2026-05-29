use serde::Serialize;
use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex, OnceLock};
use tauri::{Emitter, Manager, State};

use crate::commands::hub::{
    fetch_hub_package_info, fetch_hub_package_info_basic, HubApiResponse, HubDependency,
};
use crate::services::downloader::{
    get_installed_version, parse_package_info, AddResult, DownloadItem, DownloadManager,
    DownloadStatus,
};

const MIN_CONCURRENT_DOWNLOADS: usize = 1;
const MAX_CONCURRENT_DOWNLOADS: usize = 5;

#[tauri::command]
pub async fn get_download_queue(
    manager: State<'_, Arc<DownloadManager>>,
) -> Result<Vec<DownloadItem>, String> {
    let queue = manager.queue.lock().unwrap();
    Ok(queue.clone())
}

#[tauri::command]
pub async fn add_to_download_queue(
    manager: State<'_, Arc<DownloadManager>>,
    items: Vec<DownloadItem>,
) -> Result<Vec<AddResult>, String> {
    let mut results = Vec::new();
    for item in items {
        let res = manager.add_item_with_dedup(item);
        results.push(res);
    }
    Ok(results)
}

#[tauri::command]
pub async fn pause_download(
    manager: State<'_, Arc<DownloadManager>>,
    id: String,
) -> Result<(), String> {
    // 1. Set status to Paused in queue
    {
        let mut queue = manager.queue.lock().unwrap();
        if let Some(item) = queue.iter_mut().find(|i| i.id == id) {
            if item.status == DownloadStatus::Downloading || item.status == DownloadStatus::Pending
            {
                item.status = DownloadStatus::Paused;
                item.speed_bytes_per_sec = 0;
            }
        }
    }

    // 2. Trigger cancellation of tokio task if it is active
    {
        let mut active = manager.active_downloads.lock().unwrap();
        if let Some(cancel_tx) = active.remove(&id) {
            let _ = cancel_tx.send(());
        }
    }

    manager.save_queue_to_disk();
    manager.emit_queue_updated();
    Ok(())
}

#[tauri::command]
pub async fn resume_download(
    manager: State<'_, Arc<DownloadManager>>,
    id: String,
) -> Result<(), String> {
    {
        let mut queue = manager.queue.lock().unwrap();
        if let Some(item) = queue.iter_mut().find(|i| i.id == id) {
            if item.status == DownloadStatus::Paused || item.status == DownloadStatus::Failed {
                item.status = DownloadStatus::Pending;
                item.error_msg = None;
            }
        }
    }

    manager.save_queue_to_disk();
    manager.emit_queue_updated();
    Ok(())
}

#[tauri::command]
pub async fn cancel_download(
    manager: State<'_, Arc<DownloadManager>>,
    id: String,
) -> Result<(), String> {
    // 1. If currently downloading, trigger cancel
    {
        let mut active = manager.active_downloads.lock().unwrap();
        if let Some(cancel_tx) = active.remove(&id) {
            let _ = cancel_tx.send(());
        }
    }

    // 2. Remove from queue
    {
        let mut queue = manager.queue.lock().unwrap();
        queue.retain(|i| i.id != id);
    }

    manager.save_queue_to_disk();
    manager.emit_queue_updated();
    Ok(())
}

#[tauri::command]
pub async fn retry_download(
    manager: State<'_, Arc<DownloadManager>>,
    id: String,
) -> Result<(), String> {
    {
        let mut queue = manager.queue.lock().unwrap();
        if let Some(item) = queue.iter_mut().find(|i| i.id == id) {
            item.status = DownloadStatus::Pending;
            item.progress = 0.0;
            item.downloaded_bytes = 0;
            item.error_msg = None;
        }
    }

    manager.save_queue_to_disk();
    manager.emit_queue_updated();
    Ok(())
}

#[tauri::command]
pub async fn clear_completed_downloads(
    manager: State<'_, Arc<DownloadManager>>,
) -> Result<(), String> {
    {
        let mut queue = manager.queue.lock().unwrap();
        queue.retain(|i| i.status != DownloadStatus::Completed);
    }

    manager.save_queue_to_disk();
    manager.emit_queue_updated();
    Ok(())
}

#[tauri::command]
pub async fn set_download_settings(
    manager: State<'_, Arc<DownloadManager>>,
    app_handle: tauri::AppHandle,
    max_concurrent: usize,
    speed_limit_kb: u64,
) -> Result<(), String> {
    let max_concurrent = max_concurrent.clamp(MIN_CONCURRENT_DOWNLOADS, MAX_CONCURRENT_DOWNLOADS);
    {
        let mut mc = manager.max_concurrent.lock().unwrap();
        *mc = max_concurrent;
    }
    {
        let mut sl = manager.speed_limit_kb.lock().unwrap();
        *sl = speed_limit_kb;
    }

    // Save to settings.json
    if let Ok(app_data_dir) = app_handle.path().app_data_dir() {
        let settings_path = app_data_dir.join("settings.json");
        let mut settings = if settings_path.exists() {
            let contents = std::fs::read_to_string(&settings_path).unwrap_or_default();
            serde_json::from_str::<serde_json::Value>(&contents).unwrap_or(serde_json::json!({}))
        } else {
            serde_json::json!({})
        };

        if let Some(obj) = settings.as_object_mut() {
            obj.insert(
                "max_concurrent_downloads".to_string(),
                serde_json::Value::Number(max_concurrent.into()),
            );
            obj.insert(
                "speed_limit_kb".to_string(),
                serde_json::Value::Number(speed_limit_kb.into()),
            );
        }

        if let Ok(contents) = serde_json::to_string_pretty(&settings) {
            let _ = std::fs::write(settings_path, contents);
        }
    }

    Ok(())
}

#[derive(Serialize, Clone, Debug)]
pub struct ResolvedDependency {
    pub id: String, // "Creator.Name.Version"
    pub filename: String,
    pub creator: String,
    pub name: String,
    pub version: i32,
    pub file_size: String,
    pub download_url: Option<String>,
    pub status: String, // "installed" | "queued" | "need_download" | "no_source" | "upgraded"
    pub installed_version: Option<i32>,
    pub queued_version: Option<i32>,
}

#[derive(Serialize, Clone, Debug)]
pub struct DependencyResolutionResult {
    pub main_package_id: String,
    pub main_package_filename: String,
    pub total_size_bytes: u64,
    pub dependencies: Vec<ResolvedDependency>,
}

fn hub_dependency_identifier(dep: &HubDependency) -> Option<String> {
    dep.filename
        .clone()
        .or_else(|| dep.package_name.clone())
        .map(|value| value.trim_end_matches(".var").to_string())
}

fn hub_dependency_required_version(dep: &HubDependency, parsed_version: i32) -> i32 {
    dep.latest_version
        .as_deref()
        .and_then(|version| version.parse::<i32>().ok())
        .or_else(|| {
            dep.version
                .as_deref()
                .and_then(|version| version.parse::<i32>().ok())
        })
        .unwrap_or(parsed_version)
}

#[derive(Clone, Serialize)]
struct ResolveProgressPayload {
    resolved_count: usize,
    total_discovered: usize,
    current_package: String,
}

static CANCELLED_RESOLUTIONS: OnceLock<Mutex<HashSet<String>>> = OnceLock::new();

fn get_cancelled_resolutions() -> &'static Mutex<HashSet<String>> {
    CANCELLED_RESOLUTIONS.get_or_init(|| Mutex::new(HashSet::new()))
}

#[tauri::command]
pub fn cancel_hub_dependency_resolution(package_name: String) {
    get_cancelled_resolutions()
        .lock()
        .unwrap()
        .insert(package_name);
}

/// Recursively resolves VAM Hub dependencies and performs local & queue version comparisons.
#[tauri::command]
pub async fn resolve_hub_dependencies(
    manager: State<'_, Arc<DownloadManager>>,
    app_handle: tauri::AppHandle,
    package_name: String,
) -> Result<DependencyResolutionResult, String> {
    // Clear any previous cancellation flag for this package
    get_cancelled_resolutions()
        .lock()
        .unwrap()
        .remove(&package_name);

    let _ = app_handle.emit(
        "resolve-dependencies-progress",
        ResolveProgressPayload {
            resolved_count: 0,
            total_discovered: 1,
            current_package: package_name.clone(),
        },
    );
    let mut current_layer = vec![package_name.clone()];
    let mut resolved_packages = HashMap::new(); // package_name -> HubApiResponse
    let mut all_deps: HashMap<String, HubDependency> = HashMap::new(); // Creator.Name -> HubDependency

    let mut visited = HashSet::new();
    let mut depth = 0;
    let max_depth = 3;

    // 1. Parallel Recursive API fetch BFS loop
    while !current_layer.is_empty() && depth < max_depth {
        if get_cancelled_resolutions()
            .lock()
            .unwrap()
            .contains(&package_name)
        {
            get_cancelled_resolutions()
                .lock()
                .unwrap()
                .remove(&package_name);
            return Err("CANCELLED".to_string());
        }
        depth += 1;

        let mut to_fetch = Vec::new();
        for item in current_layer {
            if !visited.contains(&item) {
                visited.insert(item.clone());
                to_fetch.push(item);
            }
        }

        if to_fetch.is_empty() {
            break;
        }

        let should_enrich_dependencies = depth == 1;
        let mut results = Vec::new();
        for chunk in to_fetch.chunks(15) {
            if get_cancelled_resolutions()
                .lock()
                .unwrap()
                .contains(&package_name)
            {
                get_cancelled_resolutions()
                    .lock()
                    .unwrap()
                    .remove(&package_name);
                return Err("CANCELLED".to_string());
            }
            let fetch_futures = chunk.iter().map(|pkg| {
                let pkg_clone = (*pkg).clone();
                async move {
                    let fetch_result = if should_enrich_dependencies {
                        fetch_hub_package_info(pkg_clone.clone()).await
                    } else {
                        fetch_hub_package_info_basic(pkg_clone.clone()).await
                    };

                    match fetch_result {
                        Ok(info) => Some((pkg_clone, info)),
                        Err(e) => {
                            log::warn!(
                                "Failed to fetch dependency details for {}: {}",
                                pkg_clone,
                                e
                            );
                            None
                        }
                    }
                }
            });
            let mut chunk_results = futures::future::join_all(fetch_futures).await;
            // Insert chunk results into resolved_packages immediately so we can report progress!
            for res in &chunk_results {
                if let Some((pkg, info)) = res {
                    resolved_packages.insert(pkg.clone(), info.clone());
                }
            }

            let resolved_count = resolved_packages.len();
            let total_discovered = visited.len();
            let current_package = chunk.last().cloned().unwrap_or_default();

            let _ = app_handle.emit(
                "resolve-dependencies-progress",
                ResolveProgressPayload {
                    resolved_count,
                    total_discovered,
                    current_package,
                },
            );

            results.append(&mut chunk_results);
        }

        let mut next_layer = Vec::new();

        for res in results {
            if let Some((pkg, info)) = res {
                resolved_packages.insert(pkg, info.clone());

                // Parse and resolve dependencies from this package
                if let Some(deps_map) = info.dependencies {
                    for (_, list) in deps_map {
                        for dep in list {
                            let Some(dep_id) = hub_dependency_identifier(&dep) else {
                                continue;
                            };

                            if let Some(parsed) = parse_package_info(&dep_id) {
                                let key = format!("{}.{}", parsed.creator, parsed.name);
                                let required_version =
                                    hub_dependency_required_version(&dep, parsed.version);

                                // Keep highest version of this package requested
                                let mut should_add = true;
                                if let Some(existing) = all_deps.get(&key) {
                                    let existing_id = hub_dependency_identifier(existing)
                                        .unwrap_or_else(|| key.clone());
                                    if let Some(ex_parsed) = parse_package_info(&existing_id) {
                                        let existing_version = hub_dependency_required_version(
                                            existing,
                                            ex_parsed.version,
                                        );
                                        if existing_version >= required_version {
                                            should_add = false;
                                        }
                                    }
                                }

                                if should_add {
                                    all_deps.insert(key, dep.clone());

                                    // Skip resolving transitive dependencies for packages already
                                    // installed locally with sufficient version — saves API calls.
                                    let installed_ver = get_installed_version(
                                        &app_handle,
                                        &parsed.creator,
                                        &parsed.name,
                                    );
                                    let already_satisfied = installed_ver
                                        .map(|v| v >= required_version)
                                        .unwrap_or(false);

                                    if !already_satisfied {
                                        // Queue this dependency to resolve *its* transitive dependencies in the next layer
                                        if let Some(ref pn) = dep.package_name {
                                            next_layer.push(format!("{}.latest", pn));
                                        } else if let Some(ref fn_str) = dep.filename {
                                            next_layer.push(fn_str.clone());
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }

        current_layer = next_layer;

        let resolved_count = resolved_packages.len();
        let total_discovered = visited.len();
        let _ = app_handle.emit(
            "resolve-dependencies-progress",
            ResolveProgressPayload {
                resolved_count,
                total_discovered,
                current_package: "".to_string(),
            },
        );
    }

    // Get info of the main package itself from resolved_packages, or fetch it if missing
    let main_info_opt = resolved_packages.get(&package_name).cloned();
    let main_info = match main_info_opt {
        Some(info) => info,
        None => fetch_hub_package_info(package_name.clone())
            .await
            .unwrap_or_else(|_| HubApiResponse {
                title: Some(package_name.clone()),
                tag_line: None,
                username: None,
                version_string: None,
                description: None,
                download_count: None,
                rating_avg: None,
                image_url: None,
                icon_url: None,
                resource_id: None,
                category: None,
                view_count: None,
                rating_count: None,
                dependency_count: None,
                hub_files: None,
                dependencies: None,
            }),
    };

    let main_filename = if let Some(ref files) = main_info.hub_files {
        files
            .first()
            .and_then(|f| f.filename.clone())
            .unwrap_or(format!("{}.var", package_name))
    } else {
        format!("{}.var", package_name)
    };

    // 2. Perform version比对 & status mapping
    let mut resolved_list = Vec::new();
    let queue = manager.queue.lock().unwrap();

    for (_, dep) in all_deps {
        let Some(identifier) = hub_dependency_identifier(&dep) else {
            continue;
        };

        let parsed = match parse_package_info(&identifier) {
            Some(p) => p,
            None => continue,
        };
        let required_version = hub_dependency_required_version(&dep, parsed.version);

        let filename = dep.filename.clone().unwrap_or(format!(
            "{}.{}.{}.var",
            parsed.creator, parsed.name, required_version
        ));

        // Check installed version in DB
        let installed_ver = get_installed_version(&app_handle, &parsed.creator, &parsed.name);

        // Check queued version in Download Queue
        let mut queued_ver = None;
        for q in queue.iter() {
            if let Some(qp) = parse_package_info(&q.id) {
                if qp.creator.to_lowercase() == parsed.creator.to_lowercase()
                    && qp.name.to_lowercase() == parsed.name.to_lowercase()
                {
                    queued_ver = Some(qp.version);
                    break;
                }
            }
        }

        let status = if let Some(inst_v) = installed_ver {
            if inst_v >= required_version {
                "installed".to_string()
            } else if let Some(q_v) = queued_ver {
                if q_v >= required_version {
                    "queued".to_string()
                } else {
                    "upgraded".to_string() // queued but needs upgrade
                }
            } else {
                // 本地版本低于需求，标记为需要升级
                "upgraded".to_string()
            }
        } else if let Some(q_v) = queued_ver {
            if q_v >= required_version {
                "queued".to_string()
            } else {
                "upgraded".to_string()
            }
        } else {
            if dep.download_url.is_none() {
                "no_source".to_string()
            } else {
                "need_download".to_string()
            }
        };

        resolved_list.push(ResolvedDependency {
            id: format!("{}.{}.{}", parsed.creator, parsed.name, required_version),
            filename,
            creator: parsed.creator,
            name: parsed.name,
            version: required_version,
            file_size: dep.file_size.unwrap_or("未知".to_string()),
            download_url: dep.download_url,
            status,
            installed_version: installed_ver,
            queued_version: queued_ver,
        });
    }

    Ok(DependencyResolutionResult {
        main_package_id: package_name,
        main_package_filename: main_filename,
        total_size_bytes: 0, // Placeholder, calculated client-side or omitted
        dependencies: resolved_list,
    })
}
