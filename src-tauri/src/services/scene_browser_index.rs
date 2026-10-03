use std::{
    collections::BTreeMap,
    fs,
    io::Write,
    path::Path,
    sync::{
        atomic::{AtomicBool, AtomicU64, Ordering},
        Mutex,
    },
};

use rusqlite::Connection;
use serde::Serialize;
use tauri::Manager;

use crate::{
    db::Database,
    services::{install_context::resolve_install_context, resource_files::path_key},
};

pub const HEADER: &str = "# VAM Library import times v1";
pub const FILE_NAME: &str = "import-times.tsv";
static EXPORT_LOCK: Mutex<()> = Mutex::new(());
static SYNC_RUNNING: AtomicBool = AtomicBool::new(false);
static SYNC_REVISION: AtomicU64 = AtomicU64::new(0);
static TEMP_ID: AtomicU64 = AtomicU64::new(0);

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportTimeSync {
    pub package_count: usize,
    pub path: String,
}

fn milliseconds(value: &str) -> Option<i64> {
    chrono::DateTime::parse_from_rfc3339(value)
        .ok()
        .map(|date| date.timestamp_millis())
        .or_else(|| {
            chrono::NaiveDateTime::parse_from_str(value, "%Y-%m-%d %H:%M:%S")
                .ok()
                .map(|date| date.and_utc().timestamp_millis())
        })
        .filter(|value| *value >= 0)
}

/// Match the same stable packages.scan_time returned by the app's package list.
/// Physical copies and managed-library paths are scoped to the selected instance.
fn import_times(
    conn: &Connection,
    roots: &[String],
) -> Result<BTreeMap<String, i64>, crate::errors::AppError> {
    let prefixes: Vec<String> = roots
        .iter()
        .map(|root| format!("{}/", path_key(root)))
        .collect();
    let mut stmt = conn.prepare(
        "SELECT id, file_path, scan_time FROM packages
        UNION ALL SELECT pp.package_id, pp.file_path, COALESCE(p.scan_time, pp.scan_time)
        FROM physical_packages pp LEFT JOIN packages p ON p.id = pp.package_id",
    )?;
    let rows = stmt.query_map([], |row| {
        Ok((
            row.get::<_, String>(0)?,
            row.get::<_, String>(1)?,
            row.get::<_, String>(2)?,
        ))
    })?;
    let mut times: BTreeMap<String, i64> = BTreeMap::new();
    for row in rows {
        let (id, file, time) = row?;
        if id.is_empty()
            || id.chars().any(char::is_control)
            || !prefixes
                .iter()
                .any(|prefix| path_key(&file).starts_with(prefix))
        {
            continue;
        }
        if let Some(time) = milliseconds(&time) {
            times
                .entry(id)
                .and_modify(|old| *old = (*old).min(time))
                .or_insert(time);
        }
    }
    Ok(times)
}

fn write_snapshot(
    directory: &Path,
    times: &BTreeMap<String, i64>,
) -> Result<ImportTimeSync, String> {
    let target = directory.join(FILE_NAME);
    match fs::symlink_metadata(&target) {
        Ok(meta) => {
            #[cfg(windows)]
            let linked = {
                use std::os::windows::fs::MetadataExt;
                meta.file_attributes() & 0x400 != 0
            };
            #[cfg(not(windows))]
            let linked = meta.file_type().is_symlink();
            if linked || !meta.is_file() {
                return Err("入库时间映射路径不是普通文件".into());
            }
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => return Err(format!("检查入库时间映射失败: {error}")),
    }
    let mut text = format!("{HEADER}\n");
    for (id, time) in times {
        text.push_str(&format!("{id}\t{time}\n"));
    }
    // Avoid resetting the game's pagination when the library has not changed.
    if fs::read_to_string(&target).ok().as_deref() != Some(&text) {
        let temp = directory.join(format!(
            ".import-times-{}-{}.tmp",
            std::process::id(),
            TEMP_ID.fetch_add(1, Ordering::Relaxed)
        ));
        // Only clean up a temporary file if this exporter successfully created it.
        let mut file = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temp)
            .map_err(|error| format!("创建入库时间映射失败: {error}"))?;
        let result = (|| {
            file.write_all(text.as_bytes())?;
            file.sync_all()?;
            drop(file);
            // Rename replaces the snapshot atomically; game readers never see partial TSV.
            fs::rename(&temp, &target)
        })();
        if let Err(error) = result {
            let _ = fs::remove_file(&temp);
            return Err(format!("同步入库时间失败: {error}"));
        }
    }
    Ok(ImportTimeSync {
        package_count: times.len(),
        path: target.to_string_lossy().into_owned(),
    })
}

