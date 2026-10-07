use serde::{Deserialize, Serialize};
use std::collections::{HashSet, VecDeque};
use std::fs::File;
use std::io::{Read, Write};
use std::path::Path;
use tauri::{AppHandle, Emitter, State};
use zip::write::{FileOptions, ZipWriter};

use crate::db::Database;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SharePreview {
    pub package_id: String,
    pub dependencies: Vec<ShareDependencyNode>,
    pub missing_dependencies: Vec<String>,
    pub total_size_bytes: u64,
    pub total_files: usize,
    pub excluded_count: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShareDependencyNode {
    pub id: String,
    pub creator: String,
    pub name: String,
    pub version: i32,
    pub size_bytes: u64,
    pub file_path: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct ShareProgress {
    pub phase: String, // "zipping" | "done" | "error"
    pub current_file: String,
    pub processed_files: usize,
    pub total_files: usize,
    pub percentage: f64,
}

fn parse_package_id(package_id: &str) -> (String, String, i32) {
    let trimmed = package_id.trim();
    let parts: Vec<&str> = trimmed.split('.').collect();
    if parts.len() < 2 {
        return (String::new(), trimmed.to_lowercase(), 0);
    }

    let last = parts.last().copied().unwrap_or_default();
    if let Ok(version) = last.parse::<i32>() {
        let creator = parts[0].to_lowercase();
        let name = parts[1..parts.len() - 1].join(".").to_lowercase();
        (creator, name, version)
    } else if last.eq_ignore_ascii_case("latest") {
        let creator = parts[0].to_lowercase();
        let name = parts[1..parts.len() - 1].join(".").to_lowercase();
        (creator, name, i32::MAX)
    } else {
        let creator = parts[0].to_lowercase();
        let name = parts[1..].join(".").to_lowercase();
        (creator, name, i32::MAX)
    }
}

fn load_exclude_list(
    path_str: &str,
) -> Result<std::collections::HashMap<(String, String), i32>, String> {
    let path = Path::new(path_str);
    if !path.exists() {
        return Err(format!("排除列表文件不存在: {}", path_str));
    }

    let mut file = File::open(path).map_err(|e| format!("打开排除列表文件失败: {}", e))?;
    let mut contents = String::new();
    file.read_to_string(&mut contents)
        .map_err(|e| format!("读取排除列表文件失败: {}", e))?;

    let mut exclude_map = std::collections::HashMap::new();

    for line in contents.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }

        let (creator, name, version) = parse_package_id(trimmed);
        if creator.is_empty() && name.is_empty() {
            continue;
        }

        let key = (creator, name);
        let entry = exclude_map.entry(key).or_insert(version);
        if version > *entry {
            *entry = version;
        }
    }

    Ok(exclude_map)
}

/// 导出所有本地已安装包的 ID 列表为一个 TXT 文件
#[tauri::command]
pub async fn export_installed_packages(
    db: State<'_, Database>,
    target_path: String,
) -> Result<(), String> {
    db.with_conn(|conn| {
        let installed = load_installed_packages(conn).map_err(|e| e.to_string())?;

        let mut file = File::create(&target_path).map_err(|e| format!("创建文件失败: {}", e))?;

        for pkg in installed {
            writeln!(file, "{}", pkg.id).map_err(|e| format!("写入文件失败: {}", e))?;
        }

        Ok(())
    })
    .map_err(|e| e.to_string())
}

