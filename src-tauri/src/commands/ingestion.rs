use super::migration::{localized_type_folder, target_root, MigrationMode};
use crate::db::Database;
use crate::errors::AppError;
use crate::models::resource::primary_resource_type;
use crate::services::{
    install_context::resolve_install_context, resource_files as files, var_parser,
};
use files::DiskFile;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use tauri::{Emitter, Manager};

#[derive(Clone, Deserialize)]
pub struct IngestionConfig {
    source_dir: String,
    mode: MigrationMode,
    custom_folder: String,
    locale: Option<String>,
}

#[derive(Clone, Serialize)]
pub struct IngestionGroup {
    package_id: String,
    source: String,
    destination: String,
    incoming: bool,
    retired: Vec<DiskFile>,
    #[serde(skip)]
    winner: DiskFile,
}

#[derive(Clone, Serialize)]
pub struct IngestionPreview {
    plan_id: String,
    source_dir: String,
    target_dir: String,
    total_files: usize,
    move_count: usize,
    retire_count: usize,
    skipped: usize,
    warnings: Vec<String>,
    groups: Vec<IngestionGroup>,
}

struct IngestionPlan {
    preview: IngestionPreview,
    source_files: Vec<DiskFile>,
    target_files: Vec<DiskFile>,
    excluded: HashSet<String>,
    trash: PathBuf,
}

#[derive(Default)]
pub struct IngestionState(Mutex<Option<IngestionPlan>>);

#[derive(Default, Clone, Serialize)]
pub struct IngestionResult {
    plan_id: String,
    completed: usize,
    moved: usize,
    retired: usize,
    failed: usize,
    total: usize,
    errors: Vec<String>,
}

fn inventory(
    root: &Path,
    excluded: &HashSet<String>,
) -> Result<(Vec<DiskFile>, Vec<String>), String> {
    if !root.exists() {
        return Ok((vec![], vec![]));
    }
    let (mut entries, warnings) = files::inventory(root, excluded)?;
    entries.retain(|f| {
        Path::new(&f.path)
            .extension()
            .is_some_and(|e| e.eq_ignore_ascii_case("var"))
    });
    Ok((entries, warnings))
}

fn custom_folder(value: &str) -> Result<PathBuf, String> {
    // Accept nested relative folders, but never roots, traversal, internal folders or ADS.
    let value = value.trim().replace('\\', "/");
    if value.is_empty()
        || value.split('/').any(|s| {
            s.is_empty()
                || s == "."
                || s == ".."
                || s.ends_with(['.', ' '])
                || s.chars().any(|c| c.is_control() || "<>:\"|?*".contains(c))
                || matches!(
                    s.to_lowercase().as_str(),
                    ".trash" | ".downloads" | ".vamboxlibrary-downloads"
                )
                || matches!(
                    s.split('.')
                        .next()
                        .unwrap_or_default()
                        .to_uppercase()
                        .as_str(),
                    "CON"
                        | "PRN"
                        | "AUX"
                        | "NUL"
                        | "COM1"
                        | "COM2"
                        | "COM3"
                        | "COM4"
                        | "COM5"
                        | "COM6"
                        | "COM7"
                        | "COM8"
                        | "COM9"
                        | "LPT1"
                        | "LPT2"
                        | "LPT3"
                        | "LPT4"
                        | "LPT5"
                        | "LPT6"
                        | "LPT7"
                        | "LPT8"
                        | "LPT9"
                )
        })
    {
        return Err("自定义规则必须是有效的相对子目录，例如 收藏/场景".into());
    }
    Ok(PathBuf::from(value))
}

