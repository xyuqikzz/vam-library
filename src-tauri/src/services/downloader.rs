use regex::Regex;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use tauri::{Emitter, Manager};
use tokio::fs::File;
use tokio::io::AsyncWriteExt;

use crate::db::Database;
use crate::services::install_context::resolve_install_context;
use crate::services::var_parser;

const VAM_HUB_CONSENT_COOKIE: &str = "vamhubconsent=yes";
const MIN_CONCURRENT_DOWNLOADS: usize = 1;
const MAX_CONCURRENT_DOWNLOADS: usize = 5;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DownloadStatus {
    Pending,
    Downloading,
    Paused,
    Completed,
    Failed,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DownloadItem {
    pub id: String,  // Unique identifier, normally "Creator.Package.Version" or filename
    pub url: String, // Download URL
    pub filename: String, // Local target filename
    pub creator: String, // Creator name
    pub name: String, // Package name
    pub version: i32, // Package version as integer
    pub total_bytes: u64, // File size
    pub downloaded_bytes: u64, // Size currently downloaded
    pub progress: f64, // Progress percentage (0.0 to 100.0)
    pub speed_bytes_per_sec: u64, // Smoothed network transfer rate
    pub status: DownloadStatus, // Current task status
    pub error_msg: Option<String>, // Error message if failed
    pub added_at: String, // Added timestamp (RFC3339)
    #[serde(default)]
    pub final_path: Option<String>, // 最终安装路径
    #[serde(default)]
    pub temp_path: Option<String>, // 临时下载路径
    #[serde(default)]
    pub install_mode: Option<String>, // real_addon | managed_library
    #[serde(default)]
    pub indexed: bool, // 下载完成后是否已写入本地索引
    #[serde(default)]
    pub warning_msg: Option<String>, // 已保存文件的元数据/入库警告
}

#[derive(Debug, Clone)]
pub struct ParsedPackage {
    pub creator: String,
    pub name: String,
    pub version: i32,
}

/// Parses VAM package identifier or filename into Creator, Name and Version.
/// Expected format: Creator.Name.Version.var or Creator.Name.Version
pub fn parse_package_info(identifier: &str) -> Option<ParsedPackage> {
    let clean = identifier.strip_suffix(".var").unwrap_or(identifier);
    let parts: Vec<&str> = clean.split('.').collect();
    if parts.len() < 2 {
        return None;
    }
    let creator = parts[0].to_string();
    if parts.len() == 2 {
        let name = parts[1].to_string();
        return Some(ParsedPackage {
            creator,
            name,
            version: 1,
        });
    }
    let version_str = parts.last().unwrap();
    let version = version_str.parse::<i32>().unwrap_or(1);
    let name = parts[1..parts.len() - 1].join(".");

    Some(ParsedPackage {
        creator,
        name,
        version,
    })
}

/// Queries the local rusqlite database to see if a package is already installed.
pub fn get_installed_version(app: &tauri::AppHandle, creator: &str, name: &str) -> Option<i32> {
    let db = app.state::<Database>();
    db.with_conn(|conn| {
        let mut stmt = conn.prepare(
            "SELECT version FROM packages WHERE LOWER(creator) = LOWER(?1) AND LOWER(name) = LOWER(?2) ORDER BY version DESC LIMIT 1"
        ).map_err(|e| crate::errors::AppError::Database(e.to_string()))?;

        let mut rows = stmt.query([creator, name]).map_err(|e| crate::errors::AppError::Database(e.to_string()))?;
        if let Some(row) = rows.next().map_err(|e| crate::errors::AppError::Database(e.to_string()))? {
            let version: i32 = row.get(0).map_err(|e| crate::errors::AppError::Database(e.to_string()))?;
            Ok(Some(version))
        } else {
            Ok(None)
        }
    }).unwrap_or(None)
}

/// Load VAM root path from settings.json
pub fn get_vam_root_path(app_handle: &tauri::AppHandle) -> Option<String> {
    let app_data_dir = app_handle.path().app_data_dir().ok()?;
    let settings_path = app_data_dir.join("settings.json");
    if !settings_path.exists() {
        return None;
    }
    let contents = std::fs::read_to_string(settings_path).ok()?;
    let settings: serde_json::Value = serde_json::from_str(&contents).ok()?;
    settings
        .get("vam_root_path")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
}

fn normalize_var_filename(item: &mut DownloadItem) {
    if !item.filename.to_lowercase().ends_with(".var") {
        item.filename = if item.id.to_lowercase().ends_with(".var") {
            item.id.clone()
        } else {
            format!("{}.var", item.id)
        };
    }
}

pub fn validate_var_file(path: &Path) -> Result<(), String> {
    let file = std::fs::File::open(path).map_err(|e| format!("无法校验下载文件: {}", e))?;
    let mut archive = zip::ZipArchive::new(file).map_err(|e| {
        format!(
            "下载文件不是完整的 VAR/ZIP 包（可能是网页或未下载完整）: {}",
            e
        )
    })?;
    for index in 0..archive.len() {
        let mut entry = archive
            .by_index(index)
            .map_err(|e| format!("读取包目录失败: {}", e))?;
        if entry.is_file() {
            std::io::copy(&mut entry, &mut std::io::sink())
                .map_err(|e| format!("包内容校验失败（CRC/解压错误）: {}", e))?;
        }
    }
    Ok(())
}

fn extract_direct_download_url(html: &str) -> Option<String> {
    let normalized = html.replace("\\/", "/").replace("&amp;", "&");

    let full_url_re = Regex::new(
        r#"https?://[^"'<>\s]+/internal_data/attachments/[^"'<>\s]+\.data(?:\?[^"'<>\s]*)?"#,
    )
    .ok()?;
    if let Some(found) = full_url_re.find(&normalized) {
        return Some(found.as_str().to_string());
    }

    let relative_url_re =
        Regex::new(r#"/internal_data/attachments/[^"'<>\s]+\.data(?:\?[^"'<>\s]*)?"#).ok()?;
    relative_url_re
        .find(&normalized)
        .map(|found| format!("https://hub.virtamate.com{}", found.as_str()))
}

fn hub_cookie_header(app_handle: &tauri::AppHandle) -> String {
    let mut cookies = vec![VAM_HUB_CONSENT_COOKIE.to_string()];
    if let Ok(app_data_dir) = app_handle.path().app_data_dir() {
        let settings_path = app_data_dir.join("settings.json");
        if let Ok(contents) = std::fs::read_to_string(settings_path) {
            if let Ok(settings) = serde_json::from_str::<serde_json::Value>(&contents) {
                if let Some(cookie) = settings
                    .get("hub_auth_cookie")
                    .and_then(|value| value.as_str())
                    .map(|value| value.trim())
                    .filter(|value| !value.is_empty())
                {
                    cookies.push(cookie.to_string());
                }
            }
        }
    }
    cookies.join("; ")
}

#[derive(Debug, Serialize, Deserialize)]
pub enum AddResult {
    Added,
    Skipped(String),
    Upgraded(i32, i32), // (OldVersion, NewVersion)
}

pub struct DownloadManager {
    pub queue: Arc<Mutex<Vec<DownloadItem>>>,
    pub active_downloads: Arc<Mutex<HashMap<String, Option<tokio::sync::oneshot::Sender<()>>>>>,
    pub app_handle: Option<tauri::AppHandle>,
    pub max_concurrent: Arc<Mutex<usize>>,
    pub speed_limit_kb: Arc<Mutex<u64>>,
}

impl DownloadManager {
    pub fn new(app_handle: tauri::AppHandle) -> Self {
        // Load initial download settings from settings.json if available
        let mut max_concurrent = 1;
        let mut speed_limit_kb = 0;

        if let Ok(app_data_dir) = app_handle.path().app_data_dir() {
            let settings_path = app_data_dir.join("settings.json");
            if settings_path.exists() {
                if let Ok(contents) = std::fs::read_to_string(settings_path) {
                    if let Ok(settings) = serde_json::from_str::<serde_json::Value>(&contents) {
                        if let Some(mc) = settings
                            .get("max_concurrent_downloads")
                            .and_then(|v| v.as_u64())
                        {
                            max_concurrent = (mc as usize)
                                .clamp(MIN_CONCURRENT_DOWNLOADS, MAX_CONCURRENT_DOWNLOADS);
                        }
                        if let Some(sl) = settings.get("speed_limit_kb").and_then(|v| v.as_u64()) {
                            speed_limit_kb = sl;
                        }
                    }
                }
            }
        }

        let loaded_queue = Self::load_queue_from_disk(&app_handle);

        Self {
            queue: Arc::new(Mutex::new(loaded_queue)),
            active_downloads: Arc::new(Mutex::new(HashMap::new())),
            app_handle: Some(app_handle),
            max_concurrent: Arc::new(Mutex::new(max_concurrent)),
            speed_limit_kb: Arc::new(Mutex::new(speed_limit_kb)),
        }
    }

    pub fn save_queue_to_disk(&self) {
        if let Some(ref app_handle) = self.app_handle {
            if let Ok(app_data_dir) = app_handle.path().app_data_dir() {
                let path = app_data_dir.join("downloads.json");
                let queue = self.queue.lock().unwrap();
                if let Ok(contents) = serde_json::to_string_pretty(&*queue) {
                    let _ = std::fs::write(path, contents);
                }
            }
        }
    }

    pub fn load_queue_from_disk(app_handle: &tauri::AppHandle) -> Vec<DownloadItem> {
        if let Ok(app_data_dir) = app_handle.path().app_data_dir() {
            let path = app_data_dir.join("downloads.json");
            if path.exists() {
                if let Ok(contents) = std::fs::read_to_string(path) {
                    if let Ok(mut queue) = serde_json::from_str::<Vec<DownloadItem>>(&contents) {
                        // Reset items stuck in Downloading/Failed-middle to Pending
                        for item in queue.iter_mut() {
                            if item.status == DownloadStatus::Downloading {
                                item.status = DownloadStatus::Pending;
                                item.speed_bytes_per_sec = 0;
                            }
                        }
                        return queue;
                    }
                }
            }
        }
        Vec::new()
    }

    pub fn emit_queue_updated(&self) {
        if let Some(ref app_handle) = self.app_handle {
            let _ = app_handle.emit("download-queue-updated", ());
        }
    }

    /// 添加下载任务，并执行版本去重检查。
    pub fn add_item_with_dedup(&self, mut item: DownloadItem) -> AddResult {
        let app = match &self.app_handle {
            Some(a) => a,
            None => return AddResult::Added,
        };

        let parsed = match parse_package_info(&item.id) {
            Some(p) => p,
            None => {
                // 解析失败时仍按普通任务入队。
                {
                    let mut queue = self.queue.lock().unwrap();
                    queue.push(item);
                }
                self.save_queue_to_disk();
                self.emit_queue_updated();
                return AddResult::Added;
            }
        };

        normalize_var_filename(&mut item);

        // 1. 检查本地数据库。
        if let Some(installed_ver) = get_installed_version(app, &parsed.creator, &parsed.name) {
            if installed_ver >= parsed.version {
                return AddResult::Skipped(format!(
                    "本地已安装相同或更高版本 (v{})",
                    installed_ver
                ));
            }
        }

        // 2. 检查下载队列。
        let result = {
            let mut queue = self.queue.lock().unwrap();
            let mut existing_index = None;
            let mut existing_version = 0;
            let mut existing_status = DownloadStatus::Pending;

            for (i, q_item) in queue.iter().enumerate() {
                if let Some(q_parsed) = parse_package_info(&q_item.id) {
                    if q_parsed.creator.to_lowercase() == parsed.creator.to_lowercase()
                        && q_parsed.name.to_lowercase() == parsed.name.to_lowercase()
                    {
                        existing_index = Some(i);
                        existing_version = q_parsed.version;
                        existing_status = q_item.status;
                        break;
                    }
                }
            }

            if let Some(idx) = existing_index {
                if existing_version >= parsed.version {
                    return AddResult::Skipped(format!(
                        "下载队列已存在同等或更高版本 (v{})",
                        existing_version
                    ));
                } else {
                    // 原地升级已有任务；如果正在下载，先取消当前任务。
                    if existing_status == DownloadStatus::Downloading {
                        let mut active = self.active_downloads.lock().unwrap();
                        if let Some(cancel_tx) =
                            active.get_mut(&queue[idx].id).and_then(Option::take)
                        {
                            let _ = cancel_tx.send(());
                        }
                    }

                    let old_version = existing_version;
                    // 替换旧任务，并重置为等待下载。
                    item.status = DownloadStatus::Pending;
                    queue[idx] = item;

                    AddResult::Upgraded(old_version, parsed.version)
                }
            } else {
                // 3. 普通入队。
                queue.push(item);
                AddResult::Added
            }
        };

        self.save_queue_to_disk();
        self.emit_queue_updated();

        result
    }
}

pub fn start_download_worker(app_handle: tauri::AppHandle, manager: Arc<DownloadManager>) {
    tauri::async_runtime::spawn(async move {
        let mut interval = tokio::time::interval(std::time::Duration::from_millis(500));
        loop {
            interval.tick().await;

            // Claim the task and its cancellation slot under the same locks. A paused
            // worker retains its slot until it exits, so immediate resume cannot duplicate it.
            let claimed = {
                let mut queue = manager.queue.lock().unwrap();
                let mut active = manager.active_downloads.lock().unwrap();
                if active.len() < *manager.max_concurrent.lock().unwrap() {
                    if let Some(item) = queue.iter_mut().find(|i| {
                        i.status == DownloadStatus::Pending && !active.contains_key(&i.id)
                    }) {
                        item.status = DownloadStatus::Downloading;
                        item.error_msg = None;
                        let (tx, rx) = tokio::sync::oneshot::channel();
                        active.insert(item.id.clone(), Some(tx));
                        Some((item.clone(), rx))
                    } else {
                        None
                    }
                } else {
                    None
                }
            };
            if let Some((item, cancel_rx)) = claimed {
                manager.save_queue_to_disk();
                manager.emit_queue_updated();
                let mgr = Arc::clone(&manager);
                let app = app_handle.clone();
                tauri::async_runtime::spawn(async move {
                    if let Err(e) =
                        download_file_task(app.clone(), Arc::clone(&mgr), item.clone(), cancel_rx)
                            .await
                    {
                        // 非暂停/取消导致的错误标记为失败。
                        {
                            let mut queue = mgr.queue.lock().unwrap();
                            if let Some(queue_item) = queue.iter_mut().find(|i| i.id == item.id) {
                                if queue_item.status == DownloadStatus::Downloading {
                                    queue_item.status = DownloadStatus::Failed;
                                    queue_item.error_msg = Some(e.clone());
                                    queue_item.speed_bytes_per_sec = 0;
                                }
                            }
                        }
                        {
                            let mut active = mgr.active_downloads.lock().unwrap();
                            active.remove(&item.id);
                        }
                        mgr.save_queue_to_disk();
                        mgr.emit_queue_updated();
                    } else {
                        // 下载成功完成。
                        {
                            let mut queue = mgr.queue.lock().unwrap();
                            if let Some(queue_item) = queue.iter_mut().find(|i| i.id == item.id) {
                                if queue_item.status == DownloadStatus::Downloading {
                                    queue_item.status = DownloadStatus::Completed;
                                    queue_item.downloaded_bytes = queue_item.total_bytes;
                                    queue_item.progress = 100.0;
                                    queue_item.speed_bytes_per_sec = 0;
                                    queue_item.error_msg = None;
                                }
                            }
                        }
                        {
                            let mut active = mgr.active_downloads.lock().unwrap();
                            active.remove(&item.id);
                        }
                        mgr.save_queue_to_disk();
                        mgr.emit_queue_updated();
                    }
                });
            }
        }
    });
}

async fn download_file_task(
    app_handle: tauri::AppHandle,
    manager: Arc<DownloadManager>,
    item: DownloadItem,
    mut cancel_rx: tokio::sync::oneshot::Receiver<()>,
) -> Result<(), String> {
    let install_context = resolve_install_context(&app_handle)?;
    let target_dir = PathBuf::from(&install_context.download_target_dir);
    let temp_dir = PathBuf::from(&install_context.download_temp_dir);
    std::fs::create_dir_all(&target_dir)
        .map_err(|e| format!("无法创建下载目标目录 {}: {}", target_dir.display(), e))?;
    std::fs::create_dir_all(&temp_dir)
        .map_err(|e| format!("无法创建临时下载目录 {}: {}", temp_dir.display(), e))?;

    let target_path = target_dir.join(&item.filename);
    let part_path = temp_dir.join(format!("{}.part", item.filename));
    {
        let mut queue = manager.queue.lock().unwrap();
        if let Some(queue_item) = queue.iter_mut().find(|i| i.id == item.id) {
            queue_item.final_path = Some(target_path.to_string_lossy().to_string());
            queue_item.temp_path = Some(part_path.to_string_lossy().to_string());
            queue_item.install_mode = Some(install_context.mode_label.clone());
        }
    }
    manager.save_queue_to_disk();
    manager.emit_queue_updated();

    if target_path.exists() {
        // A retry after an indexing failure should reuse the saved file, never overwrite it.
        if item.final_path.as_deref() == Some(target_path.to_string_lossy().as_ref()) {
            validate_var_file(&target_path)?;
            return finish_saved_download(
                &app_handle,
                &manager,
                &item,
                &target_path,
                &part_path,
                &install_context.mode_label,
            );
        }
        return Err(format!(
            "目标文件已存在，未覆盖，请扫描资源库确认: {}",
            target_path.display()
        ));
    }
    let downloaded_bytes = std::fs::metadata(&part_path).map(|m| m.len()).unwrap_or(0);
    let client = crate::services::download_http::client()?;
    let cookie_header = hub_cookie_header(&app_handle);
    let mut current_url = item.url.clone();
    let on_retry = |attempt, seconds| {
        if let Some(task) = manager
            .queue
            .lock()
            .unwrap()
            .iter_mut()
            .find(|i| i.id == item.id)
        {
            task.error_msg = Some(format!(
                "连接暂时失败，{} 秒后进行第 {}/3 次尝试；已保留下载进度。",
                seconds, attempt
            ));
            task.speed_bytes_per_sec = 0;
        }
        manager.emit_queue_updated();
    };
    let mut res = crate::services::download_http::request(
        &client,
        &current_url,
        &cookie_header,
        downloaded_bytes,
        &mut cancel_rx,
        on_retry,
    )
    .await?;
    if is_html_response(&res) {
        let html = tokio::select! {
            _=&mut cancel_rx=>return Err("下载已暂停或取消".into()),
            result=res.text()=>result.map_err(crate::services::download_http::transport_error)?,
        };
        current_url = extract_direct_download_url(&html)
            .ok_or("下载入口返回网页，未找到文件地址；请检查 Hub 登录状态及资源权限。")?;
        // Resolving an HTML entry page does not invalidate a partial CDN download.
        res = crate::services::download_http::request(
            &client,
            &current_url,
            &cookie_header,
            downloaded_bytes,
            &mut cancel_rx,
            on_retry,
        )
        .await?;
    }
    let range_rejected = res.status() == reqwest::StatusCode::RANGE_NOT_SATISFIABLE;
    if range_rejected && downloaded_bytes > 0 {
        res = crate::services::download_http::request(
            &client,
            &current_url,
            &cookie_header,
            0,
            &mut cancel_rx,
            on_retry,
        )
        .await?;
    }
    if is_html_response(&res) {
        return Err("文件地址仍返回网页，未修改临时文件；请检查登录状态和下载权限。".into());
    }
    let offset = if range_rejected { 0 } else { downloaded_bytes };
    let (reset, total_size) =
        crate::services::download_http::response_layout(res.status(), res.headers(), offset)?;
    let file = if reset || range_rejected || downloaded_bytes == 0 {
        File::create(&part_path)
            .await
            .map_err(|e| format!("无法创建临时下载文件: {}", e))?
    } else {
        tokio::fs::OpenOptions::new()
            .append(true)
            .open(&part_path)
            .await
            .map_err(|e| format!("无法打开临时下载文件: {}", e))?
    };
    if let Some(task) = manager
        .queue
        .lock()
        .unwrap()
        .iter_mut()
        .find(|i| i.id == item.id)
    {
        task.error_msg = None;
        task.downloaded_bytes = if reset || range_rejected {
            0
        } else {
            downloaded_bytes
        };
    }

    run_download_loop(
        app_handle,
        manager,
        item,
        res,
        file,
        part_path,
        target_path,
        install_context.mode_label,
        total_size,
        &mut cancel_rx,
    )
    .await
}

async fn run_download_loop(
    app_handle: tauri::AppHandle,
    manager: Arc<DownloadManager>,
    item: DownloadItem,
    mut response: reqwest::Response,
    mut file: File,
    part_path: PathBuf,
    target_path: PathBuf,
    install_mode: String,
    total_size: u64,
    cancel_rx: &mut tokio::sync::oneshot::Receiver<()>,
) -> Result<(), String> {
    // Update total bytes
    {
        let mut queue = manager.queue.lock().unwrap();
        if let Some(queue_item) = queue.iter_mut().find(|i| i.id == item.id) {
            queue_item.total_bytes = total_size;
        }
    }

    let mut downloaded_bytes = if part_path.exists() {
        std::fs::metadata(&part_path).map(|m| m.len()).unwrap_or(0)
    } else {
        0
    };

    let mut bytes_since_last_calc = 0;
    let mut last_speed_calc_time = std::time::Instant::now();
    let mut last_progress_emit_time = std::time::Instant::now();

    loop {
        let speed_limit_kb = *manager.speed_limit_kb.lock().unwrap();
        let limit_bytes_per_sec = speed_limit_kb * 1024;

        let start_chunk = std::time::Instant::now();

        let chunk_opt = tokio::select! {
            _ = &mut *cancel_rx => {
                // Pause or cancel signal
                let current_status = {
                    let queue = manager.queue.lock().unwrap();
                    queue.iter().find(|i| i.id == item.id).map(|i| i.status).unwrap_or(DownloadStatus::Paused)
                };

                file.flush().await.map_err(|e|e.to_string())?;
                drop(file);
                if current_status == DownloadStatus::Paused || current_status == DownloadStatus::Pending {
                    // Update speed to 0 in queue
                    let mut queue = manager.queue.lock().unwrap();
                    if let Some(queue_item) = queue.iter_mut().find(|i| i.id == item.id) {
                        queue_item.speed_bytes_per_sec = 0;
                    }
                } else {
                    // Deleted/Cancelled - clean up partial file
                    let _ = std::fs::remove_file(&part_path);
                }
                return Ok(());
            }
            chunk_res = response.chunk() => {
                match chunk_res {
                    Ok(chunk) => chunk,
                    Err(e) => {
                        let _ = file.flush().await;
                        return Err(crate::services::download_http::transport_error(e));
                    },
                }
            }
        };

        let chunk = match chunk_opt {
            Some(c) => c,
            None => break, // Download complete
        };

        let chunk_len = chunk.len();
        file.write_all(&chunk)
            .await
            .map_err(|e| format!("写入文件失败: {}", e))?;

        downloaded_bytes += chunk_len as u64;
        if let Some(task) = manager
            .queue
            .lock()
            .unwrap()
            .iter_mut()
            .find(|i| i.id == item.id)
        {
            task.downloaded_bytes = downloaded_bytes;
            task.progress = if total_size > 0 {
                (downloaded_bytes as f64 / total_size as f64 * 100.0).min(100.0)
            } else {
                0.0
            };
        }
        bytes_since_last_calc += chunk_len as u64;

        // Speed Limiting
        if limit_bytes_per_sec > 0 {
            let elapsed = start_chunk.elapsed();
            let target_duration =
                std::time::Duration::from_secs_f64(chunk_len as f64 / limit_bytes_per_sec as f64);
            if elapsed < target_duration {
                tokio::time::sleep(target_duration - elapsed).await;
            }
        }

        // Speed & Progress Calc (1Hz update)
        let elapsed_calc = last_speed_calc_time.elapsed();
        if elapsed_calc >= std::time::Duration::from_secs(1) {
            let speed = bytes_since_last_calc as f64 / elapsed_calc.as_secs_f64();
            let progress = if total_size > 0 {
                (downloaded_bytes as f64 / total_size as f64) * 100.0
            } else {
                0.0
            };

            {
                let mut queue = manager.queue.lock().unwrap();
                if let Some(queue_item) = queue.iter_mut().find(|i| i.id == item.id) {
                    if queue_item.status == DownloadStatus::Downloading {
                        queue_item.downloaded_bytes = downloaded_bytes;
                        queue_item.progress = progress;
                        queue_item.speed_bytes_per_sec = speed as u64;
                    }
                }
            }

            // Emit progress event
            if last_progress_emit_time.elapsed() >= std::time::Duration::from_secs(1) {
                let progress_payload = serde_json::json!({
                    "id": item.id,
                    "downloaded": downloaded_bytes,
                    "total": total_size,
                    "progress": progress,
                    "speed": speed as u64,
                    "status": "downloading"
                });
                let _ = app_handle.emit("download-progress", progress_payload);
                last_progress_emit_time = std::time::Instant::now();
            }

            bytes_since_last_calc = 0;
            last_speed_calc_time = std::time::Instant::now();
        }
    }

    // Finalize
    file.flush()
        .await
        .map_err(|e| format!("保存文件失败: {}", e))?;

    // Close file so we can rename
    drop(file);

    if total_size > 0 && downloaded_bytes != total_size {
        return Err(format!(
            "下载尚未完整：已接收 {} / {} 字节，临时文件已保留供续传。",
            downloaded_bytes, total_size
        ));
    }
    let check_path = part_path.clone();
    let validation = tokio::task::spawn_blocking(move || validate_var_file(&check_path))
        .await
        .map_err(|e| e.to_string())?;
    if let Err(error) = validation {
        // Only this task's invalid temporary response is removed, never an installed VAR.
        let _ = std::fs::remove_file(&part_path);
        return Err(error);
    }
    if manager
        .queue
        .lock()
        .unwrap()
        .iter()
        .find(|i| i.id == item.id)
        .map(|i| i.status)
        != Some(DownloadStatus::Downloading)
    {
        return Err("下载已暂停或取消，完整临时文件已保留".into());
    }
    crate::services::resource_files::transfer(&part_path, &target_path, "move")?;
    finish_saved_download(
        &app_handle,
        &manager,
        &item,
        &target_path,
        &part_path,
        &install_mode,
    )
}

fn is_html_response(response: &reqwest::Response) -> bool {
    response
        .headers()
        .get(reqwest::header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .map(|v| v.to_ascii_lowercase().contains("text/html"))
        .unwrap_or(false)
}

fn finish_saved_download(
    app: &tauri::AppHandle,
    manager: &DownloadManager,
    item: &DownloadItem,
    target: &Path,
    part: &Path,
    mode: &str,
) -> Result<(), String> {
    let size = std::fs::metadata(target).map_err(|e| e.to_string())?.len();
    let (indexed, warning) = match index_downloaded_package_with_warning(app, target) {
        Ok(warning) => (true, warning),
        Err(error) => (
            false,
            Some(format!(
                "文件已保存，入库未完成；可以重试入库，无需重新下载。{}",
                error
            )),
        ),
    };
    {
        let mut queue = manager.queue.lock().unwrap();
        if let Some(task) = queue.iter_mut().find(|i| i.id == item.id) {
            task.final_path = Some(target.to_string_lossy().into_owned());
            task.temp_path = Some(part.to_string_lossy().into_owned());
            task.install_mode = Some(mode.into());
            task.indexed = indexed;
            task.warning_msg = warning.clone();
            task.total_bytes = size;
            task.downloaded_bytes = size;
            task.progress = 100.0;
            task.speed_bytes_per_sec = 0;
        }
    }
    let _=app.emit("download-progress",serde_json::json!({"id":item.id,"downloaded":size,"total":size,"progress":100.0,"speed":0,"status":"completed","final_path":target.to_string_lossy(),"install_mode":mode,"indexed":indexed,"warning_msg":warning}));
    Ok(())
}

pub fn index_downloaded_package(
    app_handle: &tauri::AppHandle,
    target_path: &Path,
) -> Result<(), String> {
    index_downloaded_package_with_warning(app_handle, target_path).map(|_| ())
}

pub(crate) fn index_downloaded_package_with_warning(
    app_handle: &tauri::AppHandle,
    target_path: &Path,
) -> Result<Option<String>, String> {
    let (package, warning) = var_parser::parse_downloaded_var(target_path)
        .map_err(|e| format!("下载文件已保留，无法解析入库: {}", e))?;
    let db = app_handle.state::<Database>();
    db.with_conn(|conn|{
        let tx=conn.unchecked_transaction()?;
        crate::services::resource_files::index_parsed_package(&tx,&package)?;
        if let Some(warning)=&warning {
            tx.execute("UPDATE physical_packages SET scan_status='metadata_warning',last_error=?1 WHERE file_path=?2",rusqlite::params![warning,package.file_path])?;
        }
        tx.commit()?;Ok(())
    }).map_err(|e|format!("下载文件已保留，数据库写入失败: {}",e))?;
    crate::commands::library_events::emit_library_index_changed(
        app_handle,
        "package_indexed",
        &[
            "packages",
            "dashboard",
            "dependencies",
            "statistics",
            "folders",
            "tags",
            "dedupe",
        ],
        vec![package.id],
        vec![package.file_path],
    );
    Ok(warning)
}