/// 递归计算一键分享预览，按本地已安装版本解析依赖。
#[tauri::command]
pub async fn get_share_preview(
    db: State<'_, Database>,
    package_id: String,
    exclude_list_path: Option<String>,
) -> Result<SharePreview, String> {
    let exclude_map = if let Some(ref path) = exclude_list_path {
        if !path.trim().is_empty() {
            Some(load_exclude_list(path)?)
        } else {
            None
        }
    } else {
        None
    };

    db.with_conn(|conn| {
        let installed_packages = load_installed_packages(conn).map_err(|e| e.to_string())?;
        let dependency_pairs = load_dependency_pairs(conn).map_err(|e| e.to_string())?;

        let Some(root_id) = resolve_installed_dependency_id(&package_id, &installed_packages)
        else {
            return Ok(SharePreview {
                package_id: package_id.clone(),
                dependencies: Vec::new(),
                missing_dependencies: vec![package_id],
                total_size_bytes: 0,
                total_files: 0,
                excluded_count: 0,
            });
        };

        let mut selected = HashSet::from([root_id.clone()]);
        let mut missing = HashSet::new();
        let mut queue = VecDeque::from([root_id]);

        while let Some(current) = queue.pop_front() {
            for (_, dep_id) in dependency_pairs
                .iter()
                .filter(|(source, _)| source.eq_ignore_ascii_case(&current))
            {
                let Some(resolved_dep_id) =
                    resolve_installed_dependency_id(dep_id, &installed_packages)
                else {
                    missing.insert(dep_id.clone());
                    continue;
                };

                if selected.insert(resolved_dep_id.clone()) {
                    queue.push_back(resolved_dep_id);
                }
            }
        }

        let mut excluded_count = 0;
        let mut dependencies: Vec<ShareDependencyNode> = Vec::new();

        for pkg in installed_packages {
            if !selected.contains(&pkg.id) {
                continue;
            }
            if let Some(ref map) = exclude_map {
                let (creator, name, version) = parse_package_id(&pkg.id);
                if map
                    .get(&(creator, name))
                    .is_some_and(|&excluded| version <= excluded)
                {
                    excluded_count += 1;
                    continue;
                }
            }
            dependencies.push(pkg);
        }
        dependencies.sort_by(|a, b| a.id.cmp(&b.id));

        let total_size_bytes = dependencies.iter().map(|pkg| pkg.size_bytes).sum();
        let total_files = dependencies.len();
        let mut missing_dependencies: Vec<String> = missing.into_iter().collect();
        missing_dependencies.sort();

        Ok(SharePreview {
            package_id,
            dependencies,
            missing_dependencies,
            total_size_bytes,
            total_files,
            excluded_count,
        })
    })
    .map_err(|e| e.to_string())
}

fn load_installed_packages(conn: &rusqlite::Connection) -> rusqlite::Result<Vec<ShareDependencyNode>> {
    let mut stmt =
        conn.prepare("SELECT id, creator, name, version, size_bytes, file_path FROM packages")?;
    let packages = stmt
        .query_map([], |row| {
            Ok(ShareDependencyNode {
                id: row.get(0)?,
                creator: row.get(1)?,
                name: row.get(2)?,
                version: row.get(3)?,
                size_bytes: row.get::<_, i64>(4)? as u64,
                file_path: row.get(5)?,
            })
        })?
        .collect();
    packages
}

fn load_dependency_pairs(conn: &rusqlite::Connection) -> rusqlite::Result<Vec<(String, String)>> {
    let mut stmt = conn.prepare("SELECT package_id, depends_on_id FROM dependencies")?;
    let pairs = stmt
        .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))?
        .collect();
    pairs
}

fn resolve_installed_dependency_id(
    depends_on_id: &str,
    installed_packages: &[ShareDependencyNode],
) -> Option<String> {
    if let Some(pkg) = installed_packages
        .iter()
        .find(|pkg| pkg.id.eq_ignore_ascii_case(depends_on_id))
    {
        return Some(pkg.id.clone());
    }

    let (creator, name, required_version) =
        crate::services::dependencies::parse_dependency_parts(depends_on_id)?;
    installed_packages
        .iter()
        .filter(|pkg| {
            pkg.creator.eq_ignore_ascii_case(&creator)
                && pkg.name.eq_ignore_ascii_case(&name)
                && required_version.map_or(true, |required| pkg.version >= required)
        })
        .max_by_key(|pkg| pkg.version)
        .map(|pkg| pkg.id.clone())
}