fn build_plan(
    config: &IngestionConfig,
    target: &Path,
    trash: PathBuf,
    excluded: HashSet<String>,
) -> Result<IngestionPlan, String> {
    let source = files::canonical_dir(Path::new(&config.source_dir))?;
    let target = target_root(target)?;
    let source_key = files::path_key(&source.to_string_lossy());
    let target_key = files::path_key(&target.to_string_lossy());
    if source_key == target_key
        || source_key.starts_with(&(target_key.clone() + "/"))
        || target_key.starts_with(&(source_key + "/"))
    {
        return Err("来源与游戏资源目录不能相同或互相包含".into());
    }
    let trash = target_root(&trash)?;
    let trash_key = files::path_key(&trash.to_string_lossy());
    let source_key = files::path_key(&source.to_string_lossy());
    if source_key == trash_key
        || source_key.starts_with(&(trash_key.clone() + "/"))
        || trash_key.starts_with(&(source_key + "/"))
    {
        return Err("来源不能是回收站，也不能包含回收站；请使用回收站恢复功能".into());
    }
    let custom = if config.mode == MigrationMode::Custom {
        custom_folder(&config.custom_folder)?
    } else {
        PathBuf::new()
    };
    let (source_files, mut warnings) = inventory(&source, &excluded)?;
    let (target_files, target_warnings) = inventory(&target, &excluded)?;
    warnings.extend(target_warnings);
    let mut families: BTreeMap<String, Vec<(DiskFile, bool, u64)>> = BTreeMap::new();
    let mut skipped = 0;
    for file in &source_files {
        if let Some(name) = files::package_name(Path::new(&file.path)) {
            families
                .entry(name.family.to_lowercase())
                .or_default()
                .push((file.clone(), true, name.version));
        } else {
            skipped += 1;
            warnings.push(format!("无法识别资源包名称，保留原文件: {}", file.path));
        }
    }
    for file in &target_files {
        if let Some(name) = files::package_name(Path::new(&file.path)) {
            if let Some(group) = families.get_mut(&name.family.to_lowercase()) {
                group.push((file.clone(), false, name.version));
            }
        }
    }
    // A managed hard link can look like an ordinary file. Do not supersede its manifest.
    let blocked: HashSet<_> = excluded
        .iter()
        .filter_map(|p| files::package_name(Path::new(p)).map(|n| n.family.to_lowercase()))
        .collect();
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(|e| e.to_string())?
        .as_nanos();
    let mut groups = vec![];
    let mut destinations = HashSet::new();
    for (family, mut candidates) in families {
        candidates.sort_by(|a, b| {
            b.2.cmp(&a.2)
                .then_with(|| {
                    now.abs_diff(a.0.modified.parse::<u128>().unwrap_or(0))
                        .cmp(&now.abs_diff(b.0.modified.parse::<u128>().unwrap_or(0)))
                })
                .then_with(|| a.1.cmp(&b.1)) // Equal timestamps: keep the installed copy.
                .then_with(|| a.0.path.cmp(&b.0.path))
        });
        let (winner, incoming, _) = &candidates[0];
        let pkg = var_parser::parse_var_file(Path::new(&winner.path));
        if blocked.contains(&family) || pkg.is_err() {
            skipped += candidates.iter().filter(|c| c.1).count();
            warnings.push(format!(
                "保留整个资源包组 {}: {}",
                family,
                if blocked.contains(&family) {
                    "存在按需启动托管映射，请先在资源迁移中还原".to_string()
                } else {
                    pkg.err().unwrap().to_string()
                }
            ));
            continue;
        }
        let pkg = pkg.unwrap();
        let folder = match config.mode {
            MigrationMode::Flatten => PathBuf::new(),
            MigrationMode::ByCreator => custom_folder(&pkg.creator)?,
            MigrationMode::ByType => PathBuf::from(localized_type_folder(
                &primary_resource_type(pkg.resource_types.clone()),
                config.locale.as_deref(),
            )),
            MigrationMode::ByScene => {
                if pkg.resource_types.iter().any(|t| t == "scene") {
                    custom_folder(&pkg.creator)?
                } else {
                    PathBuf::from("dependencies")
                }
            }
            MigrationMode::Custom => custom.clone(),
        };
        let destination = if *incoming {
            target
                .join(folder)
                .join(Path::new(&winner.path).file_name().ok_or("无效文件名")?)
        } else {
            PathBuf::from(&winner.path)
        };
        files::ensure_no_links(&destination)?;
        let key = files::path_key(&destination.to_string_lossy());
        if !destinations.insert(key.clone())
            || (destination.exists()
                && !candidates.iter().any(|c| files::path_key(&c.0.path) == key))
        {
            return Err(format!("目标被其他文件占用: {}", destination.display()));
        }
        groups.push(IngestionGroup {
            package_id: pkg.id,
            source: winner.path.clone(),
            destination: destination.to_string_lossy().into_owned(),
            incoming: *incoming,
            winner: winner.clone(),
            retired: candidates.iter().skip(1).map(|c| c.0.clone()).collect(),
        });
    }
    let preview = IngestionPreview {
        plan_id: format!(
            "ing_{}",
            chrono::Utc::now().timestamp_nanos_opt().unwrap_or_default()
        ),
        source_dir: source.to_string_lossy().into_owned(),
        target_dir: target.to_string_lossy().into_owned(),
        total_files: source_files.len(),
        move_count: groups.iter().filter(|g| g.incoming).count(),
        retire_count: groups.iter().map(|g| g.retired.len()).sum(),
        skipped,
        warnings,
        groups,
    };
    Ok(IngestionPlan {
        preview,
        source_files,
        target_files,
        excluded,
        trash,
    })
}

