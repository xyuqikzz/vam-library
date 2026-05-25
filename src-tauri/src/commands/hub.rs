use regex::Regex;
use reqwest::header::{HeaderMap, HeaderValue, CONTENT_TYPE, USER_AGENT};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use tauri::Emitter;
use tokio::fs::File;
use tokio::io::AsyncWriteExt;

use crate::services::downloader::{index_downloaded_package, validate_var_file};
use crate::services::install_context::resolve_install_context;
use tauri::Manager;
use std::sync::{Mutex, OnceLock};

const VAM_HUB_CONSENT_COOKIE: &str = "vamhubconsent=yes";

#[derive(Clone)]
pub struct CacheEntry {
    pub response: HubApiResponse,
    pub is_fully_enriched: bool,
}

static HUB_CACHE: OnceLock<Mutex<HashMap<String, CacheEntry>>> = OnceLock::new();

fn get_hub_cache() -> &'static Mutex<HashMap<String, CacheEntry>> {
    HUB_CACHE.get_or_init(|| Mutex::new(HashMap::new()))
}

#[derive(Serialize)]
struct HubApiRequest {
    source: String,
    action: String,
    latest_image: String,
    package_name: String,
}

#[derive(Deserialize, Debug, Serialize, Clone)]
pub struct HubFile {
    pub filename: Option<String>,
    pub file_size: Option<String>,
    #[serde(rename = "creatorName")]
    pub creator_name: Option<String>,
    #[serde(rename = "licenseType")]
    pub license_type: Option<String>,
    #[serde(rename = "urlHosted")]
    pub url_hosted: Option<String>,
}

#[derive(Deserialize, Debug, Serialize, Clone)]
pub struct HubDependency {
    #[serde(rename = "packageName")]
    pub package_name: Option<String>,
    pub filename: Option<String>,
    pub username: Option<String>,
    #[serde(rename = "licenseType")]
    pub license_type: Option<String>,
    pub version: Option<String>,
    #[serde(rename = "downloadUrl")]
    pub download_url: Option<String>,
    #[serde(rename = "file_size")]
    pub file_size: Option<String>,
    #[serde(rename = "resource_id")]
    pub resource_id: Option<String>,
    #[serde(rename = "latest_version")]
    pub latest_version: Option<String>,
    #[serde(rename = "latestUrl")]
    pub latest_url: Option<String>,
    #[serde(
        rename = "dependencyType",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub dependency_type: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tier: Option<u32>,
}

#[derive(Deserialize, Debug, Serialize, Clone)]
pub struct HubApiResponse {
    pub title: Option<String>,
    pub tag_line: Option<String>,
    pub username: Option<String>,
    pub version_string: Option<String>,
    pub description: Option<String>,
    pub download_count: Option<String>,
    pub rating_avg: Option<String>,
    pub image_url: Option<String>,
    pub icon_url: Option<String>,
    pub resource_id: Option<String>,
    pub category: Option<String>,
    pub view_count: Option<String>,
    pub rating_count: Option<String>,
    pub dependency_count: Option<u32>,
    #[serde(rename = "hubFiles")]
    pub hub_files: Option<Vec<HubFile>>,
    pub dependencies: Option<HashMap<String, Vec<HubDependency>>>,
}

#[derive(Debug, Clone)]
struct PageDependency {
    full_name: String,
    package_name: String,
    version: Option<String>,
    license_type: Option<String>,
    download_url: Option<String>,
    dependency_type: String,
    tier: u32,
}

async fn send_hub_request(
    client: &reqwest::Client,
    payload: &serde_json::Value,
) -> Result<serde_json::Value, String> {
    let mut headers = HeaderMap::new();
    headers.insert(
        USER_AGENT,
        HeaderValue::from_static("Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36")
    );
    headers.insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));

    let res = client
        .post("https://hub.virtamate.com/citizenx/api.php")
        .headers(headers)
        .json(payload)
        .send()
        .await
        .map_err(|e| format!("Network request failed: {}", e))?;

    if !res.status().is_success() {
        return Err(format!(
            "Hub returned unsuccessful status code: {}",
            res.status()
        ));
    }

    let raw_text = res
        .text()
        .await
        .map_err(|e| format!("Failed to read response body: {}", e))?;

    let parsed: serde_json::Value = serde_json::from_str(&raw_text).map_err(|e| {
        format!(
            "Failed to parse response as JSON. Error: {}. Raw response: {}",
            e, raw_text
        )
    })?;

    if let Some(status) = parsed.get("status").and_then(|s| s.as_str()) {
        if status == "error" {
            if let Some(err_msg) = parsed.get("error").and_then(|e| e.as_str()) {
                return Err(err_msg.to_string());
            }
        }
    }

    Ok(parsed)
}

