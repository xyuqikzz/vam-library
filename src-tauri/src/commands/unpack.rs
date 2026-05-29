use serde::{Deserialize, Serialize};
use std::ffi::OsStr;
use std::fs::{self, File};
use std::io::{self, Read, Write};
use std::path::{Component, Path, PathBuf};
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};
use tauri::{AppHandle, Emitter};
use walkdir::WalkDir;
use zip::{write::FileOptions, ZipArchive, ZipWriter};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ArchiveAnalysis {
    pub file_path: String,
    pub file_name: String,
    pub size_bytes: u64,
    pub archive_type: String,
    pub recommended_action: String,
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

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UnpackProgress {
    pub percentage: f64,
    pub current_file: String,
    pub processed: usize,
    pub total: usize,
}

#[derive(Debug, Clone)]
struct ArchiveEntryInfo {
    name: String,
    is_file: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ArchiveKind {
    Zip,
    SevenZip,
    Rar,
}

impl ArchiveKind {
    fn canonical_extension(self) -> &'static str {
        match self {
            Self::Zip => "zip",
            Self::SevenZip => "7z",
            Self::Rar => "rar",
        }
    }
}

fn parse_version_from_filename(filename: &str) -> Option<i32> {
    let stem = Path::new(filename)
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or(filename);

    let parts: Vec<&str> = stem.split('.').collect();
    if parts.len() >= 3 {
        if let Ok(ver) = parts.last().unwrap().parse::<i32>() {
            return Some(ver);
        }
    }

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

fn lower_extension(path: &Path) -> String {
    path.extension()
        .and_then(|ext| ext.to_str())
        .unwrap_or("")
        .to_ascii_lowercase()
}

fn read_header(path: &Path, size: usize) -> Result<Vec<u8>, String> {
    let mut file = File::open(path).map_err(|e| format!("无法打开文件: {}", e))?;
    let mut buffer = vec![0u8; size];
    let read_len = file
        .read(&mut buffer)
        .map_err(|e| format!("读取文件头失败: {}", e))?;
    buffer.truncate(read_len);
    Ok(buffer)
}

fn archive_kind_from_header(bytes: &[u8]) -> Option<ArchiveKind> {
    if bytes.len() >= 4
        && bytes[0] == 0x50
        && bytes[1] == 0x4B
        && (bytes[2] == 0x03 || bytes[2] == 0x05 || bytes[2] == 0x07)
        && (bytes[3] == 0x04 || bytes[3] == 0x06 || bytes[3] == 0x08)
    {
        return Some(ArchiveKind::Zip);
    }

    if bytes.len() >= 6
        && bytes[0] == 0x37
        && bytes[1] == 0x7A
        && bytes[2] == 0xBC
        && bytes[3] == 0xAF
        && bytes[4] == 0x27
        && bytes[5] == 0x1C
    {
        return Some(ArchiveKind::SevenZip);
    }

    if bytes.len() >= 7
        && bytes[0] == 0x52
        && bytes[1] == 0x61
        && bytes[2] == 0x72
        && bytes[3] == 0x21
        && bytes[4] == 0x1A
        && bytes[5] == 0x07
        && (bytes[6] == 0x00 || bytes[6] == 0x01)
    {
        return Some(ArchiveKind::Rar);
    }

    None
}

fn detect_archive_kind(path: &Path) -> Result<ArchiveKind, String> {
    if !path.exists() {
        return Err(format!("文件不存在: {}", path.display()));
    }

    let header = read_header(path, 8)?;
    if let Some(kind) = archive_kind_from_header(&header) {
        return Ok(kind);
    }

    match lower_extension(path).as_str() {
        "zip" | "var" => Ok(ArchiveKind::Zip),
        "7z" => Ok(ArchiveKind::SevenZip),
        "rar" => Ok(ArchiveKind::Rar),
        _ => Err("暂不支持该文件，支持 rar / zip / 7z，或可识别为伪装压缩包的文件".to_string()),
    }
}

fn detect_archive_kind_optional(path: &Path) -> Option<ArchiveKind> {
    detect_archive_kind(path).ok()
}

fn normalize_archive_name(name: &str) -> String {
    name.replace('\\', "/").trim_start_matches("./").to_string()
}

fn is_image_extension(path: &str) -> bool {
    matches!(
        Path::new(path)
            .extension()
            .and_then(|ext| ext.to_str())
            .unwrap_or("")
            .to_ascii_lowercase()
            .as_str(),
        "jpg" | "jpeg" | "png" | "webp" | "bmp" | "gif"
    )
}

fn list_archive_entries(path: &Path, kind: ArchiveKind) -> Result<Vec<ArchiveEntryInfo>, String> {
    match kind {
        ArchiveKind::Zip => list_zip_entries(path),
        ArchiveKind::SevenZip | ArchiveKind::Rar => list_archive_entries_with_tar(path),
    }
}

fn list_zip_entries(path: &Path) -> Result<Vec<ArchiveEntryInfo>, String> {
    let file = File::open(path).map_err(|e| format!("无法打开压缩包: {}", e))?;
    let mut archive = ZipArchive::new(file).map_err(|e| format!("读取 ZIP 失败: {}", e))?;
    let mut entries = Vec::with_capacity(archive.len());

    for index in 0..archive.len() {
        let entry = archive
            .by_index(index)
            .map_err(|e| format!("读取 ZIP 条目失败: {}", e))?;
        entries.push(ArchiveEntryInfo {
            name: normalize_archive_name(entry.name()),
            is_file: entry.is_file(),
        });
    }

    Ok(entries)
}

fn list_archive_entries_with_tar(path: &Path) -> Result<Vec<ArchiveEntryInfo>, String> {
    let output = Command::new("tar")
        .arg("-tf")
        .arg(path)
        .output()
        .map_err(|e| format!("调用系统解压工具失败: {}", e))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
        let reason = if !stderr.is_empty() { stderr } else { stdout };
        return Err(format!("读取压缩包目录失败: {}", reason));
    }

    let mut entries = Vec::new();
    for line in String::from_utf8_lossy(&output.stdout).lines() {
        let name = normalize_archive_name(line.trim());
        if name.is_empty() {
            continue;
        }
        let is_file = !name.ends_with('/');
        entries.push(ArchiveEntryInfo { name, is_file });
    }

    Ok(entries)
}

fn read_package_meta_from_zip(
    path: &Path,
    meta_path: &str,
) -> Option<crate::models::meta::PackageMeta> {
    let file = File::open(path).ok()?;
    let mut archive = ZipArchive::new(file).ok()?;
    let mut meta_file = archive.by_name(meta_path).ok()?;
    let mut contents = String::new();
    meta_file.read_to_string(&mut contents).ok()?;
    serde_json::from_str::<crate::models::meta::PackageMeta>(&contents).ok()
}

fn detect_base_path_from_meta(meta_path: &str) -> String {
    if meta_path == "meta.json" {
        return String::new();
    }

    let parent = Path::new(meta_path).parent();
    let parent = parent
        .map(|value| value.to_string_lossy().replace('\\', "/"))
        .unwrap_or_default();

    if parent.is_empty() {
        String::new()
    } else {
        format!("{}/", parent.trim_end_matches('/'))
    }
}

fn analyze_entries(
    archive_path: String,
    file_name: String,
    size_bytes: u64,
    kind: ArchiveKind,
    entries: &[ArchiveEntryInfo],
) -> ArchiveAnalysis {
    let mut file_count = 0usize;
    let mut sample_files = Vec::new();
    let mut contains_meta_json = false;
    let mut meta_json_path: Option<String> = None;
    let mut contains_var_files = false;
    let mut var_files = Vec::new();
    let mut has_image_files = false;
    let mut image_files = Vec::new();
    let mut paths = Vec::new();

    for entry in entries {
        let name = entry.name.clone();
        paths.push(name.clone());

        if !entry.is_file {
            continue;
        }

        file_count += 1;
        if sample_files.len() < 12 {
            sample_files.push(name.clone());
        }

        if name.ends_with("meta.json") {
            contains_meta_json = true;
            meta_json_path = Some(name.clone());
        }

        if name.to_ascii_lowercase().ends_with(".var") {
            contains_var_files = true;
            var_files.push(name.clone());
        }

        if is_image_extension(&name) {
            has_image_files = true;
            if image_files.len() < 5 {
                image_files.push(name.clone());
            }
        }
    }

    if contains_var_files {
        return ArchiveAnalysis {
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
            base_path_in_archive: String::new(),
        };
    }

    if let Some(meta_path) = meta_json_path {
        let base_path = detect_base_path_from_meta(&meta_path);
        let suspected_var_id = if kind == ArchiveKind::Zip {
            read_package_meta_from_zip(Path::new(&archive_path), &meta_path).map(|parsed| {
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
                format!("{}.{}.{}", creator, name, version)
            })
        } else {
            None
        };

        return ArchiveAnalysis {
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
        };
    }

    let mut has_saves = false;
    let mut has_custom = false;
    let mut has_addon_packages = false;
    let mut base_path = String::new();

    for path in &paths {
        let lower = path.to_ascii_lowercase();
        if lower.contains("/saves/") || lower.starts_with("saves/") {
            let index = lower.find("saves/").unwrap_or(0);
            base_path = path[..index].to_string();
            has_saves = true;
            break;
        }
        if lower.contains("/custom/") || lower.starts_with("custom/") {
            let index = lower.find("custom/").unwrap_or(0);
            base_path = path[..index].to_string();
            has_custom = true;
            break;
        }
        if lower.contains("/addonpackages/") || lower.starts_with("addonpackages/") {
            let index = lower.find("addonpackages/").unwrap_or(0);
            base_path = path[..index].to_string();
            has_addon_packages = true;
            break;
        }
    }

    if has_saves || has_custom || has_addon_packages {
        return ArchiveAnalysis {
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
        };
    }

    let mut preset_type = None::<&str>;
    for path in &paths {
        let lower = path.to_ascii_lowercase();
        if lower.ends_with(".vac") {
            preset_type = Some("appearance");
            break;
        }
        if lower.ends_with(".json") {
            preset_type = Some("scene");
        }
    }

    if let Some(kind_label) = preset_type {
        return ArchiveAnalysis {
            file_path: archive_path,
            file_name,
            size_bytes,
            archive_type: "flat_content".to_string(),
            recommended_action: format!("extract_flat_{}", kind_label),
            file_count,
            sample_files,
            contains_meta_json: false,
            contains_var_files: false,
            var_files: Vec::new(),
            has_image_files,
            image_files,
            suspected_var_id: None,
            base_path_in_archive: String::new(),
        };
    }

    ArchiveAnalysis {
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
        base_path_in_archive: String::new(),
    }
}

fn emit_progress(
    app_handle: &AppHandle,
    percentage: f64,
    current_file: impl Into<String>,
    processed: usize,
    total: usize,
) {
    let _ = app_handle.emit(
        "unpack-progress",
        UnpackProgress {
            percentage,
            current_file: current_file.into(),
            processed,
            total,
        },
    );
}

fn create_temp_dir(prefix: &str) -> Result<PathBuf, String> {
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|e| format!("获取时间戳失败: {}", e))?
        .as_millis();
    let pid = std::process::id();
    let dir = std::env::temp_dir().join(format!("vam_box_{}_{}_{}", prefix, pid, timestamp));
    fs::create_dir_all(&dir).map_err(|e| format!("创建临时目录失败: {}", e))?;
    Ok(dir)
}

