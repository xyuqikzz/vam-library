use serde::{Deserialize, Serialize};
use std::fs::{self, File};
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};
use tauri::AppHandle;
use zip::{ZipArchive, write::ZipWriter, write::FileOptions};


#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ArchiveAnalysis {
    pub file_path: String,
    pub file_name: String,
    pub size_bytes: u64,
    pub archive_type: String, // "misnamed_var" | "var_container" | "vam_content" | "flat_content" | "unknown"
    pub recommended_action: String, // "rename_to_var" | "extract_vars" | "extract_content" | "extract_flat_appearance" | "extract_flat_scene" | "extract_to_temp"
    pub file_count: usize,
    pub sample_files: Vec<String>,
    pub contains_meta_json: bool,
    pub contains_var_files: bool,
    pub var_files: Vec<String>,
    pub has_image_files: bool,
    pub image_files: Vec<String>,
    pub suspected_var_id: Option<String>,
    pub base_path_in_archive: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UnpackResult {
    pub success: bool,
    pub message: String,
    pub extracted_files: Vec<String>,
    pub destination_path: String,
}

/// Helper to parse a version number from the filename
fn parse_version_from_filename(filename: &str) -> Option<i32> {
    let stem = Path::new(filename)
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or(filename);
    
    // Check if filename fits VAM .var format Creator.PackageName.Version
    let parts: Vec<&str> = stem.split('.').collect();
    if parts.len() >= 3 {
        if let Ok(ver) = parts.last().unwrap().parse::<i32>() {
            return Some(ver);
        }
    }

    // Try regex-like match for a trailing number or _vXX, vXX etc.
    if let Ok(re) = regex::Regex::new(r"(?i)[-_]v?(\d+)") {
        if let Some(caps) = re.captures(stem) {
            if let Some(m) = caps.get(1) {
                if let Ok(ver) = m.as_str().parse::<i32>() {
                    return Some(ver);
                }
            }
        }
    }

    None
}