fn hub_cookie_header(app_handle: Option<&tauri::AppHandle>) -> String {
    let mut cookies = vec![VAM_HUB_CONSENT_COOKIE.to_string()];
    if let Some(app_handle) = app_handle {
        if let Some(auth_cookie) = read_hub_auth_cookie(app_handle) {
            cookies.push(auth_cookie);
        }
    }
    cookies.join("; ")
}

fn read_hub_auth_cookie(app_handle: &tauri::AppHandle) -> Option<String> {
    let app_data_dir = app_handle.path().app_data_dir().ok()?;
    let settings_path = app_data_dir.join("settings.json");
    let contents = std::fs::read_to_string(settings_path).ok()?;
    let settings = serde_json::from_str::<serde_json::Value>(&contents).ok()?;
    settings
        .get("hub_auth_cookie")
        .and_then(|value| value.as_str())
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

fn get_json_u64(val: &serde_json::Value) -> u64 {
    if let Some(s) = val.as_str() {
        let cleaned: String = s.chars().filter(|c| c.is_ascii_digit()).collect();
        cleaned.parse::<u64>().ok().unwrap_or(0)
    } else if let Some(n) = val.as_u64() {
        n
    } else if let Some(f) = val.as_f64() {
        f as u64
    } else {
        0
    }
}

fn get_json_f64(val: &serde_json::Value) -> f64 {
    if let Some(s) = val.as_str() {
        s.parse::<f64>().ok().unwrap_or(0.0)
    } else if let Some(n) = val.as_f64() {
        n
    } else if let Some(i) = val.as_i64() {
        i as f64
    } else {
        0.0
    }
}

fn get_json_string(val: &serde_json::Value) -> String {
    val.as_str().unwrap_or("").to_lowercase()
}

fn html_unescape_minimal(value: &str) -> String {
    value
        .replace("&amp;", "&")
        .replace("&quot;", "\"")
        .replace("&#039;", "'")
        .replace("&#39;", "'")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
}

fn normalize_dependency_key(value: &str) -> String {
    value.trim().trim_end_matches(".var").to_ascii_lowercase()
}

fn split_dependency_full_name(full_name: &str) -> (String, Option<String>) {
    if let Some((package_name, version)) = full_name.rsplit_once('.') {
        if version.eq_ignore_ascii_case("latest") || version.parse::<i32>().is_ok() {
            return (package_name.to_string(), Some(version.to_string()));
        }
    }

    (full_name.to_string(), None)
}

fn parse_dependency_section(html: &str, dependency_type: &str) -> Vec<PageDependency> {
    let row_re = match Regex::new(r#"(?s)<tr class="dataList-row">(.*?)</tr>"#) {
        Ok(re) => re,
        Err(_) => return Vec::new(),
    };
    let package_re = match Regex::new(
        r#"(?s)<td class="dataList-cell">\s*<a href="/resources/[^"]+/">([^<]+)</a>"#,
    ) {
        Ok(re) => re,
        Err(_) => return Vec::new(),
    };
    let license_re = Regex::new(r#"(?s)<span class="label[^"]*"[^>]*>([^<]+)</span>"#).ok();
    let tier_re = match Regex::new(r#"(?s)<td class="dataList-cell">(\d+)</td>"#) {
        Ok(re) => re,
        Err(_) => return Vec::new(),
    };
    let download_re = Regex::new(r#"href="([^"]+/download[^"]*)""#).ok();

    row_re
        .captures_iter(html)
        .filter_map(|row| {
            let row_html = row.get(1)?.as_str();
            let full_name =
                html_unescape_minimal(package_re.captures(row_html)?.get(1)?.as_str().trim());
            let (package_name, version) = split_dependency_full_name(&full_name);
            let tier = tier_re
                .captures(row_html)
                .and_then(|captures| captures.get(1))
                .and_then(|value| value.as_str().parse::<u32>().ok())
                .unwrap_or(if dependency_type == "direct" { 1 } else { 2 });
            let license_type = license_re
                .as_ref()
                .and_then(|re| re.captures(row_html))
                .and_then(|captures| captures.get(1))
                .map(|value| html_unescape_minimal(value.as_str().trim()));
            let download_url = download_re
                .as_ref()
                .and_then(|re| re.captures(row_html))
                .and_then(|captures| captures.get(1))
                .map(|value| {
                    let url = value.as_str().replace("&amp;", "&");
                    if url.starts_with("http") {
                        url
                    } else {
                        format!("https://hub.virtamate.com{}", url)
                    }
                });

            Some(PageDependency {
                full_name,
                package_name,
                version,
                license_type,
                download_url,
                dependency_type: dependency_type.to_string(),
                tier,
            })
        })
        .collect()
}

fn parse_dependency_page(html: &str) -> Vec<PageDependency> {
    let Some(direct_start) = html.find("Direct Dependencies") else {
        return Vec::new();
    };

    let sub_start = html.find("Sub-dependencies");
    let direct_html = match sub_start {
        Some(sub_start) if sub_start > direct_start => &html[direct_start..sub_start],
        _ => &html[direct_start..],
    };

    let mut dependencies = parse_dependency_section(direct_html, "direct");
    if let Some(sub_start) = sub_start {
        dependencies.extend(parse_dependency_section(&html[sub_start..], "sub"));
    }

    dependencies
}

fn page_dependency_lookup(page_dependencies: &[PageDependency]) -> HashMap<String, PageDependency> {
    let mut lookup = HashMap::new();
    for dep in page_dependencies {
        lookup.insert(normalize_dependency_key(&dep.full_name), dep.clone());
        lookup
            .entry(normalize_dependency_key(&dep.package_name))
            .or_insert_with(|| dep.clone());
    }
    lookup
}

fn hub_dependency_lookup_keys(dep: &HubDependency) -> Vec<String> {
    let mut keys = Vec::new();
    if let Some(filename) = &dep.filename {
        keys.push(normalize_dependency_key(filename));
    }
    if let Some(package_name) = &dep.package_name {
        if let Some(version) = &dep.version {
            keys.push(normalize_dependency_key(&format!(
                "{}.{}",
                package_name, version
            )));
        }
        keys.push(normalize_dependency_key(package_name));
    }
    keys
}

fn apply_page_dependency_metadata(dep: &mut HubDependency, page_dep: &PageDependency) {
    dep.dependency_type = Some(page_dep.dependency_type.clone());
    dep.tier = Some(page_dep.tier);
    if dep.license_type.is_none() {
        dep.license_type = page_dep.license_type.clone();
    }
    if dep.download_url.is_none() {
        dep.download_url = page_dep.download_url.clone();
    }
    if dep.version.is_none() {
        dep.version = page_dep.version.clone();
    }
}

fn fallback_dependency_group_key(response: &HubApiResponse) -> String {
    response
        .hub_files
        .as_ref()
        .and_then(|files| files.first())
        .and_then(|file| file.filename.as_ref())
        .map(|filename| {
            let clean = filename.trim_end_matches(".var");
            split_dependency_full_name(clean).0
        })
        .or_else(|| response.title.clone())
        .unwrap_or_else(|| "resource".to_string())
}

async fn enrich_dependency_sections(
    client: &reqwest::Client,
    response: &mut HubApiResponse,
) -> Result<(), String> {
    let Some(resource_id) = response.resource_id.as_deref().filter(|id| !id.is_empty()) else {
        return Ok(());
    };

    let html = client
        .get(format!(
            "https://hub.virtamate.com/resources/{}/dependencies",
            resource_id
        ))
        .header(
            USER_AGENT,
            "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36",
        )
        .header(reqwest::header::COOKIE, VAM_HUB_CONSENT_COOKIE)
        .send()
        .await
        .map_err(|e| format!("Failed to fetch dependency page: {}", e))?
        .text()
        .await
        .map_err(|e| format!("Failed to read dependency page: {}", e))?;

    let page_dependencies = parse_dependency_page(&html);
    if page_dependencies.is_empty() {
        return Ok(());
    }

    response.dependency_count = Some(page_dependencies.len() as u32);
    let lookup = page_dependency_lookup(&page_dependencies);
    let mut seen = HashSet::new();

    if let Some(deps_map) = response.dependencies.as_mut() {
        for list in deps_map.values_mut() {
            for dep in list {
                let keys = hub_dependency_lookup_keys(dep);
                if let Some(page_dep) = keys.iter().find_map(|key| lookup.get(key)) {
                    apply_page_dependency_metadata(dep, page_dep);
                    seen.insert(normalize_dependency_key(&page_dep.full_name));
                } else {
                    for key in keys {
                        seen.insert(key);
                    }
                }
            }
        }
    }

    let group_key = fallback_dependency_group_key(response);
    let deps_map = response.dependencies.get_or_insert_with(HashMap::new);
    let list = deps_map.entry(group_key).or_insert_with(Vec::new);
    for page_dep in page_dependencies {
        let key = normalize_dependency_key(&page_dep.full_name);
        if seen.contains(&key) {
            continue;
        }

        list.push(HubDependency {
            package_name: Some(page_dep.package_name.clone()),
            filename: Some(page_dep.full_name.clone()),
            username: None,
            license_type: page_dep.license_type.clone(),
            version: page_dep.version.clone(),
            download_url: page_dep.download_url.clone(),
            file_size: None,
            resource_id: None,
            latest_version: None,
            latest_url: None,
            dependency_type: Some(page_dep.dependency_type),
            tier: Some(page_dep.tier),
        });
        seen.insert(key);
    }

    Ok(())
}

#[tauri::command]
pub async fn browse_hub_packages(
    search: String,
    page: u32,
    resource_type: String,
    pricing: String,
    sort: String,
) -> Result<serde_json::Value, String> {
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(12))
        .connect_timeout(std::time::Duration::from_secs(6))
        .build()
        .map_err(|e| format!("Failed to build HTTP client: {}", e))?;

    // Always run the multi-page aggregation to ensure correct sorting and pricing filtering
    // 1. Fetch Page 1 to know how many total pages there are
    let mut payload_page1 = serde_json::json!({
        "source": "VaM",
        "action": "getResources",
        "latest_image": "Y",
        "page": 1,
    });

    if !search.is_empty() {
        if let Some(obj) = payload_page1.as_object_mut() {
            obj.insert(
                "search".to_string(),
                serde_json::Value::String(search.clone()),
            );
        }
    }

    if !resource_type.is_empty() {
        if let Some(obj) = payload_page1.as_object_mut() {
            obj.insert(
                "type".to_string(),
                serde_json::Value::String(resource_type.clone()),
            );
        }
    }

    if !pricing.is_empty() {
        if let Some(obj) = payload_page1.as_object_mut() {
            let mapped_category = match pricing.as_str() {
                "free" => "Free",
                "premium" => "Paid",
                _ => "",
            };
            if !mapped_category.is_empty() {
                obj.insert(
                    "category".to_string(),
                    serde_json::Value::String(mapped_category.to_string()),
                );
            }
        }
    }

    let page1_res = send_hub_request(&client, &payload_page1).await?;

    // Parse total pages
    let total_pages = page1_res
        .get("pagination")
        .and_then(|p| p.get("total_pages"))
        .and_then(|v| {
            if let Some(s) = v.as_str() {
                s.parse::<u32>().ok()
            } else {
                v.as_u64().map(|n| n as u32)
            }
        })
        .unwrap_or(1);

    let mut all_resources = Vec::new();
    if let Some(res_array) = page1_res.get("resources").and_then(|r| r.as_array()) {
        all_resources.extend(res_array.clone());
    }

    // 2. Fetch pages 2 to min(total_pages, 8) in parallel
    let max_pages = std::cmp::min(total_pages, 8);
    if max_pages > 1 {
        let mut join_handles = Vec::new();
        for p in 2..=max_pages {
            let mut p_payload = payload_page1.clone();
            if let Some(obj) = p_payload.as_object_mut() {
                obj.insert("page".to_string(), serde_json::Value::from(p));
            }
            let client_clone = client.clone();
            let handle =
                tokio::spawn(async move { send_hub_request(&client_clone, &p_payload).await });
            join_handles.push(handle);
        }

        for handle in join_handles {
            if let Ok(Ok(res_json)) = handle.await {
                if let Some(res_array) = res_json.get("resources").and_then(|r| r.as_array()) {
                    all_resources.extend(res_array.clone());
                }
            }
        }
    }

    // 3. Deduplicate resources by resource_id and filter by pricing
    let mut seen_ids = std::collections::HashSet::new();
    let mut deduped_resources = Vec::new();
    for item in all_resources {
        let id = item
            .get("resource_id")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();

        // Explicitly filter by pricing category client/memory-side since the XenForo server ignores it
        if !pricing.is_empty() && pricing != "all" {
            let item_category = item.get("category").and_then(|v| v.as_str()).unwrap_or("");
            let match_category = match pricing.as_str() {
                "free" => item_category.to_lowercase() == "free",
                "premium" => {
                    item_category.to_lowercase() == "paid"
                        || item_category.to_lowercase() == "premium"
                }
                _ => true,
            };
            if !match_category {
                continue;
            }
        }

        if !id.is_empty() {
            if seen_ids.insert(id) {
                deduped_resources.push(item);
            }
        } else {
            deduped_resources.push(item);
        }
    }

    // 4. Sort the compiled resources globally
    let sort_type = sort.clone();
    deduped_resources.sort_by(|a, b| {
        match sort_type.as_str() {
            "last_update" => {
                let val_a = a.get("last_update").map(get_json_u64).unwrap_or(0);
                let val_b = b.get("last_update").map(get_json_u64).unwrap_or(0);
                val_b.cmp(&val_a) // Descending
            }
            "resource_date" => {
                let val_a = a.get("resource_date").map(get_json_u64).unwrap_or(0);
                let val_b = b.get("resource_date").map(get_json_u64).unwrap_or(0);
                val_b.cmp(&val_a) // Descending
            }
            "download_count" => {
                let val_a = a.get("download_count").map(get_json_u64).unwrap_or(0);
                let val_b = b.get("download_count").map(get_json_u64).unwrap_or(0);
                val_b.cmp(&val_a) // Descending
            }
            "reaction_score" => {
                let val_a = a.get("reaction_score").map(get_json_u64).unwrap_or(0);
                let val_b = b.get("reaction_score").map(get_json_u64).unwrap_or(0);
                val_b.cmp(&val_a) // Descending
            }
            "rating_weighted" => {
                let val_a = a.get("rating_weighted").map(get_json_f64).unwrap_or(0.0);
                let val_b = b.get("rating_weighted").map(get_json_f64).unwrap_or(0.0);
                val_b
                    .partial_cmp(&val_a)
                    .unwrap_or(std::cmp::Ordering::Equal) // Descending
            }
            "rating_count" => {
                let val_a = a.get("rating_count").map(get_json_u64).unwrap_or(0);
                let val_b = b.get("rating_count").map(get_json_u64).unwrap_or(0);
                val_b.cmp(&val_a) // Descending
            }
            "title" => {
                let val_a = a.get("title").map(get_json_string).unwrap_or_default();
                let val_b = b.get("title").map(get_json_string).unwrap_or_default();
                val_a.cmp(&val_b) // Ascending
            }
            "username" => {
                let val_a = a.get("username").map(get_json_string).unwrap_or_default();
                let val_b = b.get("username").map(get_json_string).unwrap_or_default();
                val_a.cmp(&val_b) // Ascending
            }
            _ => std::cmp::Ordering::Equal,
        }
    });

    // 5. Paginate in memory
    let page_size = 24;
    let start = ((page - 1) * page_size) as usize;
    let end = std::cmp::min(start + page_size as usize, deduped_resources.len());

    let paginated_resources = if start < deduped_resources.len() {
        deduped_resources[start..end].to_vec()
    } else {
        Vec::new()
    };

    let total_pages_calc =
        ((deduped_resources.len() + page_size as usize - 1) / page_size as usize) as u32;

    let response = serde_json::json!({
        "status": "success",
        "resources": paginated_resources,
        "pagination": {
            "current_page": page.to_string(),
            "total_pages": total_pages_calc.to_string(),
            "total_found": deduped_resources.len().to_string(),
        }
    });

    Ok(response)
}

#[tauri::command]
pub async fn fetch_hub_package_info(package_name: String) -> Result<HubApiResponse, String> {
    fetch_hub_package_info_inner(package_name, true).await
}

pub async fn fetch_hub_package_info_basic(package_name: String) -> Result<HubApiResponse, String> {
    fetch_hub_package_info_inner(package_name, false).await
}

async fn fetch_hub_package_info_inner(
    package_name: String,
    enrich_dependencies: bool,
) -> Result<HubApiResponse, String> {
    let key = package_name.trim().to_lowercase();

    // 1. Check cache first
    {
        if let Ok(cache) = get_hub_cache().lock() {
            if let Some(entry) = cache.get(&key) {
                if !enrich_dependencies || entry.is_fully_enriched {
                    return Ok(entry.response.clone());
                }
            }
        }
    }

    // 2. Fetch basic API details (or reuse basic from cache if we just need enrichment)
    let mut api_response = {
        let mut cached_basic = None;
        {
            if let Ok(cache) = get_hub_cache().lock() {
                if let Some(entry) = cache.get(&key) {
                    cached_basic = Some(entry.response.clone());
                }
            }
        }

        if let Some(cached) = cached_basic {
            cached
        } else {
            // Fetch from network
            let client = reqwest::Client::builder()
                .timeout(std::time::Duration::from_secs(12))
                .connect_timeout(std::time::Duration::from_secs(6))
                .build()
                .map_err(|e| format!("Failed to build HTTP client: {}", e))?;

            let is_numeric = !package_name.is_empty() && package_name.chars().all(|c| c.is_ascii_digit());

            let payload = if is_numeric {
                serde_json::json!({
                    "source": "VaM",
                    "action": "getResourceDetail",
                    "latest_image": "Y",
                    "resource_id": package_name.clone(),
                })
            } else {
                serde_json::json!({
                    "source": "VaM",
                    "action": "getResourceDetail",
                    "latest_image": "Y",
                    "package_name": package_name.clone(),
                })
            };

            let mut headers = HeaderMap::new();
            headers.insert(
                USER_AGENT,
                HeaderValue::from_static("Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36")
            );
            headers.insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));

            let res = client
                .post("https://hub.virtamate.com/citizenx/api.php")
                .headers(headers)
                .json(&payload)
                .send()
                .await
                .map_err(|e| format!("Network request failed: {}", e))?;

            if !res.status().is_success() {
                return Err(format!(
                    "Hub returned unsuccessful status code: {}",
                    res.status()
                ));
            }

            let raw_text = res
                .text()
                .await
                .map_err(|e| format!("Failed to read response body: {}", e))?;

            // Attempt to parse JSON
            let parsed: serde_json::Value = serde_json::from_str(&raw_text).map_err(|e| {
                format!(
                    "Failed to parse response as JSON. Error: {}. Raw response: {}",
                    e, raw_text
                )
            })?;

            // Check for VAM Hub errors embedded in successful HTTP response
            if let Some(status) = parsed.get("status").and_then(|s| s.as_str()) {
                if status == "error" {
                    if let Some(err_msg) = parsed.get("error").and_then(|e| e.as_str()) {
                        return Err(err_msg.to_string());
                    }
                }
            }

            // Sometimes they just don't have resource_id or title, which indicates not found
            if let Some(resource_id) = parsed.get("resource_id").and_then(|r| r.as_str()) {
                if resource_id.is_empty() && parsed.get("title").is_none() {
                    return Err("Could not find package on the Hub.".to_string());
                }
            }

            let deserialized: HubApiResponse = serde_json::from_value(parsed)
                .map_err(|e| format!("Failed to deserialize VAM Hub response: {}", e))?;
            
            deserialized
        }
    };

    // 3. If full enrichment is required and we aren't fully enriched yet, enrich now
    let mut scrape_success = false;
    if enrich_dependencies {
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(12))
            .connect_timeout(std::time::Duration::from_secs(6))
            .build()
            .map_err(|e| format!("Failed to build HTTP client: {}", e))?;

        if let Err(e) = enrich_dependency_sections(&client, &mut api_response).await {
            log::warn!(
                "Failed to enrich VAM Hub dependency section metadata for {}: {}",
                package_name,
                e
            );
        } else {
            scrape_success = true;
        }
    }

    // 4. Save to cache and return
    {
        if let Ok(mut cache) = get_hub_cache().lock() {
            let is_fully_enriched = enrich_dependencies && scrape_success;
            let should_save = if let Some(existing) = cache.get(&key) {
                !existing.is_fully_enriched || is_fully_enriched
            } else {
                true
            };

            if should_save {
                let previously_enriched = cache.get(&key).map(|e| e.is_fully_enriched).unwrap_or(false);
                cache.insert(
                    key,
                    CacheEntry {
                        response: api_response.clone(),
                        is_fully_enriched: is_fully_enriched || previously_enriched,
                    },
                );
            }
        }
    }

    Ok(api_response)
}

