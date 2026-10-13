use std::collections::{BTreeSet, HashMap, HashSet};
use std::io::Read;
use std::path::{Path, PathBuf};

use rusqlite::{Connection, Row};
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager, State};

use crate::db::Database;
use crate::models::resource::{normalize_resource_types, ResourceType};
use crate::models::var_package::VarPackageSummary;
use crate::services::install_context::resolve_install_context;

/// Dashboard statistics for the overview page
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DashboardStats {
    pub total_packages: usize,
    pub total_size_bytes: u64,
    pub scene_count: usize,
    pub appearance_count: usize,
    pub morph_count: usize,
    pub plugin_count: usize,
    pub missing_dependencies: usize,
    pub duplicate_resources: usize,
    pub orphaned_packages: usize,
    pub corrupted_packages: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CorruptedPackage {
    pub file_path: String,
    pub package_id: String,
    pub size_bytes: u64,
    pub error: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PackageImageEntry {
    pub path: String,
    pub size_bytes: u64,
}

/// One file inside a package, as recorded when the package was added to the library.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PackageContentEntry {
    pub path: String,
    pub resource_type: String,
    pub size_bytes: u64,
}

#[derive(Debug, Clone)]
struct ImageCandidate {
    path: String,
    size_bytes: u64,
    order: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PackageFolderEntry {
    pub path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ScenePreview {
    pub package_id: String,
    pub scene_files: Vec<String>,
    pub referenced_resources: Vec<String>,
    pub missing_dependencies: Vec<String>,
    pub image_count: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QuickDeleteResult {
    pub deleted_package_ids: Vec<String>,
    pub skipped_package_ids: Vec<String>,
    pub deleted_count: usize,
    pub skipped_count: usize,
    pub deleted_file_count: usize,
    pub freed_bytes: u64,
}

/// Get aggregate dashboard statistics from the database.
#[tauri::command]
pub async fn get_dashboard_stats(db: State<'_, Database>) -> Result<DashboardStats, String> {
    let analysis = crate::services::dependency_analysis::get(&db).await?;
    let db = db.inner().clone();
    tauri::async_runtime::spawn_blocking(move || db.with_conn(|conn| {
        let total_packages: usize = conn
            .query_row(
                "SELECT
                    (SELECT COUNT(*) FROM packages) +
                    (SELECT COUNT(*)
                     FROM physical_packages pp
                     LEFT JOIN packages p ON p.id = pp.package_id
                     WHERE p.id IS NULL)",
                [],
                |row| row.get(0),
            )
            .unwrap_or(0);

        let total_size_bytes: u64 = conn
            .query_row(
                "SELECT
                    (SELECT COALESCE(SUM(size_bytes), 0) FROM packages) +
                    (SELECT COALESCE(SUM(pp.size_bytes), 0)
                     FROM physical_packages pp
                     LEFT JOIN packages p ON p.id = pp.package_id
                     WHERE p.id IS NULL)",
                [],
                |row| row.get(0),
            )
            .unwrap_or(0);

        let scene_count: usize = conn
            .query_row(
                "SELECT COUNT(*) FROM contents WHERE resource_type = 'scene'",
                [],
                |row| row.get(0),
            )
            .unwrap_or(0);

        let appearance_count: usize = conn
            .query_row(
                "SELECT COUNT(*) FROM contents WHERE resource_type = 'appearance'",
                [],
                |row| row.get(0),
            )
            .unwrap_or(0);

        let morph_count: usize = conn
            .query_row(
                "SELECT COUNT(*) FROM contents WHERE resource_type = 'morph'",
                [],
                |row| row.get(0),
            )
            .unwrap_or(0);

        let plugin_count: usize = conn
            .query_row(
                "SELECT COUNT(*) FROM contents WHERE resource_type = 'plugin'",
                [],
                |row| row.get(0),
            )
            .unwrap_or(0);

        let missing_dependencies = analysis.missing.len();

        let duplicate_resources: usize = conn
            .query_row(
                "SELECT COUNT(*) FROM (
                    SELECT package_id, file_md5
                    FROM physical_packages
                    WHERE file_md5 IS NOT NULL AND file_md5 != ''
                    GROUP BY package_id, file_md5
                    HAVING COUNT(*) > 1
                 )",
                [],
                |row| row.get(0),
            )
            .unwrap_or(0);

        let orphaned_packages: usize = conn
            .query_row(
                "SELECT COUNT(*) FROM packages p
                 WHERE p.id NOT IN (SELECT depends_on_id FROM dependencies)
                 AND p.resource_types NOT LIKE '%scene%'",
                [],
                |row| row.get(0),
            )
            .unwrap_or(0);

        let corrupted_packages: usize = conn
            .query_row(
                "SELECT COUNT(*) FROM physical_packages WHERE scan_status = 'failed'",
                [],
                |row| row.get(0),
            )
            .unwrap_or(0);

        Ok(DashboardStats {
            total_packages,
            total_size_bytes,
            scene_count,
            appearance_count,
            morph_count,
            plugin_count,
            missing_dependencies,
            duplicate_resources,
            orphaned_packages,
            corrupted_packages,
        })
    })).await.map_err(|e| e.to_string())?.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn find_corrupted_packages(
    db: State<'_, Database>,
) -> Result<Vec<CorruptedPackage>, String> {
    db.with_conn(|conn| {
        let mut stmt = conn
            .prepare(
                "SELECT file_path, package_id, size_bytes, COALESCE(last_error, '')
                 FROM physical_packages
                 WHERE scan_status = 'failed'
                 ORDER BY file_path",
            )
            .map_err(|e| e.to_string())?;

        let corrupted = stmt
            .query_map([], |row| {
                Ok(CorruptedPackage {
                    file_path: row.get(0)?,
                    package_id: row.get(1)?,
                    size_bytes: row.get::<_, i64>(2)? as u64,
                    error: row.get(3)?,
                })
            })
            .map_err(|e| e.to_string())?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| e.to_string())?;

        Ok(corrupted)
    })
    .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn quick_delete_package(
    app_handle: AppHandle,
    _db: State<'_, Database>,
    package_id: String,
    include_dependencies: bool,
) -> Result<QuickDeleteResult, String> {
    crate::commands::cleanup::quick_delete(app_handle, package_id, include_dependencies).await
}

/// List all packages as summaries for the package list view.
/// 返回包含依赖数、被依赖数 and 入库时间 的摘要
#[tauri::command]
pub async fn list_packages(
    app_handle: AppHandle,
    db: State<'_, Database>,
) -> Result<Vec<VarPackageSummary>, String> {
    let result = db.with_conn(|conn| {
        let mut tags_by_package = load_all_package_tags(conn)?;
        let mut stmt = conn
            .prepare(
                "SELECT p.id, p.creator, p.name, p.version, p.file_path, p.size_bytes, p.resource_types,
                        COALESCE(p.file_created_time, p.scan_time) as created_time, p.scan_time,
                        (SELECT COUNT(*) FROM dependencies d WHERE d.package_id = p.id) as dep_count,
                        (SELECT COUNT(*) FROM dependencies d WHERE d.depends_on_id = p.id) as rev_dep_count,
                        (SELECT COUNT(*) FROM contents c WHERE c.package_id = p.id) as content_count
                 FROM packages p
                 ORDER BY p.creator, p.name, p.version",
            )
            .map_err(|e| {
                crate::errors::AppError::Database(format!("Failed to prepare query: {}", e))
            })?;

        let mut packages = stmt
            .query_map([], |row| {
                let id: String = row.get(0)?;
                map_package_summary_row(row, tags_by_package.remove(&id).unwrap_or_default())
            })
            .map_err(|e| {
                crate::errors::AppError::Database(format!("Failed to execute query: {}", e))
            })?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| {
                crate::errors::AppError::Database(format!("Failed to collect results: {}", e))
            })?;

        let mut physical_stmt = conn
            .prepare(
                "SELECT pp.file_path, pp.package_id, pp.size_bytes, pp.scan_time
                 FROM physical_packages pp
                 LEFT JOIN packages p ON p.id = pp.package_id
                 WHERE p.id IS NULL",
            )
            .map_err(|e| {
                crate::errors::AppError::Database(format!(
                    "Failed to prepare physical package query: {}",
                    e
                ))
            })?;

        let physical_packages = physical_stmt
            .query_map([], |row| {
                let file_path: String = row.get(0)?;
                let package_id: String = row.get(1)?;
                let size_bytes: i64 = row.get(2)?;
                let scan_time: String = row.get(3)?;
                Ok(physical_package_summary(
                    file_path,
                    package_id,
                    size_bytes.max(0) as u64,
                    scan_time,
                ))
            })
            .map_err(|e| {
                crate::errors::AppError::Database(format!(
                    "Failed to execute physical package query: {}",
                    e
                ))
            })?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| {
                crate::errors::AppError::Database(format!(
                    "Failed to collect physical packages: {}",
                    e
                ))
            })?;

        packages.extend(physical_packages);
        packages.sort_by(|a, b| {
            a.creator
                .to_lowercase()
                .cmp(&b.creator.to_lowercase())
                .then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase()))
                .then_with(|| a.version.cmp(&b.version))
                .then_with(|| a.file_path.to_lowercase().cmp(&b.file_path.to_lowercase()))
        });

        Ok(packages)
    })
    .map_err(|e| e.to_string())?;
    crate::services::scene_browser_index::schedule_sync(&app_handle);
    Ok(result)
}