/// Analyze a zip archive before unpacking, detecting VAM packages and nested layouts.
#[tauri::command]
pub async fn analyze_archive(archive_path: String) -> Result<ArchiveAnalysis, String> {
    let path = Path::new(&archive_path);
    if !path.exists() {
        return Err(format!("文件不存在: {}", archive_path));
    }

    let file_name = path
        .file_name()
        .and_then(|f| f.to_str())
        .unwrap_or("unknown")
        .to_string();

    let file_metadata = fs::metadata(path).map_err(|e| e.to_string())?;
    let size_bytes = file_metadata.len();

    let file = File::open(path).map_err(|e| format!("无法打开文件: {}", e))?;
    let mut archive = ZipArchive::new(file).map_err(|e| format!("无效的 ZIP 格式: {}", e))?;

    let mut file_count = 0;
    let mut sample_files = Vec::new();
    let mut contains_meta_json = false;
    let mut contains_var_files = false;
    let mut var_files = Vec::new();
    let mut image_files = Vec::new();
    let mut meta_json_path: Option<String> = None;
    let mut has_image_files = false;

    let mut has_saves = false;
    let mut has_custom = false;
    let mut has_addon_packages = false;
    let mut paths = Vec::new();

    for i in 0..archive.len() {
        if let Ok(entry) = archive.by_index(i) {
            let name = entry.name().to_string();
            paths.push(name.clone());
            
            if entry.is_file() {
                file_count += 1;
                if sample_files.len() < 12 {
                    sample_files.push(name.clone());
                }

                // Check for meta.json
                if name.ends_with("meta.json") {
                    contains_meta_json = true;
                    meta_json_path = Some(name.clone());
                }

                // Check for .var files
                if name.to_lowercase().ends_with(".var") {
                    contains_var_files = true;
                    var_files.push(name.clone());
                }

                // Check for images
                let ext = Path::new(&name)
                    .extension()
                    .and_then(|e| e.to_str())
                    .unwrap_or("")
                    .to_lowercase();
                if ext == "jpg" || ext == "jpeg" || ext == "png" {
                    has_image_files = true;
                    if image_files.len() < 5 {
                        image_files.push(name.clone());
                    }
                }
            }
        }
    }

    // Classify Type A: Container of .var files
    if contains_var_files {
        return Ok(ArchiveAnalysis {
            file_path: archive_path,
            file_name,
            size_bytes,
            archive_type: "var_container".to_string(),
            recommended_action: "extract_vars".to_string(),
            file_count,
            sample_files,
            contains_meta_json,
            contains_var_files,
            var_files,
            has_image_files,
            image_files,
            suspected_var_id: None,
            base_path_in_archive: "".to_string(),
        });
    }

    // Classify Type B: Misnamed .var Package (contains meta.json)
    if let Some(ref m_path) = meta_json_path {
        // Resolve the base path of the package (e.g. SomeFolder/meta.json -> SomeFolder/)
        let base_path = if m_path == "meta.json" {
            "".to_string()
        } else {
            let p = Path::new(m_path);
            p.parent()
                .map(|p| {
                    let s = p.to_string_lossy().to_string().replace('\\', "/");
                    if s.ends_with('/') || s.is_empty() {
                        s
                    } else {
                        format!("{}/", s)
                    }
                })
                .unwrap_or_else(|| "".to_string())
        };

        // Try reading meta.json to determine creator and name
        let mut suspected_var_id = None;
        if let Ok(mut meta_file) = archive.by_name(m_path) {
            let mut contents = String::new();
            if meta_file.read_to_string(&mut contents).is_ok() {
                if let Ok(parsed) = serde_json::from_str::<crate::models::meta::PackageMeta>(&contents) {
                    let creator = if parsed.creator_name.trim().is_empty() {
                        "Unknown".to_string()
                    } else {
                        parsed.creator_name.trim().to_string()
                    };
                    let name = if parsed.package_name.trim().is_empty() {
                        Path::new(&file_name)
                            .file_stem()
                            .and_then(|s| s.to_str())
                            .unwrap_or("Unknown")
                            .to_string()
                    } else {
                        parsed.package_name.trim().to_string()
                    };

                    let version = parse_version_from_filename(&file_name).unwrap_or(1);
                    suspected_var_id = Some(format!("{}.{}.{}", creator, name, version));
                }
            }
        }

        return Ok(ArchiveAnalysis {
            file_path: archive_path,
            file_name,
            size_bytes,
            archive_type: "misnamed_var".to_string(),
            recommended_action: "rename_to_var".to_string(),
            file_count,
            sample_files,
            contains_meta_json: true,
            contains_var_files: false,
            var_files: Vec::new(),
            has_image_files,
            image_files,
            suspected_var_id,
            base_path_in_archive: base_path,
        });
    }

    // Classify Type C: Standard VAM Content (Saves/, Custom/, Textures/, etc.)
    let mut base_path = "".to_string();
    for p in &paths {
        let p_lower = p.to_lowercase();
        if p_lower.contains("/saves/") || p_lower.starts_with("saves/") {
            let index = p_lower.find("saves/").unwrap();
            base_path = p[..index].to_string();
            has_saves = true;
            break;
        } else if p_lower.contains("/custom/") || p_lower.starts_with("custom/") {
            let index = p_lower.find("custom/").unwrap();
            base_path = p[..index].to_string();
            has_custom = true;
            break;
        } else if p_lower.contains("/addonpackages/") || p_lower.starts_with("addonpackages/") {
            let index = p_lower.find("addonpackages/").unwrap();
            base_path = p[..index].to_string();
            has_addon_packages = true;
            break;
        }
    }

    if has_saves || has_custom || has_addon_packages {
        return Ok(ArchiveAnalysis {
            file_path: archive_path,
            file_name,
            size_bytes,
            archive_type: "vam_content".to_string(),
            recommended_action: "extract_content".to_string(),
            file_count,
            sample_files,
            contains_meta_json: false,
            contains_var_files: false,
            var_files: Vec::new(),
            has_image_files,
            image_files,
            suspected_var_id: None,
            base_path_in_archive: base_path,
        });
    }

    // Classify Type D: Flat presets (.vac Appearance looks or .json Scenes)
    let mut has_presets = false;
    let mut preset_type = "unknown";
    for p in &paths {
        if p.to_lowercase().ends_with(".vac") {
            has_presets = true;
            preset_type = "appearance";
            break;
        } else if p.to_lowercase().ends_with(".json") {
            has_presets = true;
            preset_type = "scene";
        }
    }

    if has_presets {
        return Ok(ArchiveAnalysis {
            file_path: archive_path,
            file_name,
            size_bytes,
            archive_type: "flat_content".to_string(),
            recommended_action: format!("extract_flat_{}", preset_type),
            file_count,
            sample_files,
            contains_meta_json: false,
            contains_var_files: false,
            var_files: Vec::new(),
            has_image_files,
            image_files,
            suspected_var_id: None,
            base_path_in_archive: "".to_string(),
        });
    }

    Ok(ArchiveAnalysis {
        file_path: archive_path,
        file_name,
        size_bytes,
        archive_type: "unknown".to_string(),
        recommended_action: "extract_to_temp".to_string(),
        file_count,
        sample_files,
        contains_meta_json: false,
        contains_var_files: false,
        var_files: Vec::new(),
        has_image_files,
        image_files,
        suspected_var_id: None,
        base_path_in_archive: "".to_string(),
    })
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UnpackProgress {
    pub percentage: f64,
    pub current_file: String,
    pub processed: usize,
    pub total: usize,
}

/// Execute the smart unpacking and flattening on the analyzed archive.
#[tauri::command]
pub async fn execute_unpack(
    _app_handle: AppHandle,
    analysis: ArchiveAnalysis,
    custom_target_dir: Option<String>,
) -> Result<UnpackResult, String> {
    use tauri::Emitter;

    // Resolve the target directory (default is the same directory as the ZIP file)
    let target_dir = if let Some(ref custom_dir) = custom_target_dir {
        PathBuf::from(custom_dir)
    } else {
        Path::new(&analysis.file_path)
            .parent()
            .ok_or_else(|| "无法获取压缩包的同级目录".to_string())?
            .to_path_buf()
    };

    if !target_dir.exists() {
        fs::create_dir_all(&target_dir)
            .map_err(|e| format!("创建目标文件夹失败: {}", e))?;
    }

    let archive_file = File::open(&analysis.file_path)
        .map_err(|e| format!("打不开压缩文件: {}", e))?;
    let mut archive = ZipArchive::new(archive_file)
        .map_err(|e| format!("读取压缩文件失败: {}", e))?;

    let mut extracted_files = Vec::new();
    let action = analysis.recommended_action.as_str();
    let total_entries = archive.len();

    match action {
        "rename_to_var" => {
            // Target var filename
            let target_var_name = if let Some(ref var_id) = analysis.suspected_var_id {
                format!("{}.var", var_id)
            } else {
                let stem = Path::new(&analysis.file_name)
                    .file_stem()
                    .and_then(|s| s.to_str())
                    .unwrap_or("package");
                format!("{}.var", stem)
            };

            let destination_var_path = target_dir.join(&target_var_name);

            // If meta.json is already at the root of the ZIP, we can simply copy the ZIP file
            if analysis.base_path_in_archive.is_empty() {
                let _ = _app_handle.emit("unpack-progress", UnpackProgress {
                    percentage: 10.0,
                    current_file: target_var_name.clone(),
                    processed: 0,
                    total: 1,
                });

                fs::copy(&analysis.file_path, &destination_var_path)
                    .map_err(|e| format!("复制文件到目标位置失败: {}", e))?;
                
                let _ = _app_handle.emit("unpack-progress", UnpackProgress {
                    percentage: 100.0,
                    current_file: target_var_name.clone(),
                    processed: 1,
                    total: 1,
                });

                extracted_files.push(target_var_name.clone());
                
                return Ok(UnpackResult {
                    success: true,
                    message: format!("已成功重命名并放置到同级目录: {}", target_var_name),
                    extracted_files,
                    destination_path: destination_var_path.to_string_lossy().to_string(),
                });
            } else {
                // If meta.json is nested (e.g. nested/meta.json), we REPACK it, stripping the nested wrapper
                let base_path = &analysis.base_path_in_archive;
                let repack_file = File::create(&destination_var_path)
                    .map_err(|e| format!("创建打包文件失败: {}", e))?;
                
                let mut zip_writer = ZipWriter::new(repack_file);
                let options = FileOptions::<()>::default()
                    .compression_method(zip::CompressionMethod::Deflated);

                let mut repack_buffer = vec![0u8; 128 * 1024]; // 128KB chunk

                for i in 0..total_entries {
                    let mut entry = archive.by_index(i).map_err(|e| e.to_string())?;
                    let name = entry.name().replace('\\', "/");

                    let percentage = (i as f64 / total_entries as f64) * 100.0;
                    let _ = _app_handle.emit("unpack-progress", UnpackProgress {
                        percentage,
                        current_file: name.clone(),
                        processed: i,
                        total: total_entries,
                    });
                    
                    if name.starts_with(base_path) && entry.is_file() {
                        let relative_name = &name[base_path.len()..];
                        if relative_name.is_empty() {
                            continue;
                        }

                        zip_writer.start_file(relative_name, options)
                            .map_err(|e| format!("添加打包条目失败: {}", e))?;

                        loop {
                            let read_bytes = entry.read(&mut repack_buffer)
                                .map_err(|e| format!("读取条目失败: {}", e))?;
                            if read_bytes == 0 {
                                break;
                            }
                            zip_writer.write_all(&repack_buffer[..read_bytes])
                                .map_err(|e| format!("写入打包条目失败: {}", e))?;
                        }
                    }
                }

                zip_writer.finish().map_err(|e| format!("完成打包文件失败: {}", e))?;
                extracted_files.push(target_var_name.clone());

                let _ = _app_handle.emit("unpack-progress", UnpackProgress {
                    percentage: 100.0,
                    current_file: target_var_name.clone(),
                    processed: total_entries,
                    total: total_entries,
                });

                return Ok(UnpackResult {
                    success: true,
                    message: format!("已成功提取扁平层并打包至同级目录: {}", target_var_name),
                    extracted_files,
                    destination_path: destination_var_path.to_string_lossy().to_string(),
                });
            }
        }

        "extract_vars" => {
            // Extract all .var files from inside the ZIP directly into the target dir
            for i in 0..total_entries {
                let mut entry = archive.by_index(i).map_err(|e| e.to_string())?;
                let percentage = (i as f64 / total_entries as f64) * 100.0;

                if entry.is_file() {
                    let file_name = {
                        let name = entry.name();
                        let file_path = Path::new(name);
                        file_path.file_name()
                            .and_then(|f| f.to_str())
                            .unwrap_or("")
                            .to_string()
                    };

                    let _ = _app_handle.emit("unpack-progress", UnpackProgress {
                        percentage,
                        current_file: file_name.clone(),
                        processed: i,
                        total: total_entries,
                    });

                    if !file_name.is_empty() && file_name.to_lowercase().ends_with(".var") {
                        let dest_path = target_dir.join(&file_name);
                        let mut out_file = File::create(&dest_path)
                            .map_err(|e| format!("创建提取文件失败: {}", e))?;
                        
                        io::copy(&mut entry, &mut out_file)
                            .map_err(|e| format!("写入提取文件失败: {}", e))?;
                        
                        extracted_files.push(file_name);
                    }
                } else {
                    let _ = _app_handle.emit("unpack-progress", UnpackProgress {
                        percentage,
                        current_file: entry.name().to_string(),
                        processed: i,
                        total: total_entries,
                    });
                }
            }

            let _ = _app_handle.emit("unpack-progress", UnpackProgress {
                percentage: 100.0,
                current_file: "完成解压".to_string(),
                processed: total_entries,
                total: total_entries,
            });

            return Ok(UnpackResult {
                success: true,
                message: format!("已成功在压缩包同级解压提取出 {} 个 VAR 包", extracted_files.len()),
                extracted_files,
                destination_path: target_dir.to_string_lossy().to_string(),
            });
        }

        "extract_content" => {
            // Strip the base wrapper path (if any) and extract VAM standard directories directly to target dir
            let base_path = &analysis.base_path_in_archive;
            let mut chunk_buffer = vec![0u8; 128 * 1024];

            for i in 0..total_entries {
                let mut entry = archive.by_index(i).map_err(|e| e.to_string())?;
                let name = entry.name().replace('\\', "/");
                let percentage = (i as f64 / total_entries as f64) * 100.0;

                let _ = _app_handle.emit("unpack-progress", UnpackProgress {
                    percentage,
                    current_file: name.clone(),
                    processed: i,
                    total: total_entries,
                });
                
                if name.starts_with(base_path) && entry.is_file() {
                    let relative_name = &name[base_path.len()..];
                    if relative_name.is_empty() {
                        continue;
                    }

                    let dest_path = target_dir.join(relative_name);
                    
                    if let Some(parent) = dest_path.parent() {
                        if !parent.exists() {
                            fs::create_dir_all(parent)
                                .map_err(|e| format!("创建解压目录失败: {}", e))?;
                        }
                    }

                    let mut out_file = File::create(&dest_path)
                        .map_err(|e| format!("写入解压物理文件失败: {}", e))?;
                    
                    loop {
                        let read_bytes = entry.read(&mut chunk_buffer)
                            .map_err(|e| format!("读取条目失败: {}", e))?;
                        if read_bytes == 0 {
                            break;
                        }
                        out_file.write_all(&chunk_buffer[..read_bytes])
                            .map_err(|e| format!("写入数据失败: {}", e))?;
                    }

                    extracted_files.push(relative_name.to_string());
                }
            }

            let _ = _app_handle.emit("unpack-progress", UnpackProgress {
                percentage: 100.0,
                current_file: "完成解压".to_string(),
                processed: total_entries,
                total: total_entries,
            });

            return Ok(UnpackResult {
                success: true,
                message: format!("成功整理并提取 {} 个资源文件夹至同级目录", extracted_files.len()),
                extracted_files,
                destination_path: target_dir.to_string_lossy().to_string(),
            });
        }

        "extract_flat_appearance" | "extract_flat_scene" => {
            // Extract flat files directly to target_dir (zip same directory)
            for i in 0..total_entries {
                let mut entry = archive.by_index(i).map_err(|e| e.to_string())?;
                let percentage = (i as f64 / total_entries as f64) * 100.0;

                if entry.is_file() {
                    let file_name = {
                        let name = entry.name();
                        let file_path = Path::new(name);
                        file_path.file_name()
                            .and_then(|f| f.to_str())
                            .unwrap_or("")
                            .to_string()
                    };

                    let _ = _app_handle.emit("unpack-progress", UnpackProgress {
                        percentage,
                        current_file: file_name.clone(),
                        processed: i,
                        total: total_entries,
                    });

                    if !file_name.is_empty() {
                        let dest_path = target_dir.join(&file_name);
                        let mut out_file = File::create(&dest_path)
                            .map_err(|e| format!("创建提取目标文件失败: {}", e))?;
                        
                        io::copy(&mut entry, &mut out_file)
                            .map_err(|e| format!("解压写入文件失败: {}", e))?;
                        
                        extracted_files.push(file_name);
                    }
                } else {
                    let _ = _app_handle.emit("unpack-progress", UnpackProgress {
                        percentage,
                        current_file: entry.name().to_string(),
                        processed: i,
                        total: total_entries,
                    });
                }
            }

            let folder_label = if action == "extract_flat_appearance" { "外观预设" } else { "场景预设" };

            let _ = _app_handle.emit("unpack-progress", UnpackProgress {
                percentage: 100.0,
                current_file: "完成解压".to_string(),
                processed: total_entries,
                total: total_entries,
            });

            return Ok(UnpackResult {
                success: true,
                message: format!("已成功将 {} 个散装{}文件解压提取至同级目录", extracted_files.len(), folder_label),
                extracted_files,
                destination_path: target_dir.to_string_lossy().to_string(),
            });
        }

        "extract_to_temp" | _ => {
            // Extract to a folder under target_dir (zip same directory)
            let stem = Path::new(&analysis.file_name)
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("unpacked_archive");
            
            let temp_dest = target_dir.join(format!("{}_unpacked", stem));
            if !temp_dest.exists() {
                fs::create_dir_all(&temp_dest)
                    .map_err(|e| format!("创建临时压缩包解压目录失败: {}", e))?;
            }

            for i in 0..total_entries {
                let mut entry = archive.by_index(i).map_err(|e| e.to_string())?;
                let name = entry.name().replace('\\', "/");
                let dest_path = temp_dest.join(&name);
                let percentage = (i as f64 / total_entries as f64) * 100.0;

                let _ = _app_handle.emit("unpack-progress", UnpackProgress {
                    percentage,
                    current_file: name.clone(),
                    processed: i,
                    total: total_entries,
                });
                
                if entry.is_dir() {
                    fs::create_dir_all(&dest_path)
                        .map_err(|e| format!("创建解压目录失败: {}", e))?;
                } else {
                    if let Some(parent) = dest_path.parent() {
                        if !parent.exists() {
                            fs::create_dir_all(parent)
                                .map_err(|e| format!("创建解压目录失败: {}", e))?;
                        }
                    }

                    let mut out_file = File::create(&dest_path)
                        .map_err(|e| format!("创建提取文件失败: {}", e))?;
                    io::copy(&mut entry, &mut out_file)
                        .map_err(|e| format!("提取文件失败: {}", e))?;
                    extracted_files.push(name);
                }
            }

            let _ = _app_handle.emit("unpack-progress", UnpackProgress {
                percentage: 100.0,
                current_file: "完成解压".to_string(),
                processed: total_entries,
                total: total_entries,
            });

            return Ok(UnpackResult {
                success: true,
                message: format!("已成功在同级解压 {} 个文件至安全隔离目录", extracted_files.len()),
                extracted_files,
                destination_path: temp_dest.to_string_lossy().to_string(),
            });
        }
    }
}
