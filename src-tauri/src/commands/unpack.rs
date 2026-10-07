use serde::{Deserialize, Serialize};
use std::ffi::OsStr;
use std::fs::{self, File};
use std::io::{self, Read};
use std::path::{Component, Path, PathBuf};
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};
use tauri::{AppHandle, Emitter};
use walkdir::WalkDir;
use zip::ZipArchive;

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
    pub nested_archives: Vec<String>,
    pub disguised_archive_files: Vec<String>,
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

    fn label(self) -> &'static str {
        match self {
            Self::Zip => "ZIP",
            Self::SevenZip => "7Z",
            Self::Rar => "RAR",
        }
    }
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

fn archive_kind_from_extension(path: &Path) -> Option<ArchiveKind> {
    match lower_extension(path).as_str() {
        "zip" => Some(ArchiveKind::Zip),
        "7z" => Some(ArchiveKind::SevenZip),
        "rar" => Some(ArchiveKind::Rar),
        _ => None,
    }
}

fn detect_archive_kind(path: &Path) -> Result<ArchiveKind, String> {
    if !path.exists() {
        return Err(format!("文件不存在: {}", path.display()));
    }

    let header = read_header(path, 8)?;
    if let Some(kind) = archive_kind_from_header(&header) {
        return Ok(kind);
    }

    archive_kind_from_extension(path).ok_or_else(|| {
        "暂不支持该文件，支持 rar / zip / 7z，或可识别为伪装压缩包的文件".to_string()
    })
}

fn detect_archive_kind_optional(path: &Path) -> Option<ArchiveKind> {
    detect_archive_kind(path).ok()
}

fn has_disguised_extension(path: &Path, kind: ArchiveKind) -> bool {
    lower_extension(path) != kind.canonical_extension()
}

fn normalize_archive_name(name: &str) -> String {
    name.replace('\\', "/").trim_start_matches("./").to_string()
}

fn looks_like_archive_path(path: &str) -> bool {
    matches!(
        Path::new(path)
            .extension()
            .and_then(|ext| ext.to_str())
            .unwrap_or("")
            .to_ascii_lowercase()
            .as_str(),
        "zip" | "7z" | "rar" | "jpg" | "jpeg" | "png" | "webp" | "bmp" | "gif" | "ico"
    )
}

fn is_image_or_icon_path(path: &str) -> bool {
    matches!(
        Path::new(path)
            .extension()
            .and_then(|ext| ext.to_str())
            .unwrap_or("")
            .to_ascii_lowercase()
            .as_str(),
        "jpg" | "jpeg" | "png" | "webp" | "bmp" | "gif" | "ico"
    )
}