#[tauri::command]
pub async fn get_package_summary(
    db: State<'_, Database>,
    package_id: String,
) -> Result<Option<VarPackageSummary>, String> {
    db.with_conn(|conn| {
        let mut stmt = conn
            .prepare(
                "SELECT p.id, p.creator, p.name, p.version, p.file_path, p.size_bytes, p.resource_types,
                        COALESCE(p.file_created_time, p.scan_time) as created_time, p.scan_time,
                        (SELECT COUNT(*) FROM dependencies d WHERE d.package_id = p.id) as dep_count,
                        (SELECT COUNT(*) FROM dependencies d WHERE d.depends_on_id = p.id) as rev_dep_count,
                        (SELECT COUNT(*) FROM contents c WHERE c.package_id = p.id) as content_count
                 FROM packages p
                 WHERE p.id = ?1",
            )
            .map_err(|e| {
                crate::errors::AppError::Database(format!("Failed to prepare query: {}", e))
            })?;

        let tags = load_package_tags(&package_id, conn)?;
        match stmt.query_row([package_id], |row| map_package_summary_row(row, tags)) {
            Ok(package) => Ok(Some(package)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(crate::errors::AppError::Database(format!(
                "Failed to load package summary: {}",
                e
            ))),
        }
    })
    .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn list_all_tags(db: State<'_, Database>) -> Result<Vec<String>, String> {
    db.with_conn(|conn| {
        let mut stmt = conn
            .prepare("SELECT DISTINCT tag FROM package_tags ORDER BY tag")
            .map_err(|e| crate::errors::AppError::Database(e.to_string()))?;
        let tags = stmt
            .query_map([], |row| row.get::<_, String>(0))
            .map_err(|e| crate::errors::AppError::Database(e.to_string()))?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| crate::errors::AppError::Database(e.to_string()))?;
        Ok(tags)
    })
    .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn set_package_tags(
    app_handle: AppHandle,
    db: State<'_, Database>,
    package_id: String,
    tags: Vec<String>,
) -> Result<Vec<String>, String> {
    let normalized_tags = db
        .with_conn(|conn| {
            let tx = conn
                .unchecked_transaction()
                .map_err(|e| crate::errors::AppError::Database(e.to_string()))?;
            tx.execute(
                "DELETE FROM package_tags WHERE package_id = ?1",
                [&package_id],
            )
            .map_err(|e| crate::errors::AppError::Database(e.to_string()))?;

            let mut normalized_tags = tags
                .into_iter()
                .map(|tag| tag.trim().to_string())
                .filter(|tag| !tag.is_empty())
                .collect::<Vec<_>>();
            normalized_tags.sort();
            normalized_tags.dedup();

            for tag in &normalized_tags {
                tx.execute(
                    "INSERT OR IGNORE INTO package_tags (package_id, tag) VALUES (?1, ?2)",
                    rusqlite::params![package_id, tag],
                )
                .map_err(|e| crate::errors::AppError::Database(e.to_string()))?;
            }

            tx.commit()
                .map_err(|e| crate::errors::AppError::Database(e.to_string()))?;
            Ok(normalized_tags)
        })
        .map_err(|e| e.to_string())?;

    crate::commands::library_events::emit_library_index_changed(
        &app_handle,
        "tags_changed",
        &["packages", "tags"],
        vec![package_id],
        Vec::new(),
    );

    Ok(normalized_tags)
}

#[tauri::command]
pub async fn get_scene_preview(
    db: State<'_, Database>,
    package_id: String,
) -> Result<ScenePreview, String> {
    let creator = parse_package_creator(&package_id);

    db.with_conn(|conn| {
        let mut scene_stmt = conn
            .prepare(
                "SELECT file_path FROM contents
                 WHERE package_id = ?1 AND resource_type = 'scene'
                 ORDER BY file_path",
            )
            .map_err(|e| crate::errors::AppError::Database(e.to_string()))?;
        let scene_files = scene_stmt
            .query_map([&package_id], |row| row.get::<_, String>(0))
            .map_err(|e| crate::errors::AppError::Database(e.to_string()))?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| crate::errors::AppError::Database(e.to_string()))?;

        let mut ref_stmt = conn
            .prepare(
                "SELECT file_path FROM contents
                 WHERE package_id = ?1 AND resource_type != 'scene'
                 ORDER BY resource_type, file_path
                 LIMIT 40",
            )
            .map_err(|e| crate::errors::AppError::Database(e.to_string()))?;
        let referenced_resources = ref_stmt
            .query_map([&package_id], |row| row.get::<_, String>(0))
            .map_err(|e| crate::errors::AppError::Database(e.to_string()))?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| crate::errors::AppError::Database(e.to_string()))?;

        let mut missing_stmt = conn
            .prepare(
                "SELECT d.depends_on_id
                 FROM dependencies d
                 WHERE d.package_id = ?1
                 AND NOT EXISTS (SELECT 1 FROM packages p WHERE p.id = d.depends_on_id)
                 ORDER BY d.depends_on_id",
            )
            .map_err(|e| crate::errors::AppError::Database(e.to_string()))?;
        let missing_dependencies = missing_stmt
            .query_map([&package_id], |row| row.get::<_, String>(0))
            .map_err(|e| crate::errors::AppError::Database(e.to_string()))?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| crate::errors::AppError::Database(e.to_string()))?;

        let image_count = {
            let mut image_stmt = conn
                .prepare(
                    "SELECT file_path FROM contents
                 WHERE package_id = ?1",
                )
                .map_err(|e| crate::errors::AppError::Database(e.to_string()))?;
            let file_paths = image_stmt
                .query_map([&package_id], |row| row.get::<_, String>(0))
                .map_err(|e| crate::errors::AppError::Database(e.to_string()))?
                .collect::<Result<Vec<_>, _>>()
                .map_err(|e| crate::errors::AppError::Database(e.to_string()))?;
            count_display_images_from_paths(&file_paths, creator.as_deref())
        };

        Ok(ScenePreview {
            package_id,
            scene_files,
            referenced_resources,
            missing_dependencies,
            image_count,
        })
    })
    .map_err(|e| e.to_string())
}