fn ensure_unique_path(path: &Path) -> PathBuf {
    if !path.exists() {
        return path.to_path_buf();
    }

    let parent = path.parent().unwrap_or_else(|| Path::new("."));
    let stem = path
        .file_stem()
        .and_then(|value| value.to_str())
        .unwrap_or("file");
    let ext = path
        .extension()
        .and_then(|value| value.to_str())
        .unwrap_or("");

    for index in 1..1000 {
        let name = if ext.is_empty() {
            format!("{}_{}", stem, index)
        } else {
            format!("{}_{}.{}", stem, index, ext)
        };
        let candidate = parent.join(name);
        if !candidate.exists() {
            return candidate;
        }
    }

    path.to_path_buf()
}

fn safe_join(base: &Path, relative: &str) -> Result<PathBuf, String> {
    let mut result = base.to_path_buf();
    for component in Path::new(relative).components() {
        match component {
            Component::Normal(value) => result.push(value),
            Component::CurDir => {}
            Component::ParentDir | Component::Prefix(_) | Component::RootDir => {
                return Err(format!("压缩包内存在非法路径: {}", relative));
            }
        }
    }
    Ok(result)
}

fn extract_zip_to_dir(path: &Path, dest_dir: &Path) -> Result<Vec<String>, String> {
    let file = File::open(path).map_err(|e| format!("无法打开 ZIP: {}", e))?;
    let mut archive = ZipArchive::new(file).map_err(|e| format!("读取 ZIP 失败: {}", e))?;
    let mut extracted_files = Vec::new();

    for index in 0..archive.len() {
        let mut entry = archive
            .by_index(index)
            .map_err(|e| format!("读取 ZIP 条目失败: {}", e))?;
        let name = normalize_archive_name(entry.name());
        if name.is_empty() {
            continue;
        }

        let dest_path = safe_join(dest_dir, &name)?;
        if entry.is_dir() {
            fs::create_dir_all(&dest_path).map_err(|e| format!("创建目录失败: {}", e))?;
            continue;
        }

        if let Some(parent) = dest_path.parent() {
            fs::create_dir_all(parent).map_err(|e| format!("创建目录失败: {}", e))?;
        }

        let mut out_file = File::create(&dest_path).map_err(|e| format!("创建文件失败: {}", e))?;
        io::copy(&mut entry, &mut out_file).map_err(|e| format!("写入文件失败: {}", e))?;
        extracted_files.push(name);
    }

    Ok(extracted_files)
}