/// Helper function to perform zipping in a background thread
fn do_export_zip(
    app: AppHandle,
    preview: SharePreview,
    target_zip_path: String,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    use std::fs;

    let target_path = Path::new(&target_zip_path);
    if let Some(parent) = target_path.parent() {
        fs::create_dir_all(parent)?;
    }

    let file = File::create(target_path)?;
    let mut zip = ZipWriter::new(file);
    let options = FileOptions::<()>::default().compression_method(zip::CompressionMethod::Stored); // VARs are already compressed

    let total_files = preview.dependencies.len();

    for (idx, dep) in preview.dependencies.iter().enumerate() {
        let src_path = Path::new(&dep.file_path);
        if !src_path.exists() {
            return Err(format!("文件不存在: {:?}", src_path).into());
        }

        let filename = src_path
            .file_name()
            .and_then(|f| f.to_str())
            .ok_or_else(|| format!("无效文件名: {:?}", src_path))?;

        // Broadcast progress before zipping this file
        let progress_percentage = (idx as f64 / total_files as f64) * 100.0;
        let _ = app.emit(
            "share-progress",
            ShareProgress {
                phase: "zipping".to_string(),
                current_file: filename.to_string(),
                processed_files: idx,
                total_files,
                percentage: progress_percentage,
            },
        );

        // Add file to ZIP under AddonPackages/
        let zip_entry_name = format!("AddonPackages/{}", filename);
        zip.start_file(zip_entry_name, options)?;

        let mut f = File::open(src_path)?;
        let mut buffer = [0u8; 64 * 1024]; // 64KB chunk buffer
        loop {
            let bytes_read = f.read(&mut buffer)?;
            if bytes_read == 0 {
                break;
            }
            zip.write_all(&buffer[..bytes_read])?;
        }
    }

    // Finish writing the zip
    zip.finish()?;

    // Send final progress update
    let _ = app.emit(
        "share-progress",
        ShareProgress {
            phase: "done".to_string(),
            current_file: "".to_string(),
            processed_files: total_files,
            total_files,
            percentage: 100.0,
        },
    );

    Ok(())
}

/// Export selected package and its dependencies recursively into a single ZIP archive.
/// Executes zipping in a background thread to prevent GUI blockage, emitting progress events.
#[tauri::command]
pub async fn export_share_zip(
    app: AppHandle,
    db: State<'_, Database>,
    package_id: String,
    target_zip_path: String,
    exclude_list_path: Option<String>,
) -> Result<(), String> {
    // 1. Get the list of files to package
    let preview = get_share_preview(db, package_id.clone(), exclude_list_path).await?;

    // 2. Spawn a background tokio thread to do the actual packaging
    tokio::spawn(async move {
        let result = do_export_zip(app.clone(), preview, target_zip_path);
        if let Err(e) = result {
            let _ = app.emit(
                "share-progress",
                ShareProgress {
                    phase: "error".to_string(),
                    current_file: e.to_string(),
                    processed_files: 0,
                    total_files: 0,
                    percentage: 0.0,
                },
            );
        }
    });

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dependency_selection_retains_share_metadata_and_version_rules() {
        let conn = rusqlite::Connection::open_in_memory().unwrap();
        conn.execute_batch("CREATE TABLE packages (id TEXT, creator TEXT, name TEXT, version INTEGER, size_bytes INTEGER, file_path TEXT);
            INSERT INTO packages VALUES ('Author.Sub.Package.1', 'Author', 'Sub.Package', 1, 123, 'one.var');
            INSERT INTO packages VALUES ('Author.Sub.Package.10', 'Author', 'Sub.Package', 10, 456, 'ten.var');").unwrap();
        let packages = load_installed_packages(&conn).unwrap();
        for (id, expected) in [
            ("author.sub.package.1", Some("Author.Sub.Package.1")),
            ("Author.Sub.Package.2", Some("Author.Sub.Package.10")),
            ("Author.Sub.Package.latest", Some("Author.Sub.Package.10")),
            ("Author.Sub.Package.11", None),
            ("Missing.Package.latest", None),
        ] {
            assert_eq!(
                resolve_installed_dependency_id(id, &packages).as_deref(),
                expected,
                "{id}"
            );
        }
        let serialized = serde_json::to_value(&packages[0]).unwrap();
        assert_eq!(
            serialized,
            serde_json::json!({
                "id": "Author.Sub.Package.1", "creator": "Author", "name": "Sub.Package",
                "version": 1, "size_bytes": 123, "file_path": "one.var",
            })
        );
    }
}