fn load_package_tags(
    package_id: &str,
    conn: &rusqlite::Connection,
) -> rusqlite::Result<Vec<String>> {
    let mut stmt =
        conn.prepare("SELECT tag FROM package_tags WHERE package_id = ?1 ORDER BY tag")?;
    let tags = stmt
        .query_map([package_id], |row| row.get::<_, String>(0))?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(tags)
}

fn load_all_package_tags(conn: &Connection) -> rusqlite::Result<HashMap<String, Vec<String>>> {
    let mut stmt = conn.prepare("SELECT package_id, tag FROM package_tags ORDER BY tag")?;
    let rows = stmt.query_map([], |row| {
        Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
    })?;
    let mut tags = HashMap::<String, Vec<String>>::new();
    for row in rows {
        let (id, tag) = row?;
        tags.entry(id).or_default().push(tag);
    }
    Ok(tags)
}

fn map_package_summary_row(
    row: &Row<'_>,
    tags: Vec<String>,
) -> rusqlite::Result<VarPackageSummary> {
    let package_id: String = row.get(0)?;
    let resource_types_json: String = row.get(6)?;
    let resource_types: Vec<String> =
        serde_json::from_str(&resource_types_json).unwrap_or_default();
    let resource_types = normalize_resource_types(resource_types);

    Ok(VarPackageSummary {
        id: package_id,
        creator: row.get(1)?,
        name: row.get(2)?,
        version: row.get(3)?,
        file_path: row.get(4)?,
        size_bytes: row.get(5)?,
        resource_types,
        created_time: row.get(7)?,
        scan_time: row.get(8)?,
        dependency_count: row.get(9)?,
        dependents_count: row.get(10)?,
        content_count: row.get(11)?,
        tags,
    })
}

fn physical_package_summary(
    file_path: String,
    package_id: String,
    size_bytes: u64,
    scan_time: String,
) -> VarPackageSummary {
    let (creator, name, version) = package_identity_from_id(&package_id);

    VarPackageSummary {
        id: package_id,
        creator,
        name,
        version,
        file_path,
        size_bytes,
        resource_types: vec!["other".to_string()],
        dependency_count: 0,
        dependents_count: 0,
        content_count: 0,
        created_time: scan_time.clone(),
        scan_time,
        tags: Vec::new(),
    }
}

fn package_identity_from_id(package_id: &str) -> (String, String, i32) {
    if let Some(identity) = crate::services::package_identity::from_id(package_id) {
        return (identity.creator.into(), identity.name.into(), identity.version.unwrap_or(0));
    }

    (String::new(), package_id.to_string(), 0)
}

#[tauri::command]
pub async fn list_package_folders(vam_root: String) -> Result<Vec<PackageFolderEntry>, String> {
    let root = Path::new(&vam_root);
    let source_dirs = [
        root.join("AddonPackages"),
        root.join("VAMBoxLibrary").join("AddonPackages"),
    ];
    let mut folders = BTreeSet::new();

    for source_dir in source_dirs {
        collect_package_folders(&source_dir, &source_dir, &mut folders)?;
    }

    Ok(folders
        .into_iter()
        .map(|path| PackageFolderEntry { path })
        .collect())
}

fn collect_package_folders(
    root: &Path,
    current: &Path,
    folders: &mut BTreeSet<String>,
) -> Result<(), String> {
    if !current.exists() {
        return Ok(());
    }

    let entries = std::fs::read_dir(current)
        .map_err(|e| format!("读取目录失败 {}: {}", current.display(), e))?;
    for entry in entries {
        let entry = entry.map_err(|e| format!("读取目录项失败: {}", e))?;
        let file_type = entry
            .file_type()
            .map_err(|e| format!("读取文件类型失败 {}: {}", entry.path().display(), e))?;
        if !file_type.is_dir() {
            continue;
        }

        let path = entry.path();
        if let Ok(relative) = path.strip_prefix(root) {
            let relative_path = relative
                .to_string_lossy()
                .replace('\\', "/")
                .trim_matches('/')
                .to_string();
            if !relative_path.is_empty() {
                folders.insert(relative_path);
            }
        }
        collect_package_folders(root, &path, folders)?;
    }

    Ok(())
}

