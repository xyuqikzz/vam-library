use std::collections::HashMap;
use std::path::Path;
use std::time::SystemTime;

use walkdir::WalkDir;

use crate::errors::AppError;
use crate::models::var_package::VarPackage;
use crate::services::var_parser;

use serde::{Deserialize, Serialize};

/// Progress information for ongoing scans
#[derive(Debug, Clone, Serialize, Deserialize)]
#[allow(dead_code)]
pub struct ScanProgress {
    pub total_files: usize,
    pub processed_files: usize,
    pub current_file: String,
    pub phase: String,
}

/// Result summary after a scan completes
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScanResult {
    pub packages_found: usize,
    pub errors: Vec<String>,
    pub duration_ms: u64,
    pub total_files: usize,
    pub skipped_files: usize,
    pub current_file_paths: Vec<String>,
    pub failures: Vec<ScanFailure>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScanFailure {
    pub file_path: String,
    pub size_bytes: u64,
    pub modified_time: String,
    pub error: String,
}

#[derive(Debug, Clone)]
pub struct ScanCacheEntry {
    pub size_bytes: u64,
    pub modified_time: String,
    pub scan_status: String,
}

/// Scan the AddonPackages directory under the given VAM root for .var files.
///
/// Recursively walks the directory, parses each .var file, collects results,
/// and logs errors for any files that couldn't be parsed. Reports progress
/// via the optional `on_progress` callback.
pub fn scan_addon_packages(
    root: &Path,
    on_progress: Option<&mut dyn FnMut(ScanProgress)>,
) -> Result<(Vec<VarPackage>, ScanResult), AppError> {
    scan_addon_packages_with_cache(root, HashMap::new(), on_progress)
}

pub fn scan_addon_packages_with_cache(
    root: &Path,
    cache: HashMap<String, ScanCacheEntry>,
    mut on_progress: Option<&mut dyn FnMut(ScanProgress)>,
) -> Result<(Vec<VarPackage>, ScanResult), AppError> {
    let addon_dir = root.join("AddonPackages");

    if !addon_dir.exists() {
        return Err(AppError::Io(format!(
            "AddonPackages directory not found at '{}'",
            addon_dir.display()
        )));
    }

    let start = std::time::Instant::now();

    let mut source_dirs = vec![addon_dir.clone()];
    let on_demand_library_dir = root.join("VAMBoxLibrary").join("AddonPackages");
    if on_demand_library_dir.exists() {
        source_dirs.push(on_demand_library_dir);
    }

    // First pass: collect all .var file paths
    let mut var_files = Vec::new();
    for source_dir in source_dirs {
        var_files.extend(
            WalkDir::new(&source_dir)
                .follow_links(true)
                .into_iter()
                .filter_map(|entry| entry.ok())
                .filter(|entry| {
                    entry.file_type().is_file()
                        && entry
                            .path()
                            .extension()
                            .map(|ext| ext.eq_ignore_ascii_case("var"))
                            .unwrap_or(false)
                })
                .map(|entry| entry.into_path()),
        );
    }
    var_files.sort_by(|a, b| a.to_string_lossy().cmp(&b.to_string_lossy()));

    let total_files = var_files.len();
    let current_file_paths = var_files
        .iter()
        .map(|path| path.to_string_lossy().to_string())
        .collect::<Vec<_>>();

    // Report scanning phase
    if let Some(ref mut cb) = on_progress {
        cb(ScanProgress {
            total_files,
            processed_files: 0,
            current_file: String::new(),
            phase: "scanning".to_string(),
        });
    }

    let mut packages = Vec::with_capacity(total_files);
    let mut errors = Vec::new();
    let mut failures = Vec::new();
    let mut skipped_files = 0usize;

    // Second pass: parse each .var file
    for (idx, var_path) in var_files.iter().enumerate() {
        let filename = var_path
            .file_name()
            .and_then(|f| f.to_str())
            .unwrap_or("unknown")
            .to_string();

        // Report parsing progress
        if let Some(ref mut cb) = on_progress {
            cb(ScanProgress {
                total_files,
                processed_files: idx,
                current_file: filename.clone(),
                phase: "parsing".to_string(),
            });
        }

        let file_path = var_path.to_string_lossy().to_string();
        let (size_bytes, modified_time) = match std::fs::metadata(var_path) {
            Ok(metadata) => (
                metadata.len(),
                metadata
                    .modified()
                    .map(system_time_to_rfc3339)
                    .unwrap_or_default(),
            ),
            Err(e) => {
                let error_msg = format!("{}: {}", var_path.display(), e);
                errors.push(error_msg.clone());
                failures.push(ScanFailure {
                    file_path,
                    size_bytes: 0,
                    modified_time: String::new(),
                    error: error_msg,
                });
                continue;
            }
        };

        if let Some(cache_entry) = cache.get(&file_path) {
            if cache_entry.scan_status == "ok"
                && cache_entry.size_bytes == size_bytes
                && cache_entry.modified_time == modified_time
            {
                skipped_files += 1;
                continue;
            }
        }

        match var_parser::parse_var_file(var_path) {
            Ok(package) => {
                packages.push(package);
            }
            Err(e) => {
                let error_msg = format!("{}: {}", var_path.display(), e);
                log::warn!("Failed to parse .var file: {}", error_msg);
                errors.push(error_msg.clone());
                failures.push(ScanFailure {
                    file_path,
                    size_bytes,
                    modified_time,
                    error: error_msg,
                });
            }
        }
    }

    let duration = start.elapsed();

    let result = ScanResult {
        packages_found: packages.len() + skipped_files,
        errors,
        duration_ms: duration.as_millis() as u64,
        total_files,
        skipped_files,
        current_file_paths,
        failures,
    };

    // Report completion
    if let Some(ref mut cb) = on_progress {
        cb(ScanProgress {
            total_files,
            processed_files: total_files,
            current_file: String::new(),
            phase: "done".to_string(),
        });
    }

    log::info!(
        "Scan complete: {} packages found, {} errors, took {}ms",
        result.packages_found,
        result.errors.len(),
        result.duration_ms
    );

    Ok((packages, result))
}

/// Convenience wrapper — scan without progress reporting.
pub fn scan_addon_packages_simple(root: &Path) -> Result<(Vec<VarPackage>, ScanResult), AppError> {
    scan_addon_packages(root, None)
}

fn system_time_to_rfc3339(time: SystemTime) -> String {
    let datetime: chrono::DateTime<chrono::Utc> = time.into();
    datetime.to_rfc3339()
}