#[tauri::command]
pub async fn get_hub_login_status(app_handle: tauri::AppHandle) -> Result<bool, String> {
    Ok(read_hub_auth_cookie(&app_handle).is_some())
}

#[derive(Clone, Serialize)]
struct DownloadProgress {
    filename: String,
    downloaded: u64,
    total: u64,
    percentage: f64,
    status: String,
}

#[tauri::command]
pub async fn download_hub_file(
    app_handle: tauri::AppHandle,
    url: String,
    filename: String,
) -> Result<String, String> {
    let install_context = resolve_install_context(&app_handle)?;
    let target_dir = std::path::PathBuf::from(&install_context.download_target_dir);
    let temp_dir = std::path::PathBuf::from(&install_context.download_temp_dir);
    std::fs::create_dir_all(&target_dir)
        .map_err(|e| format!("创建下载目标目录失败 {}: {}", target_dir.display(), e))?;
    std::fs::create_dir_all(&temp_dir)
        .map_err(|e| format!("创建临时下载目录失败 {}: {}", temp_dir.display(), e))?;

    let target_path = target_dir.join(&filename);
    let part_path = temp_dir.join(format!("{}.part", filename));

    // 2. Start HTTP request
    let client = reqwest::Client::builder()
        .connect_timeout(std::time::Duration::from_secs(10))
        .build()
        .map_err(|e| format!("Failed to build HTTP client: {}", e))?;
    let mut res = client
        .get(&url)
        .header(
            reqwest::header::USER_AGENT,
            "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36",
        )
        .header(
            reqwest::header::COOKIE,
            hub_cookie_header(Some(&app_handle)),
        )
        .send()
        .await
        .map_err(|e| format!("Failed to connect to VAM Hub: {}", e))?;

    if !res.status().is_success() {
        return Err(format!(
            "VAM Hub returned unsuccessful status code: {}",
            res.status()
        ));
    }

    let total_size = res.content_length().unwrap_or(0);

    // 3. 写入隔离的临时文件，完成校验后再移动到资源目录。
    let mut file = File::create(&part_path)
        .await
        .map_err(|e| format!("Failed to create local file: {}", e))?;

    let mut downloaded: u64 = 0;

    // 4. Download loop
    while let Some(chunk) = res
        .chunk()
        .await
        .map_err(|e| format!("Error downloading chunk: {}", e))?
    {
        file.write_all(&chunk)
            .await
            .map_err(|e| format!("Failed to write to file: {}", e))?;

        downloaded += chunk.len() as u64;

        let percentage = if total_size > 0 {
            (downloaded as f64 / total_size as f64) * 100.0
        } else {
            0.0
        };

        // Emit progress event
        let _ = app_handle.emit(
            "download-progress",
            DownloadProgress {
                filename: filename.clone(),
                downloaded,
                total: total_size,
                percentage,
                status: "downloading".to_string(),
            },
        );
    }

    file.flush()
        .await
        .map_err(|e| format!("Failed to flush local file: {}", e))?;
    drop(file);

    if let Err(e) = validate_var_file(&part_path) {
        let _ = std::fs::remove_file(&part_path);
        return Err(e);
    }

    if target_path.exists() {
        let _ = std::fs::remove_file(&target_path);
    }
    std::fs::rename(&part_path, &target_path)
        .map_err(|e| format!("Failed to move completed download into place: {}", e))?;
    index_downloaded_package(&app_handle, &target_path)?;

    // 5. Emit completed event
    let _ = app_handle.emit(
        "download-progress",
        DownloadProgress {
            filename: filename.clone(),
            downloaded,
            total: total_size,
            percentage: 100.0,
            status: "completed".to_string(),
        },
    );

    Ok(target_path.to_string_lossy().to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_fetch_hub_package_info() {
        println!("Testing with macgruber.Life.latest...");
        match fetch_hub_package_info("macgruber.Life.latest".to_string()).await {
            Ok(res) => println!("Success: {:?}", res),
            Err(e) => println!("Error: {}", e),
        }

        println!("Testing with NoStage3.Hair_Long_Upswept_Top_Bun.latest...");
        match fetch_hub_package_info("NoStage3.Hair_Long_Upswept_Top_Bun.latest".to_string()).await
        {
            Ok(res) => println!("Success: {:?}", res),
            Err(e) => println!("Error: {}", e),
        }

        println!("Testing with NoOC.Clothing_SailorLingerie.latest...");
        match fetch_hub_package_info("NoOC.Clothing_SailorLingerie.latest".to_string()).await {
            Ok(res) => println!("Success: {:?}", res),
            Err(e) => println!("Error: {}", e),
        }
    }
}