fn thumbnail_directory(app: &AppHandle) -> Result<PathBuf, String> {
    Ok(app.path().app_data_dir().map_err(|e| e.to_string())?.join(crate::services::preview_cache::DIRECTORY))
}
fn package_roots(app: &AppHandle) -> Result<Vec<PathBuf>, String> {
    let context = resolve_install_context(app)?;
    Ok([context.real_addon_dir, context.managed_library_dir].into_iter()
        .filter_map(|root| std::fs::canonicalize(root).ok()).collect())
}
fn package_file_path(app: &AppHandle, db: &Database, package_id: &str) -> Result<Option<PathBuf>, String> {
    crate::services::resource_files::resolve_indexed_package_file(db, package_id, &package_roots(app)?)
}

#[tauri::command]
pub async fn get_package_thumbnail(app: AppHandle, package_id: String) -> Result<Option<String>, String> {
    let _timer = crate::services::perf::CommandTimer::new("get_package_thumbnail");
    tauri::async_runtime::spawn_blocking(move || {
        let Some(path) = package_file_path(&app, &app.state::<Database>(), &package_id)? else { return Ok(None); };
        thumbnail_for_path(&path, &package_id, &thumbnail_directory(&app)?)
    }).await.map_err(|e| e.to_string())?
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ThumbnailResult { package_id: String, path: Option<String>, cached: bool, error: Option<String> }

/// One IPC for all warm/negative hits. Cold extraction runs separately so a slow VAR
/// cannot delay cached images on the same page.
#[tauri::command]
pub async fn get_package_thumbnails(app: AppHandle, package_ids: Vec<String>, cached_only: bool) -> Result<Vec<ThumbnailResult>, String> {
    let _timer = crate::services::perf::CommandTimer::new("get_package_thumbnails");
    if package_ids.len() > 400 { return Err("一次最多读取 400 个预览".into()); }
    tauri::async_runtime::spawn_blocking(move || {
        let roots = package_roots(&app)?;
        let directory = thumbnail_directory(&app)?;
        let db = app.state::<Database>();
        let paths = crate::services::resource_files::indexed_package_paths(&db, &package_ids)?;
        let _cache_timer = crate::services::perf::CommandTimer::new("thumbnails.cache_lookup");
        package_ids.into_iter().map(|id| {
            let result = (|| -> Result<Option<Option<String>>, String> {
                let Some(path) = crate::services::resource_files::resolve_package_candidates(paths.get(&id).map(Vec::as_slice).unwrap_or(&[]), &roots) else { return Ok(Some(None)); };
                let cache = crate::services::preview_cache::PreviewCache::new(&directory, &path, &format!("cover:{id}"))?;
                if cached_only { cache.lookup() } else { thumbnail_for_path(&path, &id, &directory).map(Some) }
            })();
            Ok(match result {
                Ok(value) => ThumbnailResult { package_id: id, cached: value.is_some(), path: value.flatten(), error: None },
                Err(error) => ThumbnailResult { package_id: id, cached: false, path: None, error: Some(error) },
            })
        }).collect()
    }).await.map_err(|e| e.to_string())?
}

fn thumbnail_for_path(var_path: &Path, package_id: &str, directory: &Path) -> Result<Option<String>, String> {
    crate::services::preview_cache::PreviewCache::new(directory, var_path, &format!("cover:{package_id}"))?.image(|| {
        let mut archive = crate::services::resource_files::open_var_archive(var_path)?;
        let creator = parse_package_creator(package_id);
        let Some(name) = find_best_cover_entry_name(&mut archive, creator.as_deref()) else { return Ok(None); };
        let mut entry = archive.by_name(&name).map_err(|e| e.to_string())?;
        let mut data = Vec::new();
        entry.read_to_end(&mut data).map_err(|e| e.to_string())?;
        Ok(Some((name, data)))
    })
}

/// Files inside a package, read from the library records (no archive access).
#[tauri::command]
pub async fn list_package_contents(
    db: State<'_, Database>,
    package_id: String,
) -> Result<Vec<PackageContentEntry>, String> {
    db.with_conn(|conn| {
        let mut stmt = conn
            .prepare(
                "SELECT file_path, size_bytes FROM contents
                 WHERE package_id = ?1
                 ORDER BY file_path COLLATE NOCASE",
            )
            .map_err(|e| {
                crate::errors::AppError::Database(format!("Failed to prepare query: {}", e))
            })?;
        let rows = stmt
            .query_map([&package_id], |row| {
                let path: String = row.get(0)?;
                let size: i64 = row.get(1)?;
                Ok(PackageContentEntry {
                    resource_type: ResourceType::from_path(&path).as_str().to_string(),
                    path,
                    size_bytes: size.max(0) as u64,
                })
            })
            .map_err(|e| {
                crate::errors::AppError::Database(format!("Failed to list package contents: {}", e))
            })?;
        rows.collect::<Result<Vec<_>, _>>().map_err(|e| {
            crate::errors::AppError::Database(format!("Failed to read package contents: {}", e))
        })
    })
    .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn list_package_images(app: AppHandle, package_id: String) -> Result<Vec<PackageImageEntry>, String> {
    let _timer = crate::services::perf::CommandTimer::new("list_package_images");
    tauri::async_runtime::spawn_blocking(move || {
        let Some(path) = package_file_path(&app, &app.state::<Database>(), &package_id)? else { return Ok(Vec::new()); };
        package_images_cached(&path, &package_id, &thumbnail_directory(&app)?)
    }).await.map_err(|e| e.to_string())?
}
fn package_images_cached(path: &Path, id: &str, directory: &Path) -> Result<Vec<PackageImageEntry>, String> {
    crate::services::preview_cache::PreviewCache::new(directory, path, &format!("gallery:{id}"))?.json(|| package_images(path, id))
}

fn package_images(var_path: &Path, package_id: &str) -> Result<Vec<PackageImageEntry>, String> {
    let mut archive = crate::services::resource_files::open_var_archive(var_path)?;

    let creator = parse_package_creator(&package_id);
    let images_with_sizes = collect_display_image_candidates(&mut archive, creator.as_deref());

    let images = images_with_sizes
        .into_iter()
        .map(|candidate| PackageImageEntry {
            path: candidate.path,
            size_bytes: candidate.size_bytes,
        })
        .collect();

    Ok(images)
}

#[tauri::command]
pub async fn get_package_image(app: AppHandle, package_id: String, image_path: String) -> Result<Option<String>, String> {
    let _timer = crate::services::perf::CommandTimer::new("get_package_image");
    tauri::async_runtime::spawn_blocking(move || {
        let Some(path) = package_file_path(&app, &app.state::<Database>(), &package_id)? else { return Ok(None); };
        let directory = thumbnail_directory(&app)?;
        crate::services::preview_cache::PreviewCache::new(&directory, &path, &format!("image:{image_path}"))?.image(|| {
            let mut archive = crate::services::resource_files::open_var_archive(&path)?;
            let mut entry = match archive.by_name(&image_path) { Ok(entry) => entry, Err(_) => return Ok(None) };
            let creator = parse_package_creator(&package_id);
            if !entry.is_file() || !is_webview_image_path(entry.name()) || (!is_saves_image_path(entry.name()) && !is_custom_display_image_path(entry.name(), creator.as_deref())) { return Ok(None); }
            let mut data = Vec::new();
            entry.read_to_end(&mut data).map_err(|e| e.to_string())?;
            Ok(Some((image_path, data)))
        })
    }).await.map_err(|e| e.to_string())?
}

fn find_best_cover_entry_name(
    archive: &mut zip::ZipArchive<std::fs::File>,
    creator: Option<&str>,
) -> Option<String> {
    collect_display_image_candidates(archive, creator)
        .into_iter()
        .next()
        .map(|candidate| candidate.path)
}

fn collect_display_image_candidates(
    archive: &mut zip::ZipArchive<std::fs::File>,
    creator: Option<&str>,
) -> Vec<ImageCandidate> {
    let mut saves_images = Vec::new();
    let mut custom_images = Vec::new();
    let mut favorite_preset_keys = HashSet::new();
    let mut preset_file_keys = HashSet::new();

    for i in 0..archive.len() {
        if let Ok(entry) = archive.by_index(i) {
            if !entry.is_file() {
                continue;
            }

            let path = entry.name().to_string();
            if is_vap_favorite_path(&path) {
                favorite_preset_keys.insert(strip_known_extension_key(&path));
                continue;
            }
            if is_preset_file_path(&path) {
                preset_file_keys.insert(strip_known_extension_key(&path));
                continue;
            }

            if is_webview_image_path(&path) {
                let candidate = ImageCandidate {
                    path: path.clone(),
                    size_bytes: entry.size(),
                    order: i,
                };
                if is_saves_image_path(&path) {
                    saves_images.push(candidate);
                } else if is_custom_display_image_path(&path, creator) {
                    custom_images.push(candidate);
                }
            }
        }
    }

    choose_display_image_candidates(
        saves_images,
        custom_images,
        &favorite_preset_keys,
        &preset_file_keys,
    )
}

fn choose_display_image_candidates(
    mut saves_images: Vec<ImageCandidate>,
    mut custom_images: Vec<ImageCandidate>,
    favorite_preset_keys: &HashSet<String>,
    preset_file_keys: &HashSet<String>,
) -> Vec<ImageCandidate> {
    if !saves_images.is_empty() {
        sort_saves_cover_first(&mut saves_images);
        saves_images
    } else {
        let mut favorite_images: Vec<ImageCandidate> = custom_images
            .iter()
            .filter(|candidate| {
                favorite_preset_keys.contains(&strip_known_extension_key(&candidate.path))
            })
            .cloned()
            .collect();
        if !favorite_images.is_empty() {
            sort_last_order_first(&mut favorite_images);
            favorite_images
        } else {
            let mut preset_images: Vec<ImageCandidate> = custom_images
                .iter()
                .filter(|candidate| {
                    preset_file_keys.contains(&strip_known_extension_key(&candidate.path))
                })
                .cloned()
                .collect();
            if !preset_images.is_empty() {
                sort_last_order_first(&mut preset_images);
                preset_images
            } else {
                let mut direct_images: Vec<ImageCandidate> = custom_images
                    .iter()
                    .filter(|candidate| is_direct_custom_item_image_path(&candidate.path))
                    .cloned()
                    .collect();
                if !direct_images.is_empty() {
                    sort_last_order_first(&mut direct_images);
                    direct_images
                } else {
                    sort_last_order_first(&mut custom_images);
                    custom_images
                }
            }
        }
    }
}

fn count_display_images_from_paths(paths: &[String], creator: Option<&str>) -> usize {
    let favorite_preset_keys: HashSet<String> = paths
        .iter()
        .filter(|path| is_vap_favorite_path(path))
        .map(|path| strip_known_extension_key(path))
        .collect();
    let preset_file_keys: HashSet<String> = paths
        .iter()
        .filter(|path| is_preset_file_path(path))
        .map(|path| strip_known_extension_key(path))
        .collect();
    let saves_count = paths
        .iter()
        .filter(|path| is_webview_image_path(path.as_str()) && is_saves_image_path(path.as_str()))
        .count();
    if saves_count > 0 {
        return saves_count;
    }

    let custom_images: Vec<&String> = paths
        .iter()
        .filter(|path| {
            is_webview_image_path(path.as_str())
                && is_custom_display_image_path(path.as_str(), creator)
        })
        .collect();
    let favorite_count = custom_images
        .iter()
        .filter(|path| favorite_preset_keys.contains(&strip_known_extension_key(path.as_str())))
        .count();
    if favorite_count > 0 {
        favorite_count
    } else {
        let preset_count = custom_images
            .iter()
            .filter(|path| preset_file_keys.contains(&strip_known_extension_key(path.as_str())))
            .count();
        if preset_count > 0 {
            preset_count
        } else {
            let direct_count = custom_images
                .iter()
                .filter(|path| is_direct_custom_item_image_path(path.as_str()))
                .count();
            if direct_count > 0 {
                direct_count
            } else {
                custom_images.len()
            }
        }
    }
}

fn is_direct_custom_item_image_path(path: &str) -> bool {
    let lower = normalize_zip_path(path);
    let parts: Vec<&str> = lower.split('/').filter(|part| !part.is_empty()).collect();
    parts.len() == 6 && parts[0] == "custom" && parts[2] == "female"
}

fn is_custom_display_image_path(path: &str, creator: Option<&str>) -> bool {
    is_female_custom_creator_image_path(path, creator) || is_person_skin_image_path(path)
}

fn is_person_skin_image_path(path: &str) -> bool {
    let lower = normalize_zip_path(path);
    let parts: Vec<&str> = lower.split('/').filter(|part| !part.is_empty()).collect();
    parts.len() > 5
        && parts[0] == "custom"
        && parts[1] == "atom"
        && parts[2] == "person"
        && parts[3] == "skin"
}

fn is_preset_file_path(path: &str) -> bool {
    let lower = normalize_zip_path(path);
    lower.ends_with(".vap")
        || lower.ends_with(".vam")
        || lower.ends_with(".vaj")
        || lower.ends_with(".vab")
}

fn sort_last_order_first(images: &mut [ImageCandidate]) {
    images.sort_by(|a, b| b.order.cmp(&a.order));
}

fn sort_saves_cover_first(images: &mut [ImageCandidate]) {
    images.sort_by(|a, b| {
        saves_image_priority(&a.path)
            .cmp(&saves_image_priority(&b.path))
            .then_with(|| b.order.cmp(&a.order))
    });
}

fn saves_image_priority(path: &str) -> u8 {
    let lower = normalize_zip_path(path);
    if lower.starts_with("saves/scene/") {
        0
    } else if lower.starts_with("saves/person/") {
        1
    } else {
        2
    }
}

fn parse_package_creator(package_id: &str) -> Option<String> {
    package_id
        .split('.')
        .next()
        .filter(|creator| !creator.is_empty())
        .map(|creator| creator.to_string())
}

fn is_webview_image_path(path: &str) -> bool {
    let lower = path.to_lowercase();
    lower.ends_with(".jpg")
        || lower.ends_with(".jpeg")
        || lower.ends_with(".png")
        || lower.ends_with(".webp")
        || lower.ends_with(".bmp")
        || lower.ends_with(".gif")
}

fn is_vap_favorite_path(path: &str) -> bool {
    normalize_zip_path(path).ends_with(".vap.fav")
}

fn strip_known_extension_key(path: &str) -> String {
    let lower = normalize_zip_path(path);
    for extension in [
        ".vap.fav", ".jpeg", ".jpg", ".png", ".webp", ".bmp", ".gif", ".vap", ".vam", ".vaj",
        ".vab",
    ] {
        if let Some(stripped) = lower.strip_suffix(extension) {
            return stripped.to_string();
        }
    }
    lower
}

fn is_saves_image_path(path: &str) -> bool {
    normalize_zip_path(path).starts_with("saves/")
}

fn is_female_custom_creator_image_path(path: &str, creator: Option<&str>) -> bool {
    let Some(creator) = creator else {
        return false;
    };
    let creator_lower = creator.to_lowercase();
    if creator_lower.is_empty() {
        return false;
    }

    let lower = normalize_zip_path(path);
    let parts: Vec<&str> = lower.split('/').filter(|part| !part.is_empty()).collect();
    parts.len() > 4
        && parts[0] == "custom"
        && parts[2] == "female"
        && parts[3] == creator_lower.as_str()
}

fn normalize_zip_path(path: &str) -> String {
    path.replace('\\', "/").to_lowercase()
}

pub fn clear_thumbnail_cache_inner(app: &AppHandle) -> Result<(), String> {
    crate::services::preview_cache::clear(&app.path().app_data_dir().map_err(|e| e.to_string())?)
}
#[tauri::command]
pub async fn clear_thumbnail_cache(app: AppHandle) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || clear_thumbnail_cache_inner(&app)).await.map_err(|e| e.to_string())?
}