fn validate_plan(plan: &IngestionPlan) -> Result<(), String> {
    files::canonical_dir(Path::new(&plan.preview.source_dir))?;
    target_root(Path::new(&plan.preview.target_dir))?;
    let (source, _) = inventory(Path::new(&plan.preview.source_dir), &plan.excluded)?;
    let (target, _) = inventory(Path::new(&plan.preview.target_dir), &plan.excluded)?;
    if source != plan.source_files || target != plan.target_files {
        return Err("来源或游戏资源目录已变化，请重新预览".into());
    }
    files::ensure_no_links(&plan.trash)?;
    for group in &plan.preview.groups {
        files::ensure_no_links(Path::new(&group.destination))?;
        if group.incoming
            && Path::new(&group.destination).exists()
            && !group
                .retired
                .iter()
                .any(|f| files::path_key(&f.path) == files::path_key(&group.destination))
        {
            return Err(format!("目标已被占用，请重新预览: {}", group.destination));
        }
    }
    Ok(())
}

fn execute_group(
    db: &Database,
    plan: &IngestionPlan,
    group: &IngestionGroup,
    index: usize,
) -> Result<(), String> {
    files::verify(&group.winner)?;
    for file in &group.retired {
        files::verify(file)?;
    }
    // Parse before moving anything so a corrupt winner cannot replace working resources.
    let mut pkg =
        var_parser::parse_var_file(Path::new(&group.source)).map_err(|e| e.to_string())?;
    pkg.file_path = group.destination.clone();
    let mut moved: Vec<(PathBuf, PathBuf)> = vec![];
    let result = db.with_conn(|conn| {
        let tx = conn.unchecked_transaction()?;
        for (n, file) in group.retired.iter().enumerate() {
            let source = Path::new(&file.path);
            let trash = plan.trash.join(format!("{}_{}_{}_{}", plan.preview.plan_id, index, n, source.file_name().unwrap().to_string_lossy()));
            let id = files::package_name(source).ok_or_else(|| AppError::Io("无效资源包".into()))?.id;
            tx.execute("INSERT INTO cleanup_trash (package_id, original_path, trash_path, size_bytes) VALUES (?1,?2,?3,?4)", rusqlite::params![id,file.path,trash.to_string_lossy(),file.size])?;
            files::verify(file).map_err(AppError::Io)?;
            files::transfer(source, &trash, "move").map_err(AppError::Io)?;
            moved.push((source.to_path_buf(), trash));
            tx.execute("DELETE FROM physical_packages WHERE lower(replace(file_path, '\\', '/'))=?1", [files::path_key(&file.path)])?;
            // Preserve tags and metadata for a same-ID replacement until its upsert below.
            tx.execute("DELETE FROM packages WHERE lower(replace(file_path, '\\', '/'))=?1 AND id<>?2", rusqlite::params![files::path_key(&file.path),pkg.id])?;
        }
        if group.incoming {
            files::verify(&group.winner).map_err(AppError::Io)?;
            files::transfer(Path::new(&group.source), Path::new(&group.destination), "move").map_err(AppError::Io)?;
            moved.push((PathBuf::from(&group.source), PathBuf::from(&group.destination)));
            files::update_index_path(&tx, &group.source, &group.destination)?;
        }
        files::index_parsed_package(&tx, &pkg)?;
        tx.commit()?;
        Ok(())
    });
    if let Err(error) = result {
        let mut errors = vec![error.to_string()];
        for (source, destination) in moved.iter().rev() {
            if let Err(e) = files::transfer(destination, source, "move") {
                errors.push(format!(
                    "恢复失败，文件保留在 {}: {}",
                    destination.display(),
                    e
                ));
            }
        }
        return Err(errors.join("\n"));
    }
    Ok(())
}