fn extract_archive_with_tar(path: &Path, dest_dir: &Path) -> Result<(), String> {
    fs::create_dir_all(dest_dir).map_err(|e| format!("创建解压目录失败: {}", e))?;

    let output = Command::new("tar")
        .arg("-xf")
        .arg(path)
        .arg("-C")
        .arg(dest_dir)
        .output()
        .map_err(|e| format!("调用系统解压工具失败: {}", e))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
        let reason = if !stderr.is_empty() { stderr } else { stdout };
        return Err(format!("解压失败: {}", reason));
    }

    Ok(())
}

fn extract_archive_to_dir(
    path: &Path,
    kind: ArchiveKind,
    dest_dir: &Path,
) -> Result<Vec<String>, String> {
    match kind {
        ArchiveKind::Zip => extract_zip_to_dir(path, dest_dir),
        ArchiveKind::SevenZip | ArchiveKind::Rar => {
            extract_archive_with_tar(path, dest_dir)?;
            collect_relative_files(dest_dir, dest_dir)
        }
    }
}

fn collect_relative_files(root: &Path, base: &Path) -> Result<Vec<String>, String> {
    let mut files = Vec::new();
    for entry in WalkDir::new(root) {
        let entry = entry.map_err(|e| format!("遍历目录失败: {}", e))?;
        if !entry.file_type().is_file() {
            continue;
        }

        let relative = entry
            .path()
            .strip_prefix(base)
            .map_err(|e| format!("计算相对路径失败: {}", e))?
            .to_string_lossy()
            .replace('\\', "/");
        files.push(relative);
    }
    files.sort();
    Ok(files)
}