#[tauri::command]
pub async fn open_package_in_explorer(file_path: String) -> Result<(), String> {
    let path = std::path::Path::new(&file_path);
    if !path.is_file() {
        return Err("文件不存在".to_string());
    }
    tauri_plugin_opener::reveal_item_in_dir(path)
        .map_err(|e| format!("无法打开文件位置: {e}"))
}

#[tauri::command]
pub async fn open_path_in_explorer(path: String) -> Result<(), String> {
    let target = std::path::Path::new(&path);
    if !target.exists() {
        return Err("路径不存在".to_string());
    }

    #[cfg(target_os = "windows")]
    {
        use std::process::Command;
        let windows_path = path.replace('/', "\\");
        let result = Command::new("explorer.exe").arg(windows_path).spawn();
        match result {
            Ok(_) => Ok(()),
            Err(e) => Err(format!("无法打开文件管理器: {}", e)),
        }
    }
    #[cfg(target_os = "macos")]
    {
        use std::process::Command;
        let result = Command::new("open").arg(target).spawn();
        match result {
            Ok(_) => Ok(()),
            Err(e) => Err(format!("无法打开访达: {}", e)),
        }
    }
    #[cfg(all(not(target_os = "windows"), not(target_os = "macos")))]
    {
        Err("当前系统暂不支持直接打开该路径".to_string())
    }
}