#[tauri::command]
pub async fn preview_ingestion(
    app_handle: tauri::AppHandle,
    config: IngestionConfig,
) -> Result<IngestionPreview, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let _guard = files::maintenance_lock()?;
        let context = resolve_install_context(&app_handle)?;
        let plan = build_plan(
            &config,
            Path::new(&context.real_addon_dir),
            Path::new(&context.vam_root).join("VAMBoxLibrary/.trash"),
            super::deduplication::managed_link_paths(&app_handle),
        )?;
        let preview = plan.preview.clone();
        *app_handle
            .state::<IngestionState>()
            .0
            .lock()
            .map_err(|e| e.to_string())? = Some(plan);
        Ok(preview)
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
pub async fn execute_ingestion(
    app_handle: tauri::AppHandle,
    plan_id: String,
) -> Result<IngestionResult, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let _guard = files::maintenance_lock()?;
        let plan = {
            let state = app_handle.state::<IngestionState>();
            let mut guard = state.0.lock().map_err(|e| e.to_string())?;
            if guard.as_ref().map(|p| &p.preview.plan_id) != Some(&plan_id) {
                return Err("入库预览已失效，请重新预览".into());
            }
            guard.take().unwrap()
        };
        let context = resolve_install_context(&app_handle)?;
        if files::path_key(&target_root(Path::new(&context.real_addon_dir))?.to_string_lossy())
            != files::path_key(&plan.preview.target_dir)
            || super::deduplication::managed_link_paths(&app_handle) != plan.excluded
        {
            return Err("游戏目录或托管映射已改变，请重新预览".into());
        }
        validate_plan(&plan)?;
        let mut result = IngestionResult {
            plan_id,
            total: plan.preview.groups.len(),
            ..Default::default()
        };
        for (index, group) in plan.preview.groups.iter().enumerate() {
            match execute_group(&app_handle.state::<Database>(), &plan, group, index) {
                Ok(()) => {
                    result.completed += 1;
                    result.moved += usize::from(group.incoming);
                    result.retired += group.retired.len();
                }
                Err(e) => {
                    result.failed += 1;
                    result.errors.push(format!("{}: {}", group.package_id, e));
                }
            }
            let _ = app_handle.emit("ingestion-progress", &result);
            // Stop on failure: a partial result must be previewed again before retrying.
            if result.failed > 0 {
                break;
            }
        }
        super::library_events::emit_library_index_changed(
            &app_handle,
            "ingestion_changed",
            &[
                "packages",
                "dashboard",
                "dependencies",
                "statistics",
                "folders",
                "dedupe",
            ],
            vec![],
            plan.preview
                .groups
                .iter()
                .map(|g| g.destination.clone())
                .collect(),
        );
        Ok(result)
    })
    .await
    .map_err(|e| e.to_string())?
}

#[cfg(test)]
mod tests {
    use super::*;
    use files::tests::TestDir;
    use std::fs;
    use std::io::Write;