fn copy_tree_contents(source_root: &Path, target_dir: &Path) -> Result<Vec<String>, String> {
    let mut extracted_files = Vec::new();
    for entry in WalkDir::new(source_root) {
        let entry = entry.map_err(|e| format!("遍历目录失败: {}", e))?;
        if !entry.file_type().is_file() {
            continue;
        }

        let relative = entry
            .path()
            .strip_prefix(source_root)
            .map_err(|e| format!("计算相对路径失败: {}", e))?;
        let dest_path = ensure_unique_path(&target_dir.join(relative));

        if let Some(parent) = dest_path.parent() {
            fs::create_dir_all(parent).map_err(|e| format!("创建目标目录失败: {}", e))?;
        }

        fs::copy(entry.path(), &dest_path).map_err(|e| format!("复制文件失败: {}", e))?;
        let relative_name = dest_path
            .strip_prefix(target_dir)
            .map_err(|e| format!("计算输出路径失败: {}", e))?
            .to_string_lossy()
            .replace('\\', "/");
        extracted_files.push(relative_name);
    }

    extracted_files.sort();
    Ok(extracted_files)
}

fn copy_files_by_extension(
    source_root: &Path,
    target_dir: &Path,
    extension: &str,
) -> Result<Vec<String>, String> {
    let mut extracted_files = Vec::new();
    for entry in WalkDir::new(source_root) {
        let entry = entry.map_err(|e| format!("遍历目录失败: {}", e))?;
        if !entry.file_type().is_file() {
            continue;
        }

        let current_ext = entry
            .path()
            .extension()
            .and_then(|value| value.to_str())
            .unwrap_or("")
            .to_ascii_lowercase();
        if current_ext != extension {
            continue;
        }

        let file_name = entry
            .path()
            .file_name()
            .and_then(|value| value.to_str())
            .unwrap_or("file");
        let dest_path = ensure_unique_path(&target_dir.join(file_name));
        fs::copy(entry.path(), &dest_path).map_err(|e| format!("复制文件失败: {}", e))?;

        extracted_files.push(
            dest_path
                .file_name()
                .and_then(|value| value.to_str())
                .unwrap_or("file")
                .to_string(),
        );
    }

    extracted_files.sort();
    Ok(extracted_files)
}