/// 简单的 base64 编码（避免额外依赖）
pub(super) fn base64(data: &[u8]) -> std::string::String {
    const CHARS: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut result = String::with_capacity((data.len() + 2) / 3 * 4);

    for chunk in data.chunks(3) {
        let b0 = chunk[0] as u32;
        let b1 = if chunk.len() > 1 { chunk[1] as u32 } else { 0 };
        let b2 = if chunk.len() > 2 { chunk[2] as u32 } else { 0 };
        let triple = (b0 << 16) | (b1 << 8) | b2;

        result.push(CHARS[((triple >> 18) & 0x3F) as usize] as char);
        result.push(CHARS[((triple >> 12) & 0x3F) as usize] as char);
        if chunk.len() > 1 {
            result.push(CHARS[((triple >> 6) & 0x3F) as usize] as char);
        } else {
            result.push('=');
        }
        if chunk.len() > 2 {
            result.push(CHARS[(triple & 0x3F) as usize] as char);
        } else {
            result.push('=');
        }
    }

    result
}

#[cfg(test)]
mod summary_tests {
    use super::*;

    #[test]
    fn bulk_tags_match_single_package_loading_and_keep_sort_order() {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(
            "CREATE TABLE package_tags (package_id TEXT, tag TEXT);
            INSERT INTO package_tags VALUES ('A.P.1','z'),('B.P.1','other'),('A.P.1','a');",
        )
        .unwrap();
        let tags = load_all_package_tags(&conn).unwrap();
        assert_eq!(tags["A.P.1"], vec!["a", "z"]);
        assert_eq!(tags["A.P.1"], load_package_tags("A.P.1", &conn).unwrap());
        assert_eq!(tags["B.P.1"], vec!["other"]);
        assert!(!tags.contains_key("missing"));
        conn.execute("DROP TABLE package_tags", []).unwrap();
        assert!(load_all_package_tags(&conn).is_err());
    }
}

#[cfg(test)]
mod thumbnail_tests {
    use super::*;
    use std::collections::HashSet;

    #[test]
    fn seven_zip_files_renamed_as_var_report_format_instead_of_a_missing_path() {
        use crate::services::resource_files::{open_var_archive, tests::TestDir};
        let dir = TestDir::new();
        let file = dir.write("JYY happy cafe.var", &[0x37, 0x7a, 0xbc, 0xaf, 0x27, 0x1c]);
        assert!(open_var_archive(&file).err().unwrap().contains("实际是 7z"));
        assert!(package_images(&file, "JYY happy cafe").err().unwrap().contains("实际是 7z"));
    }