fn list_archive_entries(path: &Path, kind: ArchiveKind) -> Result<Vec<ArchiveEntryInfo>, String> {
    match kind {
        ArchiveKind::Zip => list_zip_entries(path),
        ArchiveKind::SevenZip | ArchiveKind::Rar => list_archive_entries_with_external_tool(path),
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

fn list_archive_entries_with_external_tool(path: &Path) -> Result<Vec<ArchiveEntryInfo>, String> {
    let output = Command::new("tar")
        .arg("-tf")
        .arg(path)
        .output()
        .map_err(|e| format!("调用系统解压工具失败: {}", e))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
        let reason = if !stderr.is_empty() { stderr } else { stdout };
        return list_archive_entries_with_7z(path)
            .map_err(|fallback| format!("读取压缩包目录失败: {}; {}", reason, fallback));
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

fn list_archive_entries_with_7z(path: &Path) -> Result<Vec<ArchiveEntryInfo>, String> {
    let mut last_error = String::new();

    for command_name in ["7z", "7zz", "7za"] {
        let output = match Command::new(command_name)
            .arg("l")
            .arg("-slt")
            .arg(path)
            .output()
        {
            Ok(value) => value,
            Err(err) => {
                last_error = format!("{} 不可用: {}", command_name, err);
                continue;
            }
        };

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
            let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
            last_error = if !stderr.is_empty() { stderr } else { stdout };
            continue;
        }

        let mut entries = Vec::new();
        let mut current_path = None::<String>;
        let mut current_is_dir = false;

        for line in String::from_utf8_lossy(&output.stdout).lines() {
            if let Some(value) = line.strip_prefix("Path = ") {
                if let Some(name) = current_path.take() {
                    if name != path.to_string_lossy().as_ref() {
                        entries.push(ArchiveEntryInfo {
                            name: normalize_archive_name(&name),
                            is_file: !current_is_dir,
                        });
                    }
                }
                current_path = Some(value.trim().to_string());
                current_is_dir = false;
                continue;
            }

            if let Some(value) = line.strip_prefix("Folder = ") {
                current_is_dir = value.trim() == "+";
            }
        }

        if let Some(name) = current_path.take() {
            if name != path.to_string_lossy().as_ref() {
                entries.push(ArchiveEntryInfo {
                    name: normalize_archive_name(&name),
                    is_file: !current_is_dir,
                });
            }
        }

        return Ok(entries);
    }

    Err(last_error)
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
    let mut nested_archives = Vec::new();
    let mut disguised_archive_files = Vec::new();
    let mut has_image_files = false;
    let mut image_files = Vec::new();

    if has_disguised_extension(Path::new(&archive_path), kind) {
        disguised_archive_files.push(file_name.clone());
    }

    for entry in entries {
        if !entry.is_file {
            continue;
        }

        file_count += 1;
        if sample_files.len() < 12 {
            sample_files.push(entry.name.clone());
        }

        if looks_like_archive_path(&entry.name) && nested_archives.len() < 12 {
            nested_archives.push(entry.name.clone());
        }

        if is_image_or_icon_path(&entry.name) {
            has_image_files = true;
            if image_files.len() < 5 {
                image_files.push(entry.name.clone());
            }
        }
    }

    let archive_type = if !nested_archives.is_empty() || !disguised_archive_files.is_empty() {
        "recursive_archive"
    } else {
        "plain_archive"
    };

    ArchiveAnalysis {
        file_path: archive_path,
        file_name,
        size_bytes,
        archive_type: archive_type.to_string(),
        recommended_action: "recursive_extract".to_string(),
        file_count,
        sample_files,
        contains_meta_json: false,
        contains_var_files: false,
        var_files: Vec::new(),
        has_image_files,
        image_files,
        suspected_var_id: None,
        base_path_in_archive: String::new(),
        nested_archives,
        disguised_archive_files,
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

struct UnpackTempDir(PathBuf);

impl Drop for UnpackTempDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn create_temp_dir(prefix: &str) -> Result<UnpackTempDir, String> {
    static SEQUENCE: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|e| format!("获取时间戳失败: {}", e))?
        .as_millis();
    let pid = std::process::id();
    let sequence = SEQUENCE.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let dir = std::env::temp_dir().join(format!(
        "vam_box_{}_{}_{}_{}",
        prefix, pid, timestamp, sequence
    ));
    fs::create_dir(&dir).map_err(|e| format!("创建临时目录失败: {}", e))?;
    Ok(UnpackTempDir(dir))
}

fn ensure_unique_path(path: &Path) -> Result<PathBuf, String> {
    if !path.exists() {
        return Ok(path.to_path_buf());
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
            return Ok(candidate);
        }
    }

    Err(format!(
        "目标重名文件过多，未覆盖已有文件: {}",
        path.display()
    ))
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

fn extract_archive_with_external_tool(path: &Path, dest_dir: &Path) -> Result<(), String> {
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
        return extract_archive_with_7z(path, dest_dir)
            .map_err(|fallback| format!("解压失败: {}; {}", reason, fallback));
    }

    Ok(())
}

fn extract_archive_with_7z(path: &Path, dest_dir: &Path) -> Result<(), String> {
    let mut last_error = String::new();
    let output_arg = format!("-o{}", dest_dir.to_string_lossy());

    for command_name in ["7z", "7zz", "7za"] {
        let output = match Command::new(command_name)
            .arg("x")
            .arg("-y")
            .arg(&output_arg)
            .arg(path)
            .output()
        {
            Ok(value) => value,
            Err(err) => {
                last_error = format!("{} 不可用: {}", command_name, err);
                continue;
            }
        };

        if output.status.success() {
            return Ok(());
        }

        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
        last_error = if !stderr.is_empty() { stderr } else { stdout };
    }

    Err(last_error)
}

fn extract_archive_to_dir(
    path: &Path,
    kind: ArchiveKind,
    dest_dir: &Path,
) -> Result<Vec<String>, String> {
    match kind {
        ArchiveKind::Zip => extract_zip_to_dir(path, dest_dir),
        ArchiveKind::SevenZip | ArchiveKind::Rar => {
            extract_archive_with_external_tool(path, dest_dir)?;
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

    let renamed = ensure_unique_path(&path.with_extension(kind.canonical_extension()))?;
    crate::services::resource_files::transfer(path, &renamed, "move")
        .map_err(|e| format!("修正伪装后缀失败: {}", e))?;
    Ok(renamed)
}

fn collect_nested_archive_files(root: &Path) -> Result<Vec<PathBuf>, String> {
    let mut archive_files = Vec::new();
    for entry in WalkDir::new(root) {
        let entry = entry.map_err(|e| format!("遍历目录失败: {}", e))?;
        if !entry.file_type().is_file() {
            continue;
        }

        if detect_archive_kind_optional(entry.path()).is_some() {
            archive_files.push(entry.path().to_path_buf());
        }
    }
    archive_files.sort();
    Ok(archive_files)
}

fn unpack_nested_archives(
    app_handle: &AppHandle,
    root: &Path,
    max_depth: usize,
) -> Result<usize, String> {
    let mut unpacked_count = 0usize;

    for depth in 0..max_depth {
        let archive_files = collect_nested_archive_files(root)?;
        if archive_files.is_empty() {
            return Ok(unpacked_count);
        }

        let total = archive_files.len();
        for (index, archive_path) in archive_files.iter().enumerate() {
            if !archive_path.exists() {
                continue;
            }

            let kind = detect_archive_kind(archive_path)?;
            let normalized_archive = rename_archive_to_canonical_extension(archive_path, kind)?;
            let stem = normalized_archive
                .file_stem()
                .and_then(OsStr::to_str)
                .unwrap_or("archive");
            let dest_dir =
                ensure_unique_path(&normalized_archive.parent().unwrap_or(root).join(stem))?;

            emit_progress(
                app_handle,
                35.0 + ((depth as f64 + (index as f64 / total as f64)) * 45.0 / max_depth as f64),
                format!(
                    "继续解压 {}: {}",
                    kind.label(),
                    normalized_archive.display()
                ),
                index,
                total,
            );

            fs::create_dir_all(&dest_dir).map_err(|e| format!("创建嵌套解压目录失败: {}", e))?;
            extract_archive_to_dir(&normalized_archive, kind, &dest_dir)?;
            fs::remove_file(&normalized_archive)
                .map_err(|e| format!("清理中间压缩包失败: {}", e))?;
            unpacked_count += 1;
        }
    }

    Err(format!(
        "嵌套层级超过 {} 层，已停止以避免循环解压",
        max_depth
    ))
}

fn collapse_single_folder_chain(root: &Path) -> Result<PathBuf, String> {
    let mut current = root.to_path_buf();

    for _ in 0..16 {
        if !current.is_dir() {
            return Ok(current);
        }

        let children = list_visible_children(&current)?;
        if children.len() != 1 || !children[0].is_dir() {
            return Ok(current);
        }

        current = children[0].clone();
    }

    Ok(current)
}

fn copy_file_to_target(source: &Path, target_dir: &Path) -> Result<PathBuf, String> {
    let file_name = source
        .file_name()
        .and_then(OsStr::to_str)
        .unwrap_or("unpacked_file");
    let dest_path = ensure_unique_path(&target_dir.join(file_name))?;
    crate::services::resource_files::transfer(source, &dest_path, "copy")
        .map_err(|e| format!("复制文件失败: {}", e))?;
    Ok(dest_path)
}

fn copy_dir_to_target(
    source_root: &Path,
    target_dir: &Path,
    preferred_name: &str,
) -> Result<PathBuf, String> {
    let dest_root = ensure_unique_path(&target_dir.join(preferred_name))?;
    fs::create_dir(&dest_root).map_err(|e| format!("创建目标目录失败: {}", e))?;

    for entry in WalkDir::new(source_root) {
        let entry = entry.map_err(|e| format!("遍历目录失败: {}", e))?;
        let relative = entry
            .path()
            .strip_prefix(source_root)
            .map_err(|e| format!("计算相对路径失败: {}", e))?;
        if relative.as_os_str().is_empty() {
            continue;
        }

        let dest_path = dest_root.join(relative);
        if entry.file_type().is_dir() {
            fs::create_dir_all(&dest_path).map_err(|e| format!("创建目录失败: {}", e))?;
            continue;
        }

        if let Some(parent) = dest_path.parent() {
            fs::create_dir_all(parent).map_err(|e| format!("创建目录失败: {}", e))?;
        }
        crate::services::resource_files::transfer(entry.path(), &dest_path, "copy")
            .map_err(|e| format!("复制文件失败: {}", e))?;
    }

    Ok(dest_root)
}

#[tauri::command]
pub async fn analyze_archive(archive_path: String) -> Result<ArchiveAnalysis, String> {
    tauri::async_runtime::spawn_blocking(move || {
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
    })
    .await
    .map_err(|error| error.to_string())?
}

fn execute_recursive_extract(
    app_handle: &AppHandle,
    analysis: &ArchiveAnalysis,
    target_dir: &Path,
    kind: ArchiveKind,
) -> Result<UnpackResult, String> {
    let source_path = Path::new(&analysis.file_path);
    let temp_dir = create_temp_dir("recursive_unpack")?;
    let stem = Path::new(&analysis.file_name)
        .file_stem()
        .and_then(OsStr::to_str)
        .unwrap_or("unpacked_archive");

    emit_progress(app_handle, 10.0, "正在解压外层压缩包", 0, 3);
    extract_archive_to_dir(source_path, kind, &temp_dir.0)?;

    emit_progress(app_handle, 30.0, "正在查找嵌套压缩包和伪装后缀", 1, 3);
    let nested_count = unpack_nested_archives(app_handle, &temp_dir.0, 16)?;

    emit_progress(app_handle, 82.0, "正在整理最终输出", 2, 3);
    let final_root = collapse_single_folder_chain(&temp_dir.0)?;
    let output_path = if final_root.is_file() {
        copy_file_to_target(&final_root, target_dir)?
    } else {
        let preferred_name = final_root
            .file_name()
            .and_then(OsStr::to_str)
            .filter(|name| !name.starts_with("vam_box_recursive_unpack_"))
            .unwrap_or(stem);
        copy_dir_to_target(&final_root, target_dir, preferred_name)?
    };

    drop(temp_dir);

    let extracted_files = if output_path.is_file() {
        vec![output_path
            .file_name()
            .and_then(OsStr::to_str)
            .unwrap_or("unpacked_file")
            .to_string()]
    } else {
        collect_relative_files(&output_path, &output_path)?
    };

    emit_progress(
        app_handle,
        100.0,
        "完成解压",
        extracted_files.len(),
        extracted_files.len(),
    );

    let message = if nested_count > 0 {
        format!(
            "已完成通用递归解压，额外解开 {} 个嵌套或伪装压缩包，最终输出 {} 个文件",
            nested_count,
            extracted_files.len()
        )
    } else {
        format!("已完成解压，最终输出 {} 个文件", extracted_files.len())
    };

    Ok(UnpackResult {
        success: true,
        message,
        extracted_files,
        destination_path: output_path.to_string_lossy().to_string(),
    })
}

#[tauri::command]
pub async fn execute_unpack(
    app_handle: AppHandle,
    analysis: ArchiveAnalysis,
    custom_target_dir: Option<String>,
) -> Result<UnpackResult, String> {
    tauri::async_runtime::spawn_blocking(move || {
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
        execute_recursive_extract(&app_handle, &analysis, &target_dir, archive_kind)
    })
    .await
    .map_err(|error| error.to_string())?
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::services::resource_files::tests::TestDir;

    #[test]
    fn exhausted_collision_names_never_overwrite_originals() {
        let root = TestDir::new();
        let source = TestDir::new();
        let file = source.write("asset.txt", b"new");
        root.write("asset.txt", b"original");
        for i in 1..1000 {
            root.write(&format!("asset_{i}.txt"), b"keep");
        }
        assert!(copy_file_to_target(&file, &root.0).is_err());
        assert_eq!(fs::read(root.0.join("asset.txt")).unwrap(), b"original");
        assert_eq!(fs::read(file).unwrap(), b"new");
    }

    #[test]
    fn temporary_directories_are_unique_and_removed_on_early_error() {
        let first = create_temp_dir("test").unwrap();
        let second = create_temp_dir("test").unwrap();
        assert_ne!(first.0, second.0);
        let path = first.0.clone();
        fn fail(_owned: UnpackTempDir) -> Result<(), String> {
            Err("copy failed".into())
        }
        let failure = fail(first);
        assert!(failure.is_err());
        assert!(!path.exists());
        assert!(second.0.exists());
    }

    #[test]
    fn archive_paths_and_copy_collisions_keep_destination_contained() {
        let source = TestDir::new();
        let target = TestDir::new();
        assert!(safe_join(&target.0, "../escape").is_err());
        assert!(safe_join(&target.0, "/escape").is_err());
        let original = source.write("asset.txt", b"new");
        target.write("asset.txt", b"old");
        let copied = copy_file_to_target(&original, &target.0).unwrap();
        assert_eq!(copied, target.0.join("asset_1.txt"));
        assert_eq!(fs::read(target.0.join("asset.txt")).unwrap(), b"old");
        assert_eq!(fs::read(copied).unwrap(), b"new");
    }
}