fn resolve_base_root(extracted_root: &Path, base_path_in_archive: &str) -> PathBuf {
    let trimmed = base_path_in_archive.trim_matches('/');
    if trimmed.is_empty() {
        extracted_root.to_path_buf()
    } else {
        extracted_root.join(trimmed.replace('/', std::path::MAIN_SEPARATOR_STR))
    }
}

fn zip_directory_contents(source_root: &Path, output_path: &Path) -> Result<(), String> {
    let file = File::create(output_path).map_err(|e| format!("创建打包文件失败: {}", e))?;
    let mut zip_writer = ZipWriter::new(file);
    let options = FileOptions::<()>::default().compression_method(zip::CompressionMethod::Deflated);
    let mut buffer = vec![0u8; 128 * 1024];

    for entry in WalkDir::new(source_root) {
        let entry = entry.map_err(|e| format!("遍历打包目录失败: {}", e))?;
        let path = entry.path();
        if path == source_root {
            continue;
        }

        let relative = path
            .strip_prefix(source_root)
            .map_err(|e| format!("计算相对路径失败: {}", e))?
            .to_string_lossy()
            .replace('\\', "/");

        if entry.file_type().is_dir() {
            if !relative.is_empty() {
                zip_writer
                    .add_directory(format!("{}/", relative.trim_end_matches('/')), options)
                    .map_err(|e| format!("写入目录条目失败: {}", e))?;
            }
            continue;
        }

        zip_writer
            .start_file(relative, options)
            .map_err(|e| format!("写入文件条目失败: {}", e))?;

        let mut input = File::open(path).map_err(|e| format!("读取待打包文件失败: {}", e))?;
        loop {
            let read_bytes = input
                .read(&mut buffer)
                .map_err(|e| format!("读取待打包文件失败: {}", e))?;
            if read_bytes == 0 {
                break;
            }
            zip_writer
                .write_all(&buffer[..read_bytes])
                .map_err(|e| format!("写入打包内容失败: {}", e))?;
        }
    }

    zip_writer
        .finish()
        .map_err(|e| format!("完成打包失败: {}", e))?;
    Ok(())
}

fn list_visible_children(dir: &Path) -> Result<Vec<PathBuf>, String> {
    let mut children = Vec::new();
    for entry in fs::read_dir(dir).map_err(|e| format!("读取目录失败: {}", e))? {
        let entry = entry.map_err(|e| format!("读取目录项失败: {}", e))?;
        let path = entry.path();
        let name = path
            .file_name()
            .and_then(|value| value.to_str())
            .unwrap_or("");
        if name.eq_ignore_ascii_case("__MACOSX") {
            continue;
        }
        children.push(path);
    }
    children.sort();
    Ok(children)
}

fn rename_archive_to_canonical_extension(
    path: &Path,
    kind: ArchiveKind,
) -> Result<PathBuf, String> {
    let current_ext = lower_extension(path);
    if current_ext == kind.canonical_extension() {
        return Ok(path.to_path_buf());
    }

    let renamed = ensure_unique_path(&path.with_extension(kind.canonical_extension()));
    fs::rename(path, &renamed).map_err(|e| format!("修正伪装后缀失败: {}", e))?;
    Ok(renamed)
}