    #[test]
    fn web_or_xml_downloads_renamed_as_var_report_non_archive_content() {
        use crate::services::resource_files::{open_var_archive, tests::TestDir};
        let dir = TestDir::new();
        for (name, content) in [
            ("Web.1.var", "<!DOCTYPE html><html>Download failed</html>"),
            ("Xml.1.var", "<?xml version=\"1.0\"?><Error/>"),
        ] {
            let file = dir.write(name, content.as_bytes());
            assert!(open_var_archive(&file).err().unwrap().contains("不是 VAR 压缩包"));
        }
    }

    #[test]
    #[ignore = "reads a selected real 7z file without modifying it; set VAM_PACKAGE_TEST_FILE"]
    fn real_seven_zip_package_reports_format_read_only() {
        use crate::services::resource_files::open_var_archive;
        let file = PathBuf::from(std::env::var("VAM_PACKAGE_TEST_FILE").unwrap());
        let mut signature = [0; 6];
        std::fs::File::open(&file).unwrap().read_exact(&mut signature).unwrap();
        assert_eq!(signature, [0x37, 0x7a, 0xbc, 0xaf, 0x27, 0x1c]);
        let error = open_var_archive(&file).err().unwrap();
        assert!(error.contains("实际是 7z"));
        eprintln!("Read-only format check: {error}");
    }

    #[test]
    fn physical_packages_with_spaces_and_unicode_read_images_without_metadata() {
        use crate::services::resource_files::{resolve_indexed_package_file, tests::TestDir};
        use std::io::Write;
        let dir = TestDir::new();
        let file = dir.0.join("作者 happy cafe, preview.var");
        let image_path = "Saves/scene/Cafe.jpg";
        let mut zip = zip::ZipWriter::new(std::fs::File::create(&file).unwrap());
        zip.start_file(image_path, zip::write::SimpleFileOptions::default()).unwrap();
        zip.write_all(b"preview bytes").unwrap();
        zip.finish().unwrap();
        let db = Database::new(&dir.0.join("test.db")).unwrap();
        let id = "作者 happy cafe, preview";
        db.with_conn(|conn| {
            conn.execute("INSERT INTO physical_packages(file_path,package_id,scan_time,scan_status) VALUES (?1,?2,'now','failed')", rusqlite::params![file.to_string_lossy(), id])?;
            Ok(())
        }).unwrap();
        let roots = vec![std::fs::canonicalize(&dir.0).unwrap()];
        let resolved = resolve_indexed_package_file(&db, id, &roots).unwrap().unwrap();
        let images = package_images(&resolved, id).unwrap();
        assert_eq!(images.len(), 1);
        assert_eq!(images[0].path, image_path);
        let mut archive = zip::ZipArchive::new(std::fs::File::open(&resolved).unwrap()).unwrap();
        assert_eq!(find_best_cover_entry_name(&mut archive, parse_package_creator(id).as_deref()).as_deref(), Some(image_path));
        let outside = TestDir::new();
        assert!(resolve_indexed_package_file(&db, id, &[std::fs::canonicalize(&outside.0).unwrap()]).unwrap().is_none());
        std::fs::remove_file(&file).unwrap();
        assert!(resolve_indexed_package_file(&db, id, &roots).unwrap().is_none());
    }

    fn candidate(path: &str, order: usize) -> ImageCandidate {
        ImageCandidate {
            path: path.to_string(),
            size_bytes: 1,
            order,
        }
    }

    #[test]
    fn saves_images_win_over_custom_images() {
        let images = choose_display_image_candidates(
            vec![
                candidate("Saves/scene/a.jpg", 1),
                candidate("Saves/scene/z.jpg", 2),
            ],
            vec![candidate(
                "Custom/Clothing/Female/phantasydope/item/z.jpg",
                3,
            )],
            &HashSet::new(),
            &HashSet::new(),
        );

        assert_eq!(images[0].path, "Saves/scene/z.jpg");
        assert_eq!(images.len(), 2);
    }

    #[test]
    fn scene_saves_images_win_over_texture_mix_images() {
        let images = choose_display_image_candidates(
            vec![
                candidate("Saves/Textures-Mix/styleuneed101200044.jpg", 8),
                candidate(
                    "Saves/scene/CG-STUDIO/Touchy-Booty Shake Dance Vol.5/Touchy-Booty Shake Dance Vol.5 - CAMRIDE.jpg",
                    3,
                ),
            ],
            Vec::new(),
            &HashSet::new(),
            &HashSet::new(),
        );

        assert_eq!(
            images[0].path,
            "Saves/scene/CG-STUDIO/Touchy-Booty Shake Dance Vol.5/Touchy-Booty Shake Dance Vol.5 - CAMRIDE.jpg"
        );
        assert_eq!(images.len(), 2);
    }

    #[test]
    fn custom_images_are_used_when_saves_is_empty() {
        let images = choose_display_image_candidates(
            Vec::new(),
            vec![
                candidate("Custom/Hair/Female/Theuf/Juliahairstyleredhead/a.jpg", 1),
                candidate("Custom/Hair/Female/Theuf/Juliahairstyleredhead/z.jpg", 2),
            ],
            &HashSet::new(),
            &HashSet::new(),
        );

        assert_eq!(
            images[0].path,
            "Custom/Hair/Female/Theuf/Juliahairstyleredhead/z.jpg"
        );
        assert_eq!(images.len(), 2);
    }

    #[test]
    fn favorite_preset_images_win_over_other_custom_images() {
        let mut favorite_keys = HashSet::new();
        favorite_keys.insert(strip_known_extension_key(
            "Custom/Clothing/Female/Skynet/Toenails HD shorter/Toenails HD shorter_7.Natural.vap.fav",
        ));
        let images = choose_display_image_candidates(
            Vec::new(),
            vec![
                candidate("Custom/Clothing/Female/Skynet/Toenails HD shorter/universe.jpg", 87),
                candidate(
                    "Custom/Clothing/Female/Skynet/Toenails HD shorter/Toenails HD shorter_7.Natural.jpg",
                    83,
                ),
            ],
            &favorite_keys,
            &HashSet::new(),
        );

        assert_eq!(
            images[0].path,
            "Custom/Clothing/Female/Skynet/Toenails HD shorter/Toenails HD shorter_7.Natural.jpg"
        );
        assert_eq!(images.len(), 1);
    }

    #[test]
    fn display_image_count_uses_favorite_preset_images() {
        let paths = vec![
            "Custom/Clothing/Female/Skynet/Toenails HD shorter/universe.jpg".to_string(),
            "Custom/Clothing/Female/Skynet/Toenails HD shorter/Toenails HD shorter_7.Natural.jpg"
                .to_string(),
            "Custom/Clothing/Female/Skynet/Toenails HD shorter/Toenails HD shorter_7.Natural.vap.fav"
                .to_string(),
        ];

        assert_eq!(count_display_images_from_paths(&paths, Some("Skynet")), 1);
    }