pub fn sync_current(app: &tauri::AppHandle) -> Result<Option<ImportTimeSync>, String> {
    let _guard = EXPORT_LOCK.lock().map_err(|error| error.to_string())?;
    let context = resolve_install_context(app)?;
    let Some(directory) =
        crate::commands::game_mods::owned_mod_directory(Path::new(&context.vam_root))?
    else {
        return Ok(None);
    };
    let times = app
        .state::<Database>()
        .with_conn(|conn| {
            import_times(conn, &[context.real_addon_dir, context.managed_library_dir])
        })
        .map_err(|error| error.to_string())?;
    write_snapshot(&directory, &times).map(Some)
}

/// Coalesce index changes; each exporter reads the latest committed DB snapshot.
pub fn schedule_sync(app: &tauri::AppHandle) {
    SYNC_REVISION.fetch_add(1, Ordering::SeqCst);
    if SYNC_RUNNING
        .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
        .is_err()
    {
        return;
    }
    let app = app.clone();
    tauri::async_runtime::spawn_blocking(move || loop {
        let revision = SYNC_REVISION.load(Ordering::SeqCst);
        if let Err(error) = sync_current(&app) {
            log::warn!("Scene browser import-time synchronization: {error}");
        }
        SYNC_RUNNING.store(false, Ordering::SeqCst);
        if SYNC_REVISION.load(Ordering::SeqCst) == revision
            || SYNC_RUNNING
                .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
                .is_err()
        {
            break;
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::schema;

    #[test]
    fn reindexing_and_moving_keep_first_import_time_but_new_versions_get_their_own() {
        let conn = Connection::open_in_memory().unwrap();
        schema::create_tables(&conn).unwrap();
        let mut package = crate::models::var_package::VarPackage {
            id: "A.Scene.1".into(),
            creator: "A".into(),
            name: "Scene".into(),
            version: 1,
            file_path: "E:/VaM/AddonPackages/A.Scene.1.var".into(),
            size_bytes: 100,
            meta: None,
            resource_types: vec!["scene".into()],
            contents: vec![("Saves/scene/Test.json".into(), 20)],
            created_time: "2000-01-01T00:00:00Z".into(),
            modified_time: "2001-01-01T00:00:00Z".into(),
            scan_time: "2026-10-01T00:00:00Z".into(),
        };
        let roots = vec![
            "E:/VaM/AddonPackages".into(),
            "D:/Library/AddonPackages".into(),
        ];
        super::super::resource_files::index_parsed_package(&conn, &package).unwrap();
        let first = import_times(&conn, &roots).unwrap();
        package.scan_time = "2026-10-02T00:00:00Z".into();
        super::super::resource_files::index_parsed_package(&conn, &package).unwrap();
        assert_eq!(import_times(&conn, &roots).unwrap(), first);
        package.file_path = "D:/Library/AddonPackages/Scene/A.Scene.1.var".into();
        super::super::resource_files::index_parsed_package(&conn, &package).unwrap();
        assert_eq!(import_times(&conn, &roots).unwrap(), first);
        package.id = "A.Scene.2".into();
        package.version = 2;
        package.file_path = "E:/VaM/AddonPackages/A.Scene.2.var".into();
        super::super::resource_files::index_parsed_package(&conn, &package).unwrap();
        let map = import_times(&conn, &roots).unwrap();
        assert_eq!(map.len(), 2);
        assert_eq!(map["A.Scene.1"], first["A.Scene.1"]);
        assert!(map["A.Scene.2"] > map["A.Scene.1"]);
    }

    #[test]
    fn exports_software_time_not_file_created_time_and_scopes_instances() {
        let conn = Connection::open_in_memory().unwrap();
        schema::create_tables(&conn).unwrap();
        for (id, file, scan) in [
            (
                "作者.Scene.1",
                "E:/VaM/AddonPackages/场景/作者.Scene.1.var",
                "2026-10-01T12:00:00+08:00",
            ),
            (
                "Other.Scene.1",
                "E:/Other/AddonPackages/Other.Scene.1.var",
                "2026-10-02T00:00:00Z",
            ),
            (
                "Boundary.Scene.1",
                "E:/VaM/AddonPackagesOther/Boundary.Scene.1.var",
                "2026-10-02T00:00:00Z",
            ),
            (
                "Managed.Scene.1",
                "D:/Library/AddonPackages/Managed.Scene.1.var",
                "2026-09-01T00:00:00Z",
            ),
            (
                "Bad.Time.1",
                "E:/VaM/AddonPackages/Bad.Time.1.var",
                "invalid",
            ),
        ] {
            conn.execute("INSERT INTO packages (id,creator,name,version,file_path,scan_time,file_created_time) VALUES (?1,'A','B',1,?2,?3,'2000-01-01T00:00:00Z')", rusqlite::params![id,file,scan]).unwrap();
        }
        conn.execute("INSERT INTO physical_packages (file_path,package_id,scan_time) VALUES ('E:/VaM/AddonPackages/copy.var','作者.Scene.1','2026-10-02T00:00:00Z')", []).unwrap();
        let roots = vec![
            "e:\\vam\\AddonPackages".into(),
            "D:/Library/AddonPackages".into(),
        ];
        let map = import_times(&conn, &roots).unwrap();
        assert_eq!(map.len(), 2);
        assert_eq!(
            map["作者.Scene.1"],
            milliseconds("2026-10-01T04:00:00Z").unwrap()
        );
        // The package's first indexed time survives rescans and path reorganization.
        conn.execute("UPDATE packages SET file_path='E:/VaM/AddonPackages/new/作者.Scene.1.var', updated_at=datetime('now') WHERE id='作者.Scene.1'", []).unwrap();
        assert_eq!(import_times(&conn, &roots).unwrap(), map);
    }

    #[test]
    fn writes_and_replaces_a_complete_snapshot_including_empty_library() {
        let root = std::env::temp_dir().join(format!(
            "vam-import-times-{}-{}",
            std::process::id(),
            TEMP_ID.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&root).unwrap();
        let mut map = BTreeMap::new();
        map.insert("作者.Scene.1".into(), 1000);
        let first = write_snapshot(&root, &map).unwrap();
        assert_eq!(first.package_count, 1);
        assert_eq!(
            fs::read_to_string(root.join(FILE_NAME)).unwrap(),
            format!("{HEADER}\n作者.Scene.1\t1000\n")
        );
        map.insert("New.Scene.1".into(), 2000);
        assert_eq!(write_snapshot(&root, &map).unwrap().package_count, 2);
        assert_eq!(fs::read_dir(&root).unwrap().count(), 1);
        write_snapshot(&root, &BTreeMap::new()).unwrap();
        assert_eq!(
            fs::read_to_string(root.join(FILE_NAME)).unwrap(),
            format!("{HEADER}\n")
        );
        fs::remove_file(root.join(FILE_NAME)).unwrap();
        fs::remove_dir(root).unwrap();
    }

    #[test]
    fn rejects_an_unknown_mod_directory() {
        let root = std::env::temp_dir().join(format!(
            "vam-unknown-mod-{}-{}",
            std::process::id(),
            TEMP_ID.fetch_add(1, Ordering::Relaxed)
        ));
        let directory = root.join("BepInEx/plugins/VamLibrary.SceneBrowser");
        fs::create_dir_all(&directory).unwrap();
        let dll = directory.join("VamLibrary.SceneBrowser.dll");
        fs::write(&dll, b"different plugin").unwrap();
        assert!(crate::commands::game_mods::owned_mod_directory(&root)
            .unwrap()
            .is_none());
        assert!(!directory.join(FILE_NAME).exists());
        fs::remove_file(dll).unwrap();
        fs::remove_dir(directory).unwrap();
        fs::remove_dir(root.join("BepInEx/plugins")).unwrap();
        fs::remove_dir(root.join("BepInEx")).unwrap();
        fs::remove_dir(root).unwrap();
    }
}