fn unwrap_single_payload_chain(root: &Path) -> Result<PathBuf, String> {
    let mut current = root.to_path_buf();

    for _ in 0..8 {
        if current.is_file() {
            if let Some(kind) = detect_archive_kind_optional(&current) {
                let normalized_archive = rename_archive_to_canonical_extension(&current, kind)?;
                let next_dir = ensure_unique_path(
                    &normalized_archive
                        .with_file_name(format!(
                            "{}_unpacked",
                            normalized_archive
                                .file_stem()
                                .and_then(OsStr::to_str)
                                .unwrap_or("archive")
                        ))
                        .with_extension(""),
                );
                fs::create_dir_all(&next_dir)
                    .map_err(|e| format!("创建嵌套解压目录失败: {}", e))?;
                extract_archive_to_dir(&normalized_archive, kind, &next_dir)?;
                current = next_dir;
                continue;
            }
            return Ok(current);
        }

        let children = list_visible_children(&current)?;
        if children.len() != 1 {
            return Ok(current);
        }

        let only_child = children[0].clone();
        if only_child.is_dir() {
            current = only_child;
            continue;
        }

        if let Some(kind) = detect_archive_kind_optional(&only_child) {
            let normalized_archive = rename_archive_to_canonical_extension(&only_child, kind)?;
            let next_dir = ensure_unique_path(
                &normalized_archive
                    .with_file_name(format!(
                        "{}_unpacked",
                        normalized_archive
                            .file_stem()
                            .and_then(OsStr::to_str)
                            .unwrap_or("archive")
                    ))
                    .with_extension(""),
            );
            fs::create_dir_all(&next_dir).map_err(|e| format!("创建嵌套解压目录失败: {}", e))?;
            extract_archive_to_dir(&normalized_archive, kind, &next_dir)?;
            current = next_dir;
            continue;
        }

        return Ok(only_child);
    }

    Ok(current)
}

fn summarize_output(target: &Path) -> Result<Vec<String>, String> {
    if target.is_file() {
        return Ok(vec![target
            .file_name()
            .and_then(|value| value.to_str())
            .unwrap_or("file")
            .to_string()]);
    }
    collect_relative_files(target, target)
}

fn cleanup_temp_dir(path: &Path) {
    let _ = fs::remove_dir_all(path);
}

#[tauri::command]
pub async fn analyze_archive(archive_path: String) -> Result<ArchiveAnalysis, String> {
    let path = Path::new(&archive_path);
    if !path.exists() {
        return Err(format!("文件不存在: {}", archive_path));
    }

    let file_name = path
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or("unknown")
        .to_string();
    let size_bytes = fs::metadata(path)
        .map_err(|e| format!("读取文件信息失败: {}", e))?
        .len();
    let kind = detect_archive_kind(path)?;
    let entries = list_archive_entries(path, kind)?;

    Ok(analyze_entries(
        archive_path,
        file_name,
        size_bytes,
        kind,
        &entries,
    ))
}

fn execute_rename_to_var(
    app_handle: &AppHandle,
    analysis: &ArchiveAnalysis,
    target_dir: &Path,
    kind: ArchiveKind,
) -> Result<UnpackResult, String> {
    let target_var_name = if let Some(var_id) = &analysis.suspected_var_id {
        format!("{}.var", var_id)
    } else {
        let stem = Path::new(&analysis.file_name)
            .file_stem()
            .and_then(|value| value.to_str())
            .unwrap_or("package");
        format!("{}.var", stem)
    };

    let source_path = Path::new(&analysis.file_path);
    let preferred_destination = target_dir.join(&target_var_name);

    if kind == ArchiveKind::Zip
        && lower_extension(source_path) == "var"
        && source_path == preferred_destination
    {
        return Ok(UnpackResult {
            success: true,
            message: format!("已识别为标准 VAR 包: {}", target_var_name),
            extracted_files: vec![target_var_name],
            destination_path: analysis.file_path.clone(),
        });
    }

    let destination_var_path = ensure_unique_path(&preferred_destination);

    if kind == ArchiveKind::Zip && analysis.base_path_in_archive.is_empty() {
        emit_progress(app_handle, 20.0, &target_var_name, 0, 1);
        fs::copy(source_path, &destination_var_path)
            .map_err(|e| format!("复制 VAR 文件失败: {}", e))?;
        emit_progress(app_handle, 100.0, &target_var_name, 1, 1);

        return Ok(UnpackResult {
            success: true,
            message: format!("已成功重命名并输出为 VAR 包: {}", target_var_name),
            extracted_files: vec![target_var_name],
            destination_path: destination_var_path.to_string_lossy().to_string(),
        });
    }

    let temp_dir = create_temp_dir("repack")?;
    emit_progress(app_handle, 10.0, "正在解压原始压缩包", 0, 3);
    let extract_result = extract_archive_to_dir(source_path, kind, &temp_dir);
    if let Err(err) = extract_result {
        cleanup_temp_dir(&temp_dir);
        return Err(err);
    }

    let source_root = resolve_base_root(&temp_dir, &analysis.base_path_in_archive);
    if !source_root.exists() {
        cleanup_temp_dir(&temp_dir);
        return Err("未找到可重新打包的 VAM 包根目录".to_string());
    }

    emit_progress(app_handle, 65.0, "正在重新封装为 .var", 1, 3);
    let zip_result = zip_directory_contents(&source_root, &destination_var_path);
    cleanup_temp_dir(&temp_dir);
    zip_result?;
    emit_progress(app_handle, 100.0, &target_var_name, 3, 3);

    Ok(UnpackResult {
        success: true,
        message: format!("已成功转换并输出为 VAR 包: {}", target_var_name),
        extracted_files: vec![target_var_name],
        destination_path: destination_var_path.to_string_lossy().to_string(),
    })
}