    #[test]
    fn preset_screenshot_images_win_over_texture_images() {
        let mut preset_keys = HashSet::new();
        preset_keys.insert(strip_known_extension_key(
            "Custom/Clothing/Female/VL_13/Top_SK/Top_SK.vam",
        ));
        preset_keys.insert(strip_known_extension_key(
            "Custom/Clothing/Female/VL_13/Top_SK/Top_SK_open.vam",
        ));
        let images = choose_display_image_candidates(
            Vec::new(),
            vec![
                candidate("Custom/Clothing/Female/VL_13/Top_SK/Top_SK.jpg", 1),
                candidate("Custom/Clothing/Female/VL_13/Top_SK/Top_SK_open.jpg", 5),
                candidate(
                    "Custom/Clothing/Female/VL_13/Top_SK/texture/Top_SK_spc.jpg",
                    13,
                ),
            ],
            &HashSet::new(),
            &preset_keys,
        );

        assert_eq!(
            images[0].path,
            "Custom/Clothing/Female/VL_13/Top_SK/Top_SK_open.jpg"
        );
        assert_eq!(images.len(), 2);
    }

    #[test]
    fn display_image_count_uses_preset_screenshot_images() {
        let paths = vec![
            "Custom/Clothing/Female/VL_13/Top_SK/Top_SK.jpg".to_string(),
            "Custom/Clothing/Female/VL_13/Top_SK/Top_SK.vam".to_string(),
            "Custom/Clothing/Female/VL_13/Top_SK/Top_SK_open.jpg".to_string(),
            "Custom/Clothing/Female/VL_13/Top_SK/Top_SK_open.vam".to_string(),
            "Custom/Clothing/Female/VL_13/Top_SK/texture/Top_SK_spc.jpg".to_string(),
        ];

        assert_eq!(count_display_images_from_paths(&paths, Some("VL_13")), 2);
    }

    #[test]
    fn person_skin_preset_images_win_over_texture_folder_images() {
        let mut preset_keys = HashSet::new();
        preset_keys.insert(strip_known_extension_key(
            "Custom/Atom/Person/Skin/Tan Lines/Victoria 6/Preset_String Bikini 6%.vap",
        ));
        let images = choose_display_image_candidates(
            Vec::new(),
            vec![
                candidate(
                    "Custom/Atom/Person/Skin/Tan Lines/Victoria 6/Preset_String Bikini 6%.jpg",
                    431,
                ),
                candidate(
                    "Custom/Atom/Person/Textures/Tan Lines (decals)/Victoria 6/6%/torsoDecal.png",
                    640,
                ),
            ],
            &HashSet::new(),
            &preset_keys,
        );

        assert_eq!(
            images[0].path,
            "Custom/Atom/Person/Skin/Tan Lines/Victoria 6/Preset_String Bikini 6%.jpg"
        );
        assert_eq!(images.len(), 1);
    }

    #[test]
    fn female_custom_author_filter_only_accepts_requested_shape() {
        assert!(is_female_custom_creator_image_path(
            "Custom/Clothing/Female/phantasydope/bla_psd_pamnt1/bla_psd_pamnt1.jpg",
            Some("phantasydope")
        ));
        assert!(!is_female_custom_creator_image_path(
            "Custom/Clothing/Male/phantasydope/bla_psd_pamnt1/bla_psd_pamnt1.jpg",
            Some("phantasydope")
        ));
        assert!(!is_female_custom_creator_image_path(
            "Custom/Hair/Female/Other/Juliahairstyleredhead/Juliahairstyleredhead.jpg",
            Some("Theuf")
        ));
        assert!(!is_female_custom_creator_image_path(
            "Saves/scene/Theuf/preview.jpg",
            Some("Theuf")
        ));
        assert!(!is_female_custom_creator_image_path(
            "Custom/Assets/Theuf/Prop/preview.png",
            Some("Theuf")
        ));
    }

    #[test]
    fn custom_display_filter_accepts_skin_but_not_textures() {
        assert!(is_custom_display_image_path(
            "Custom/Atom/Person/Skin/Tan Lines/Preset_String Bikini 6%.jpg",
            Some("DJ")
        ));
        assert!(!is_custom_display_image_path(
            "Custom/Atom/Person/Textures/Tan Lines (decals)/Victoria 6/6%/torsoDecal.png",
            Some("DJ")
        ));
    }

    #[test]
    fn saves_filter_accepts_saves_folder_only() {
        assert!(is_saves_image_path("Saves/scene/z.jpg"));
        assert!(is_saves_image_path("saves/person/preview.png"));
        assert!(!is_saves_image_path("Custom/Saves/preview.jpg"));
        assert!(!is_saves_image_path("meta.jpg"));
    }
}

#[cfg(test)]
mod performance_measurement {
    use super::*;
    #[test]
    #[ignore = "read-only local performance sample; requires VAM_BENCH_DB and VAM_BENCH_CACHE"]
    fn measure_thumbnail_cache() {
        let conn = Connection::open_with_flags(std::env::var("VAM_BENCH_DB").unwrap(), rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY).unwrap();
        let cache = PathBuf::from(std::env::var("VAM_BENCH_CACHE").unwrap());
        let mut stmt = conn.prepare("SELECT id,file_path FROM packages ORDER BY id").unwrap();
        let rows: Vec<(String,String)> = stmt.query_map([], |r| Ok((r.get(0)?, r.get(1)?))).unwrap().map(Result::unwrap).collect();
        let samples: Vec<_> = rows.into_iter().filter(|(id,path)| cache.join(format!("{id}.jpg")).is_file() && Path::new(path).is_file()).take(100).collect();
        let cache = PathBuf::from(std::env::var("VAM_BENCH_OUTPUT").unwrap());
        assert!(!samples.is_empty());
        let samples: Vec<_> = samples.iter().cycle().take(100).collect();
        for _ in 0..3 {
            let t = std::time::Instant::now();
            let mut bytes = 0;
            for (id, path) in &samples {
                bytes += thumbnail_for_path(Path::new(path), id, &cache).unwrap().unwrap().len();
            }
            eprintln!("MEASURE thumbnails100={:.2}ms ipc_bytes={bytes}", t.elapsed().as_secs_f64()*1000.0);
        }
        let path: String = conn.query_row("SELECT file_path FROM packages WHERE id='Solerrain.Caroline.1'", [], |r| r.get(0)).unwrap();
        for _ in 0..3 {
            let t = std::time::Instant::now();
            let images = package_images_cached(Path::new(&path), "Solerrain.Caroline.1", &cache).unwrap();
            eprintln!("MEASURE image_list={:.2}ms count={}", t.elapsed().as_secs_f64()*1000.0, images.len());
        }
    }
}