    fn var(dir: &TestDir, name: &str, seconds: u64, content: &str) -> PathBuf {
        let path = dir.0.join(name);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        let mut zip = zip::ZipWriter::new(fs::File::create(&path).unwrap());
        zip.start_file("meta.json", zip::write::SimpleFileOptions::default())
            .unwrap();
        zip.write_all(br#"{"dependencies":{"D.Dependency.latest":{}}}"#)
            .unwrap();
        zip.start_file(content, zip::write::SimpleFileOptions::default())
            .unwrap();
        zip.write_all(b"{}").unwrap();
        zip.finish()
            .unwrap()
            .set_times(
                fs::FileTimes::new()
                    .set_modified(std::time::UNIX_EPOCH + std::time::Duration::from_secs(seconds)),
            )
            .unwrap();
        path
    }

    fn config(source: &TestDir) -> IngestionConfig {
        IngestionConfig {
            source_dir: source.0.to_string_lossy().into_owned(),
            mode: MigrationMode::Flatten,
            custom_folder: "Favorites/Scenes".into(),
            locale: Some("zh-CN".into()),
        }
    }

    fn plan(source: &TestDir, game: &TestDir) -> IngestionPlan {
        build_plan(
            &config(source),
            &game.0.join("AddonPackages"),
            game.0.join("VAMBoxLibrary/.trash"),
            HashSet::new(),
        )
        .unwrap()
    }

    #[test]
    fn recursively_imports_highest_numeric_version_and_recycles_duplicates() {
        let source = TestDir::new();
        let game = TestDir::new();
        let data = TestDir::new();
        var(&source, "a/A.Asset.2.var", 300, "Custom/a.txt");
        let winner = var(&source, "b/A.Asset.10.var", 100, "Custom/b.txt");
        let bytes = fs::read(&winner).unwrap();
        var(
            &game,
            "AddonPackages/nested/A.Asset.1.var",
            400,
            "Custom/c.txt",
        );
        let unrelated = var(
            &game,
            "AddonPackages/B.Unrelated.1.var",
            100,
            "Custom/d.txt",
        );
        let ordinary = source.write("a/readme.txt", b"leave me");
        let p = plan(&source, &game);
        assert_eq!(p.preview.total_files, 2);
        assert_eq!(p.preview.move_count, 1);
        assert_eq!(p.preview.retire_count, 2);
        assert_eq!(p.preview.groups[0].package_id, "A.Asset.10");
        validate_plan(&p).unwrap();
        let db = Database::new(&data.0.join("test.db")).unwrap();
        execute_group(&db, &p, &p.preview.groups[0], 0).unwrap();
        assert!(!winner.exists());
        assert!(ordinary.exists() && unrelated.exists());
        assert_eq!(
            fs::read(game.0.join("AddonPackages/A.Asset.10.var")).unwrap(),
            bytes
        );
        db.with_conn(|conn| {
            let count: i64 =
                conn.query_row("SELECT count(*) FROM cleanup_trash", [], |r| r.get(0))?;
            assert_eq!(count, 2);
            let path: String = conn.query_row(
                "SELECT file_path FROM packages WHERE id='A.Asset.10'",
                [],
                |r| r.get(0),
            )?;
            assert_eq!(
                Path::new(&path),
                game.0.join("AddonPackages/A.Asset.10.var")
            );
            let deps: i64 = conn.query_row(
                "SELECT count(*) FROM dependencies WHERE package_id='A.Asset.10'",
                [],
                |r| r.get(0),
            )?;
            assert_eq!(deps, 1);
            let mut stmt = conn.prepare("SELECT trash_path FROM cleanup_trash")?;
            for path in stmt.query_map([], |r| r.get::<_, String>(0))? {
                assert!(Path::new(&path?).is_file());
            }
            Ok(())
        })
        .unwrap();
    }

    #[test]
    fn equal_version_replaces_older_installed_copy_and_preserves_tags() {
        let source = TestDir::new();
        let game = TestDir::new();
        let data = TestDir::new();
        let incoming = var(&source, "nested/A.Asset.1.var", 200, "Custom/new.txt");
        let installed = var(&game, "AddonPackages/A.Asset.1.var", 100, "Custom/old.txt");
        let bytes = fs::read(&incoming).unwrap();
        let db = Database::new(&data.0.join("test.db")).unwrap();
        db.with_conn(|conn| {
            files::index_package(conn, &installed)?;
            conn.execute(
                "INSERT INTO package_tags (package_id,tag) VALUES ('A.Asset.1','favorite')",
                [],
            )?;
            Ok(())
        })
        .unwrap();
        let p = plan(&source, &game);
        execute_group(&db, &p, &p.preview.groups[0], 0).unwrap();
        assert_eq!(fs::read(&installed).unwrap(), bytes);
        assert_eq!(
            files::fingerprint(&installed).unwrap().modified,
            "200000000000"
        );
        db.with_conn(|conn| {
            assert_eq!(
                conn.query_row("SELECT count(*) FROM package_tags", [], |r| r
                    .get::<_, i64>(0))?,
                1
            );
            assert_eq!(
                conn.query_row("SELECT count(*) FROM physical_packages", [], |r| r
                    .get::<_, i64>(0))?,
                1
            );
            Ok(())
        })
        .unwrap();
    }

    #[test]
    fn installed_winner_stays_in_place_and_incoming_duplicates_are_recycled() {
        let source = TestDir::new();
        let game = TestDir::new();
        let data = TestDir::new();
        let incoming = var(&source, "A.Asset.1.var", 100, "Custom/one.txt");
        let installed = var(
            &game,
            "AddonPackages/Author/A.Asset.1.var",
            200,
            "Custom/two.txt",
        );
        let p = plan(&source, &game);
        assert_eq!(p.preview.move_count, 0);
        assert_eq!(Path::new(&p.preview.groups[0].destination), installed);
        let db = Database::new(&data.0.join("test.db")).unwrap();
        execute_group(&db, &p, &p.preview.groups[0], 0).unwrap();
        assert!(!incoming.exists());
        assert!(installed.exists());
    }

    #[test]
    fn same_version_prefers_closest_time_and_installed_copy_on_exact_tie() {
        let source = TestDir::new();
        let game = TestDir::new();
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();
        var(&source, "a/A.Asset.1.var", now - 5, "Custom/a.txt");
        var(&source, "future/A.Asset.1.var", now + 1000, "Custom/b.txt");
        let installed = var(
            &game,
            "AddonPackages/A.Asset.1.var",
            now - 5,
            "Custom/c.txt",
        );
        let p = plan(&source, &game);
        assert_eq!(Path::new(&p.preview.groups[0].source), installed);
        assert_eq!(p.preview.retire_count, 2);
    }

    #[test]
    fn all_rules_produce_expected_relative_folders() {
        let source = TestDir::new();
        let game = TestDir::new();
        var(&source, "A.Scene.1.var", 100, "Saves/scene/demo.json");
        var(&source, "B.Plugin.1.var", 100, "Custom/Scripts/demo.cs");
        for (mode, scene_folder, plugin_folder) in [
            (MigrationMode::Flatten, "", ""),
            (MigrationMode::ByCreator, "A", "B"),
            (MigrationMode::ByType, "场景", "插件"),
            (MigrationMode::ByScene, "A", "dependencies"),
            (
                MigrationMode::Custom,
                "Favorites/Scenes",
                "Favorites/Scenes",
            ),
        ] {
            let mut c = config(&source);
            c.mode = mode;
            let p = build_plan(
                &c,
                &game.0.join("AddonPackages"),
                game.0.join(".trash"),
                HashSet::new(),
            )
            .unwrap();
            assert_eq!(
                Path::new(&p.preview.groups[0].destination),
                game.0
                    .join("AddonPackages")
                    .join(scene_folder)
                    .join("A.Scene.1.var")
            );
            assert_eq!(
                Path::new(&p.preview.groups[1].destination),
                game.0
                    .join("AddonPackages")
                    .join(plugin_folder)
                    .join("B.Plugin.1.var")
            );
        }
    }

    #[test]
    fn stale_preview_rejects_new_target_files_and_changed_source() {
        let source = TestDir::new();
        let game = TestDir::new();
        var(&source, "A.Asset.1.var", 100, "Custom/a.txt");
        let p = plan(&source, &game);
        var(&game, "AddonPackages/A.Asset.2.var", 100, "Custom/a.txt");
        assert!(validate_plan(&p).is_err());
        let p = plan(&source, &game);
        source.write("A.Asset.1.var", b"changed");
        assert!(validate_plan(&p).is_err());
    }

    #[test]
    fn rollback_restores_all_files_when_index_update_fails() {
        let source = TestDir::new();
        let game = TestDir::new();
        let data = TestDir::new();
        let incoming = var(&source, "A.Asset.1.var", 200, "Custom/new.txt");
        let installed = var(&game, "AddonPackages/A.Asset.1.var", 100, "Custom/old.txt");
        let old = fs::read(&installed).unwrap();
        let new = fs::read(&incoming).unwrap();
        let p = plan(&source, &game);
        let db = Database::new(&data.0.join("test.db")).unwrap();
        db.with_conn(|conn| {
            conn.execute_batch("CREATE TRIGGER fail_import BEFORE INSERT ON packages BEGIN SELECT RAISE(ABORT, 'injected failure'); END;")?;
            Ok(())
        }).unwrap();
        assert!(execute_group(&db, &p, &p.preview.groups[0], 0).is_err());
        assert_eq!(fs::read(&installed).unwrap(), old);
        assert_eq!(fs::read(&incoming).unwrap(), new);
        db.with_conn(|conn| {
            assert_eq!(
                conn.query_row("SELECT count(*) FROM cleanup_trash", [], |r| r
                    .get::<_, i64>(0))?,
                0
            );
            Ok(())
        })
        .unwrap();
    }

    #[test]
    fn corrupt_winner_and_managed_mapping_preserve_whole_family() {
        let source = TestDir::new();
        let game = TestDir::new();
        source.write("A.Asset.2.var", b"corrupt");
        var(&game, "AddonPackages/A.Asset.1.var", 100, "Custom/a.txt");
        let p = plan(&source, &game);
        assert!(p.preview.groups.is_empty());
        assert_eq!(p.preview.skipped, 1);
        var(&source, "A.Asset.2.var", 200, "Custom/a.txt");
        let p = build_plan(
            &config(&source),
            &game.0.join("AddonPackages"),
            game.0.join(".trash"),
            HashSet::from([files::path_key(
                &game.0.join("AddonPackages/A.Asset.1.var").to_string_lossy(),
            )]),
        )
        .unwrap();
        assert!(p.preview.groups.is_empty());
        assert_eq!(p.preview.skipped, 1);
    }

    #[test]
    fn rejects_overlapping_roots_and_unsafe_custom_paths() {
        let source = TestDir::new();
        let game = TestDir::new();
        assert!(build_plan(
            &config(&source),
            &game.0.join("AddonPackages"),
            source.0.join(".trash"),
            HashSet::new()
        )
        .is_err());
        assert!(build_plan(
            &config(&source),
            &game.0.join("AddonPackages"),
            source.0.clone(),
            HashSet::new()
        )
        .is_err());
        assert!(build_plan(
            &config(&source),
            &source.0,
            source.0.join(".trash"),
            HashSet::new()
        )
        .is_err());
        assert!(build_plan(
            &config(&source),
            &source.0.join("nested"),
            source.0.join(".trash"),
            HashSet::new()
        )
        .is_err());
        for path in [
            "",
            "../escape",
            "/absolute",
            "C:\\absolute",
            "x/../y",
            ".trash",
            "a:stream",
            "CON",
            "NUL.txt",
            "a//b",
        ] {
            assert!(custom_folder(path).is_err(), "{}", path);
        }
        assert_eq!(
            custom_folder("收藏\\场景").unwrap(),
            PathBuf::from("收藏/场景")
        );
    }
}