fn execute_extract_vars(
    app_handle: &AppHandle,
    analysis: &ArchiveAnalysis,
    target_dir: &Path,
    kind: ArchiveKind,
) -> Result<UnpackResult, String> {
    let temp_dir = create_temp_dir("extract_vars")?;
    emit_progress(app_handle, 10.0, "正在解压压缩包", 0, 2);
    let extract_result = extract_archive_to_dir(Path::new(&analysis.file_path), kind, &temp_dir);
    if let Err(err) = extract_result {
        cleanup_temp_dir(&temp_dir);
        return Err(err);
    }

    let mut extracted_files = Vec::new();
    for entry in WalkDir::new(&temp_dir) {
        let entry = entry.map_err(|e| format!("遍历临时目录失败: {}", e))?;
        if !entry.file_type().is_file() {
            continue;
        }

        let ext = entry
            .path()
            .extension()
            .and_then(|value| value.to_str())
            .unwrap_or("")
            .to_ascii_lowercase();
        if ext != "var" {
            continue;
        }

        let file_name = entry
            .path()
            .file_name()
            .and_then(|value| value.to_str())
            .unwrap_or("package.var");
        let dest_path = ensure_unique_path(&target_dir.join(file_name));
        fs::copy(entry.path(), &dest_path).map_err(|e| format!("复制 VAR 文件失败: {}", e))?;
        extracted_files.push(
            dest_path
                .file_name()
                .and_then(|value| value.to_str())
                .unwrap_or("package.var")
                .to_string(),
        );
    }

    cleanup_temp_dir(&temp_dir);
    extracted_files.sort();
    emit_progress(
        app_handle,
        100.0,
        "完成解压",
        extracted_files.len(),
        extracted_files.len(),
    );

    Ok(UnpackResult {
        success: true,
        message: format!("已成功提取 {} 个 VAR 包", extracted_files.len()),
        extracted_files,
        destination_path: target_dir.to_string_lossy().to_string(),
    })
}

fn execute_extract_content(
    app_handle: &AppHandle,
    analysis: &ArchiveAnalysis,
    target_dir: &Path,
    kind: ArchiveKind,
) -> Result<UnpackResult, String> {
    let temp_dir = create_temp_dir("extract_content")?;
    emit_progress(app_handle, 10.0, "正在解压压缩包", 0, 2);
    let extract_result = extract_archive_to_dir(Path::new(&analysis.file_path), kind, &temp_dir);
    if let Err(err) = extract_result {
        cleanup_temp_dir(&temp_dir);
        return Err(err);
    }

    let source_root = resolve_base_root(&temp_dir, &analysis.base_path_in_archive);
    if !source_root.exists() {
        cleanup_temp_dir(&temp_dir);
        return Err("未找到可提取的资源根目录".to_string());
    }

    emit_progress(app_handle, 70.0, "正在整理标准目录结构", 1, 2);
    let copy_result = copy_tree_contents(&source_root, target_dir);
    cleanup_temp_dir(&temp_dir);
    let extracted_files = copy_result?;
    emit_progress(
        app_handle,
        100.0,
        "完成解压",
        extracted_files.len(),
        extracted_files.len(),
    );

    Ok(UnpackResult {
        success: true,
        message: format!("成功整理并提取 {} 个资源文件", extracted_files.len()),
        extracted_files,
        destination_path: target_dir.to_string_lossy().to_string(),
    })
}

