use regex::Regex;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::io::Read;
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
    let mut file = std::fs::File::open(path).map_err(|e| format!("无法校验下载文件: {}", e))?;
    let mut signature = [0_u8; 4];
    let bytes_read = file
        .read(&mut signature)
        .map_err(|e| format!("无法读取下载文件头: {}", e))?;

    if bytes_read < 4 || &signature[0..2] != b"PK" {
        return Err(
            "下载结果不是有效的 .var 包，Hub 可能返回了网页/登录页而不是文件。".to_string(),
        );
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
    pub active_downloads: Arc<Mutex<HashMap<String, tokio::sync::oneshot::Sender<()>>>>,
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
                        if let Some(cancel_tx) = active.remove(&queue[idx].id) {
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

            // Check if there is space for new active downloads
            let (item_to_download, _max_concurrent_val) = {
                let queue = manager.queue.lock().unwrap();
                let active = manager.active_downloads.lock().unwrap();
                let max_concurrent_val = *manager.max_concurrent.lock().unwrap();

                if active.len() < max_concurrent_val {
                    // Find first pending item
                    let pending_item = queue
                        .iter()
                        .find(|i| i.status == DownloadStatus::Pending)
                        .cloned();
                    (pending_item, max_concurrent_val)
                } else {
                    (None, max_concurrent_val)
                }
            };

            if let Some(item) = item_to_download {
                // Set status to Downloading
                {
                    let mut queue = manager.queue.lock().unwrap();
                    if let Some(queue_item) = queue.iter_mut().find(|i| i.id == item.id) {
                        queue_item.status = DownloadStatus::Downloading;
                    }
                }
                manager.save_queue_to_disk();
                manager.emit_queue_updated();

                // Spawn download task
                let mgr = Arc::clone(&manager);
                let app = app_handle.clone();
                let (cancel_tx, cancel_rx) = tokio::sync::oneshot::channel::<()>();

                {
                    let mut active = manager.active_downloads.lock().unwrap();
                    active.insert(item.id.clone(), cancel_tx);
                }

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

    // Support resume downloading if part file exists
    let mut downloaded_bytes = 0;
    if part_path.exists() {
        if let Ok(metadata) = std::fs::metadata(&part_path) {
            downloaded_bytes = metadata.len();
        }
    }

    let mut file = if downloaded_bytes > 0 {
        tokio::fs::OpenOptions::new()
            .write(true)
            .append(true)
            .open(&part_path)
            .await
            .map_err(|e| format!("无法打开临时下载文件: {}", e))?
    } else {
        File::create(&part_path)
            .await
            .map_err(|e| format!("无法创建临时下载文件: {}", e))?
    };

    // 连接 VAM Hub。站内下载入口可能返回 HTML，需要从中提取真实 CDN 文件地址。
    let client = reqwest::Client::builder()
        .connect_timeout(std::time::Duration::from_secs(10))
        .build()
        .map_err(|e| format!("Failed to build HTTP client: {}", e))?;
    let original_url = item.url.clone();
    let mut current_url = original_url.clone();
    let cookie_header = hub_cookie_header(&app_handle);
    let mut req = client.get(&current_url)
        .header(reqwest::header::USER_AGENT, "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36")
        .header(reqwest::header::ACCEPT, "application/octet-stream,application/zip,*/*")
        .header(reqwest::header::COOKIE, cookie_header.clone())
        .header(reqwest::header::REFERER, "https://hub.virtamate.com/");

    if downloaded_bytes > 0 {
        req = req.header(
            reqwest::header::RANGE,
            format!("bytes={}-", downloaded_bytes),
        );
    }

    let mut res = req
        .send()
        .await
        .map_err(|e| format!("连接在线库失败: {}", e))?;

    let is_html_response = res
        .headers()
        .get(reqwest::header::CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .map(|value| value.to_ascii_lowercase().contains("text/html"))
        .unwrap_or(false);

    if is_html_response {
        let html = res
            .text()
            .await
            .map_err(|e| format!("读取下载入口页面失败: {}", e))?;

        let direct_url = extract_direct_download_url(&html)
            .ok_or_else(|| "下载入口返回的是网页，但没有找到真实 CDN 文件地址。可能需要登录、权限确认或 Hub 改版。".to_string())?;

        current_url = direct_url;
        if downloaded_bytes > 0 {
            file = File::create(&part_path)
                .await
                .map_err(|e| format!("重置临时文件失败: {}", e))?;
            downloaded_bytes = 0;
        }

        let mut direct_req = client.get(&current_url)
            .header(reqwest::header::USER_AGENT, "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36")
            .header(reqwest::header::ACCEPT, "application/octet-stream,application/zip,*/*")
            .header(reqwest::header::COOKIE, cookie_header.clone())
            .header(reqwest::header::REFERER, original_url);

        if downloaded_bytes > 0 {
            direct_req = direct_req.header(
                reqwest::header::RANGE,
                format!("bytes={}-", downloaded_bytes),
            );
        }

        res = direct_req
            .send()
            .await
            .map_err(|e| format!("连接真实文件地址失败: {}", e))?;
    }

    if !res.status().is_success() && res.status() != reqwest::StatusCode::PARTIAL_CONTENT {
        // Range 不可用时，清空临时文件并重新完整下载。
        if downloaded_bytes > 0 {
            file = File::create(&part_path)
                .await
                .map_err(|e| format!("重置临时文件失败: {}", e))?;
            downloaded_bytes = 0;

            // 不带 Range 重新请求。
            let retry_res = client
                .get(&current_url)
                .header(
                    reqwest::header::USER_AGENT,
                    "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36",
                )
                .header(
                    reqwest::header::ACCEPT,
                    "application/octet-stream,application/zip,*/*",
                )
                .header(reqwest::header::COOKIE, cookie_header)
                .header(reqwest::header::REFERER, "https://hub.virtamate.com/")
                .send()
                .await
                .map_err(|e| format!("连接在线库失败: {}", e))?;

            if !retry_res.status().is_success() {
                return Err(format!("在线库返回错误代码: {}", retry_res.status()));
            }
            return run_download_loop(
                app_handle,
                manager,
                item,
                retry_res,
                file,
                part_path,
                target_path,
                install_context.mode_label.clone(),
                0,
                &mut cancel_rx,
            )
            .await;
        }
        return Err(format!("在线库返回错误代码: {}", res.status()));
    }

    let total_size = if res.status() == reqwest::StatusCode::PARTIAL_CONTENT {
        downloaded_bytes + res.content_length().unwrap_or(0)
    } else {
        // 请求 Range 但服务端返回完整内容时，重置临时文件。
        if downloaded_bytes > 0 {
            file = File::create(&part_path)
                .await
                .map_err(|e| format!("重置临时文件失败: {}", e))?;
            downloaded_bytes = 0;
        }
        res.content_length().unwrap_or(0)
    };

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

                if current_status == DownloadStatus::Paused {
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
                    Err(e) => return Err(format!("数据传输失败: {}", e)),
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

    if let Err(e) = validate_var_file(&part_path) {
        let _ = std::fs::remove_file(&part_path);
        return Err(e);
    }

    if target_path.exists() {
        let _ = std::fs::remove_file(&target_path);
    }

    std::fs::rename(&part_path, &target_path).map_err(|e| format!("保存包文件失败: {}", e))?;
    index_downloaded_package(&app_handle, &target_path)?;

    let progress_payload = serde_json::json!({
        "id": item.id,
        "downloaded": total_size,
        "total": total_size,
        "progress": 100.0,
        "speed": 0,
        "status": "completed",
        "final_path": target_path.to_string_lossy().to_string(),
        "install_mode": install_mode,
        "indexed": true
    });
    let _ = app_handle.emit("download-progress", progress_payload);

    {
        let mut queue = manager.queue.lock().unwrap();
        if let Some(queue_item) = queue.iter_mut().find(|i| i.id == item.id) {
            queue_item.final_path = Some(target_path.to_string_lossy().to_string());
            queue_item.temp_path = Some(part_path.to_string_lossy().to_string());
            queue_item.install_mode = Some(install_mode);
            queue_item.indexed = true;
        }
    }

    Ok(())
}

pub fn index_downloaded_package(
    app_handle: &tauri::AppHandle,
    target_path: &Path,
) -> Result<(), String> {
    let package = var_parser::parse_var_file(target_path).map_err(|e| {
        let _ = std::fs::remove_file(target_path);
        format!("下载文件已保存但解析入库失败，已清理坏文件: {}", e)
    })?;
    let package_id = package.id.clone();
    let package_path = package.file_path.clone();

    let db = app_handle.state::<Database>();
    db.with_conn(|conn| {
        let tx = conn.unchecked_transaction().map_err(|e| {
            crate::errors::AppError::Database(format!("开始下载入库事务失败: {}", e))
        })?;

        let resource_types_json =
            serde_json::to_string(&package.resource_types).unwrap_or_else(|_| "[]".to_string());
        let meta_json = package
            .meta
            .as_ref()
            .and_then(|meta| serde_json::to_string(meta).ok());
        let description = package.meta.as_ref().and_then(|meta| meta.description.clone());
        let credits = package.meta.as_ref().and_then(|meta| meta.credits.clone());
        let instructions = package.meta.as_ref().and_then(|meta| meta.instructions.clone());
        let promotional_link = package
            .meta
            .as_ref()
            .and_then(|meta| meta.promotional_link.clone());
        let license_type = package
            .meta
            .as_ref()
            .map(|meta| meta.license_type.clone())
            .unwrap_or_default();

        tx.execute(
            "INSERT INTO packages (
                id, creator, name, version, file_path, size_bytes,
                license_type, description, credits, instructions,
                promotional_link, meta_json, resource_types, file_created_time, scan_time
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15)
            ON CONFLICT(id) DO UPDATE SET
                creator = excluded.creator,
                name = excluded.name,
                version = excluded.version,
                file_path = excluded.file_path,
                size_bytes = excluded.size_bytes,
                license_type = excluded.license_type,
                description = excluded.description,
                credits = excluded.credits,
                instructions = excluded.instructions,
                promotional_link = excluded.promotional_link,
                meta_json = excluded.meta_json,
                resource_types = excluded.resource_types,
                file_created_time = excluded.file_created_time,
                scan_time = excluded.scan_time,
                updated_at = datetime('now')",
            rusqlite::params![
                &package.id,
                &package.creator,
                &package.name,
                package.version,
                &package.file_path,
                package.size_bytes,
                license_type,
                description,
                credits,
                instructions,
                promotional_link,
                meta_json,
                resource_types_json,
                package.created_time,
                package.scan_time,
            ],
        )
        .map_err(|e| {
            crate::errors::AppError::Database(format!(
                "写入下载包索引失败 '{}': {}",
                package.id, e
            ))
        })?;

        tx.execute(
            "INSERT OR REPLACE INTO physical_packages (file_path, package_id, size_bytes, scan_time)
             VALUES (?1, ?2, ?3, ?4)",
            rusqlite::params![
                &package.file_path,
                &package.id,
                package.size_bytes as i64,
                &package.scan_time,
            ],
        )
        .map_err(|e| crate::errors::AppError::Database(format!("写入物理包索引失败: {}", e)))?;

        tx.execute("DELETE FROM contents WHERE package_id = ?1", [&package.id])
            .map_err(|e| {
                crate::errors::AppError::Database(format!("清理旧内容索引失败: {}", e))
            })?;
        tx.execute("DELETE FROM dependencies WHERE package_id = ?1", [&package.id])
            .map_err(|e| {
                crate::errors::AppError::Database(format!("清理旧依赖索引失败: {}", e))
            })?;

        for (content_path, size) in &package.contents {
            let resource_type = crate::models::resource::ResourceType::from_path(content_path);
            tx.execute(
                "INSERT OR IGNORE INTO contents (package_id, file_path, resource_type, size_bytes)
                 VALUES (?1, ?2, ?3, ?4)",
                rusqlite::params![&package.id, content_path, resource_type.as_str(), size],
            )
            .map_err(|e| {
                crate::errors::AppError::Database(format!("写入内容索引失败: {}", e))
            })?;
        }

        if let Some(meta) = &package.meta {
            for (dep_id, _) in &meta.dependencies {
                let parts: Vec<&str> = dep_id.splitn(3, '.').collect();
                let required_version = if parts.len() == 3 {
                    parts[2].to_string()
                } else {
                    "latest".to_string()
                };

                tx.execute(
                    "INSERT OR IGNORE INTO dependencies (package_id, depends_on_id, required_version)
                     VALUES (?1, ?2, ?3)",
                    rusqlite::params![&package.id, dep_id, required_version],
                )
                .map_err(|e| {
                    crate::errors::AppError::Database(format!("写入依赖索引失败: {}", e))
                })?;
            }
        }

        tx.commit().map_err(|e| {
            crate::errors::AppError::Database(format!("提交下载入库事务失败: {}", e))
        })?;
        Ok(())
    })
    .map_err(|e| e.to_string())?;

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
        vec![package_id],
        vec![package_path],
    );
    Ok(())
}