fn execute_extract_flat(
    app_handle: &AppHandle,
    analysis: &ArchiveAnalysis,
    target_dir: &Path,
    kind: ArchiveKind,
    extension: &str,
    label: &str,
) -> Result<UnpackResult, String> {
    let temp_dir = create_temp_dir("extract_flat")?;
    emit_progress(app_handle, 10.0, "正在解压压缩包", 0, 2);
    let extract_result = extract_archive_to_dir(Path::new(&analysis.file_path), kind, &temp_dir);
    if let Err(err) = extract_result {
        cleanup_temp_dir(&temp_dir);
        return Err(err);
    }

    emit_progress(app_handle, 70.0, format!("正在整理{}", label), 1, 2);
    let copy_result = copy_files_by_extension(&temp_dir, target_dir, extension);
    cleanup_temp_dir(&temp_dir);
    let extracted_files = copy_result?;
    emit_progress(
        app_handle,
        100.0,
        "完成解压",
        extracted_files.len(),
        extracted_files.len(),
    );

    Ok(UnpackResult {
        success: true,
        message: format!("已成功提取 {} 个{}文件", extracted_files.len(), label),
        extracted_files,
        destination_path: target_dir.to_string_lossy().to_string(),
    })
}

fn execute_extract_to_temp(
    app_handle: &AppHandle,
    analysis: &ArchiveAnalysis,
    target_dir: &Path,
    kind: ArchiveKind,
) -> Result<UnpackResult, String> {
    let stem = Path::new(&analysis.file_name)
        .file_stem()
        .and_then(|value| value.to_str())
        .unwrap_or("unpacked_archive");
    let temp_dest = ensure_unique_path(&target_dir.join(format!("{}_unpacked", stem)));
    fs::create_dir_all(&temp_dest).map_err(|e| format!("创建目标目录失败: {}", e))?;

    emit_progress(app_handle, 10.0, "正在解压外层压缩包", 0, 3);
    extract_archive_to_dir(Path::new(&analysis.file_path), kind, &temp_dest)?;

    emit_progress(app_handle, 55.0, "正在识别嵌套压缩包", 1, 3);
    let final_target = unwrap_single_payload_chain(&temp_dest)?;

    emit_progress(app_handle, 90.0, "正在整理最终输出", 2, 3);
    let extracted_files = summarize_output(&final_target)?;
    emit_progress(
        app_handle,
        100.0,
        "完成解压",
        extracted_files.len(),
        extracted_files.len(),
    );

    let message = if final_target != temp_dest {
        format!(
            "已自动继续解压嵌套压缩包，并定位到真实根内容，共输出 {} 个文件",
            extracted_files.len()
        )
    } else {
        format!("已成功解压 {} 个文件到独立目录", extracted_files.len())
    };

    Ok(UnpackResult {
        success: true,
        message,
        extracted_files,
        destination_path: final_target.to_string_lossy().to_string(),
    })
}

#[tauri::command]
pub async fn execute_unpack(
    app_handle: AppHandle,
    analysis: ArchiveAnalysis,
    custom_target_dir: Option<String>,
) -> Result<UnpackResult, String> {
    let target_dir = if let Some(custom_dir) = custom_target_dir {
        PathBuf::from(custom_dir)
    } else {
        Path::new(&analysis.file_path)
            .parent()
            .ok_or_else(|| "无法获取压缩包所在目录".to_string())?
            .to_path_buf()
    };

    fs::create_dir_all(&target_dir).map_err(|e| format!("创建目标文件夹失败: {}", e))?;
    let archive_kind = detect_archive_kind(Path::new(&analysis.file_path))?;

    match analysis.recommended_action.as_str() {
        "rename_to_var" => execute_rename_to_var(&app_handle, &analysis, &target_dir, archive_kind),
        "extract_vars" => execute_extract_vars(&app_handle, &analysis, &target_dir, archive_kind),
        "extract_content" => {
            execute_extract_content(&app_handle, &analysis, &target_dir, archive_kind)
        }
        "extract_flat_appearance" => execute_extract_flat(
            &app_handle,
            &analysis,
            &target_dir,
            archive_kind,
            "vac",
            "外观预设",
        ),
        "extract_flat_scene" => execute_extract_flat(
            &app_handle,
            &analysis,
            &target_dir,
            archive_kind,
            "json",
            "场景预设",
        ),
        "extract_to_temp" | _ => {
            execute_extract_to_temp(&app_handle, &analysis, &target_dir, archive_kind)
        }
    }
}
