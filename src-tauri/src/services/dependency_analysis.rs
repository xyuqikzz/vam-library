//! Immutable dependency analysis, shared by dashboard and missing-dependency requests.
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use rusqlite::Connection;
use serde::{Deserialize, Serialize};

use super::dependencies::{load_installed_packages, DependencyIndex, InstalledPackage};
use super::perf::CommandTimer;
use crate::{db::Database, errors::AppError};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct MissingDependency {
    pub package_id: String,
    pub depends_on_id: String,
    pub required_version: String,
    pub dependent_package: String,
    pub status: String,
    pub installed_version: Option<i32>,
}

pub(crate) struct DependencyAnalysis {
    pub revision: i64,
    pub missing: Vec<MissingDependency>,
}

#[derive(Default)]
pub(crate) struct AnalysisCache {
    // Async waiters don't occupy blocking workers. Owned by the actual worker so
    // cancelling its IPC waiter cannot allow a second build to start early.
    gate: Arc<tokio::sync::Mutex<()>>,
    value: Mutex<Option<Arc<DependencyAnalysis>>>,
}

/// Transactional invalidation covers every writer, including cascades, rollback,
/// bulk scans and future commands; UI notification timing is irrelevant.
pub(crate) fn init_schema(conn: &Connection) -> Result<(), AppError> {
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS dependency_analysis_revision (
            singleton INTEGER PRIMARY KEY CHECK (singleton = 1),
            revision INTEGER NOT NULL
        );
        INSERT OR IGNORE INTO dependency_analysis_revision VALUES (1, 0);",
    )?;
    for (table, events) in [
        (
            "packages",
            ["INSERT", "DELETE", "UPDATE OF id, creator, name, version"],
        ),
        ("dependencies", ["INSERT", "DELETE", "UPDATE"]),
    ] {
        for (i, event) in events.iter().enumerate() {
            conn.execute_batch(&format!(
                "CREATE TRIGGER IF NOT EXISTS dependency_revision_{table}_{i}
                 AFTER {event} ON {table} BEGIN
                   UPDATE dependency_analysis_revision SET revision = revision + 1 WHERE singleton = 1;
                 END;"
            ))?;
        }
    }
    Ok(())
}

fn revision(conn: &Connection) -> Result<i64, AppError> {
    Ok(conn.query_row(
        "SELECT revision FROM dependency_analysis_revision WHERE singleton = 1",
        [],
        |row| row.get(0),
    )?)
}

type DependencyRow = (String, String, String, String);

pub(crate) struct Snapshot {
    packages: Vec<InstalledPackage>,
    dependencies: Vec<DependencyRow>,
}

impl Snapshot {
    pub fn load(conn: &Connection) -> Result<Self, AppError> {
        let packages = load_installed_packages(conn)?;
        let mut stmt = conn.prepare(
            "SELECT d.package_id, d.depends_on_id, d.required_version, p.name
             FROM dependencies d JOIN packages p ON p.id = d.package_id
             ORDER BY d.package_id",
        )?;
        let dependencies = stmt
            .query_map([], |row| {
                Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?))
            })?
            .collect::<Result<_, _>>()?;
        Ok(Self {
            packages,
            dependencies,
        })
    }

    pub fn analyze(self) -> DependencyAnalysis {
        let _timer = CommandTimer::new("dependencies.match");
        let index = DependencyIndex::new(&self.packages);
        let mut resolved = HashMap::new();
        let mut missing = Vec::new();
        for (package_id, depends_on_id, required_version, dependent_package) in self.dependencies {
            let result = resolved
                .entry(depends_on_id.to_ascii_lowercase())
                .or_insert_with(|| index.resolve(&depends_on_id));
            if result.resolved.is_none() {
                // Deduplicate work, not dependency rows: each referring package
                // must still appear, and the dashboard counts these same rows.
                missing.push(MissingDependency {
                    package_id,
                    depends_on_id,
                    required_version,
                    dependent_package,
                    status: result.status.to_string(),
                    installed_version: result.installed_version,
                });
            }
        }
        DependencyAnalysis {
            revision: 0,
            missing,
        }
    }
}

pub(crate) async fn get(db: &Database) -> Result<Arc<DependencyAnalysis>, String> {
    let gate = {
        let _timer = CommandTimer::new("dependencies.shared_wait");
        db.dependency_analysis.gate.clone().lock_owned().await
    };
    let db = db.clone();
    let queued = CommandTimer::new("dependencies.worker_queue");
    tauri::async_runtime::spawn_blocking(move || {
        drop(queued);
        let _gate = gate;
        compute(&db, Snapshot::analyze)
    })
    .await
    .map_err(|e| e.to_string())?
    .map_err(|e| e.to_string())
}

enum Read {
    Cached(Arc<DependencyAnalysis>),
    Snapshot(i64, Snapshot),
}

fn compute(
    db: &Database,
    analyze: impl Fn(Snapshot) -> DependencyAnalysis,
) -> Result<Arc<DependencyAnalysis>, AppError> {
    for _ in 0..3 {
        let read = db.with_conn_timed("dependencies.db_wait", |conn| {
            let _timer = CommandTimer::new("dependencies.snapshot");
            // A read transaction also protects against another connection writing
            // between the revision, installed-package and dependency queries.
            let tx = conn.unchecked_transaction()?;
            let version = revision(&tx)?;
            if let Some(hit) = db
                .dependency_analysis
                .value
                .lock()
                .map_err(|e| e.to_string())?
                .as_ref()
                .filter(|hit| hit.revision == version)
            {
                return Ok(Read::Cached(hit.clone()));
            }
            Ok(Read::Snapshot(version, Snapshot::load(&tx)?))
        })?;
        let (version, snapshot) = match read {
            Read::Cached(hit) => return Ok(hit),
            Read::Snapshot(version, snapshot) => (version, snapshot),
        };
        let mut result = analyze(snapshot); // No database/cache lock during computation.
        result.revision = version;
        let result = Arc::new(result);
        let published = db.with_conn(|conn| {
            if revision(conn)? != version {
                return Ok(false);
            }
            *db.dependency_analysis
                .value
                .lock()
                .map_err(|e| e.to_string())? = Some(result.clone());
            Ok(true)
        })?;
        if published {
            return Ok(result);
        }
    }
    // Bound retries when a writer is continually changing the library. Never
    // publish an obsolete result as current or turn this failure into zero missing.
    Err(AppError::Database(
        "Library changed during dependency analysis; retry after indexing completes".into(),
    ))
}

#[cfg(test)]
mod tests {
    use super::super::dependencies::{parse_dependency_parts, resolve_installed_dependency_id};
    use super::*;
    use crate::services::resource_files::tests::TestDir;
    use std::sync::atomic::{AtomicUsize, Ordering};

    fn package(conn: &Connection, id: &str, creator: &str, name: &str, version: i32) {
        conn.execute("INSERT INTO packages(id,creator,name,version,file_path,scan_time) VALUES (?1,?2,?3,?4,?1,'now')",
            rusqlite::params![id, creator, name, version]).unwrap();
    }
    fn fixture() -> (Database, TestDir) {
        let dir = TestDir::new();
        let db = Database::new(&dir.0.join("library.db")).unwrap();
        db.with_conn(|conn| {
            package(conn, "A.Scene.1", "A", "Scene", 1);
            package(conn, "A.Scene.2", "A", "Scene", 2);
            package(conn, "B.Asset.1", "B", "Asset", 1);
            conn.execute_batch(
                "INSERT INTO dependencies(package_id,depends_on_id) VALUES
                ('A.Scene.1','B.Asset.2'), ('A.Scene.2','B.Asset.2');",
            )?;
            Ok(())
        })
        .unwrap();
        (db, dir)
    }

    fn legacy(snapshot: Snapshot) -> Vec<MissingDependency> {
        snapshot
            .dependencies
            .into_iter()
            .filter_map(
                |(package_id, depends_on_id, required_version, dependent_package)| {
                    let resolved =
                        resolve_installed_dependency_id(&depends_on_id, &snapshot.packages);
                    let best =
                        parse_dependency_parts(&depends_on_id).and_then(|(creator, name, _)| {
                            snapshot
                                .packages
                                .iter()
                                .filter(|p| {
                                    p.creator.eq_ignore_ascii_case(&creator)
                                        && p.name.eq_ignore_ascii_case(&name)
                                })
                                .max_by_key(|p| p.version)
                        });
                    if resolved.is_some() {
                        return None;
                    }
                    Some(MissingDependency {
                        package_id,
                        depends_on_id,
                        required_version,
                        dependent_package,
                        installed_version: best.map(|p| p.version),
                        status: if best.is_some() {
                            "lower_version"
                        } else {
                            "missing"
                        }
                        .into(),
                    })
                },
            )
            .collect()
    }

    #[test]
    fn indexed_resolution_and_rows_match_legacy_for_edge_cases() {
        let (db, _dir) = fixture();
        db.with_conn(|conn| {
            for (id, creator, name, version) in [
                ("b.asset.1", "b", "asset", 1), ("B.Asset.10", "B", "Asset", 10),
                ("author.Sub.Name.3", "author", "Sub.Name", 3),
                ("Äuthor.Name.1", "Äuthor", "Name", 1), ("invalid", "X", "Odd", -2),
            ] { package(conn, id, creator, name, version); }
            let packages = load_installed_packages(conn)?;
            let index = DependencyIndex::new(&packages);
            for id in ["B.Asset.1", "b.ASSET.1", "B.Asset.2", "B.Asset.11", "B.Asset.latest",
                "B.Asset.invalid", "B.Asset.2147483648", "B.Asset.-1", "B.Asset", "B.Asset.",
                "author.Sub.Name.3", "author.Sub.Name.2", "Author.Sub.Name.4", "Äuthor.Name.latest",
                "äuthor.Name.latest", "Other.Missing.1", "invalid", "", "."] {
                let new = index.resolve(id);
                assert_eq!(new.resolved, resolve_installed_dependency_id(id, &packages).as_deref(), "{id}");
                let best = parse_dependency_parts(id).and_then(|(c,n,_)| packages.iter()
                    .filter(|p| p.creator.eq_ignore_ascii_case(&c) && p.name.eq_ignore_ascii_case(&n))
                    .map(|p| p.version).max());
                assert_eq!(new.installed_version, best, "{id}");
                conn.execute("INSERT OR IGNORE INTO dependencies(package_id,depends_on_id,required_version) VALUES ('A.Scene.1',?1,'preserved')", [id])?;
            }
            let expected = legacy(Snapshot::load(conn)?);
            assert_eq!(Snapshot::load(conn)?.analyze().missing, expected);
            Ok(())
        }).unwrap();
    }

    #[tokio::test]
    async fn concurrent_requests_share_one_result_and_database_identity() {
        let (db, _dir) = fixture();
        let results = futures::future::join_all((0..12).map(|_| get(&db))).await;
        let first = results[0].as_ref().unwrap();
        assert_eq!(first.missing.len(), 2, "keep one row per referring package");
        for result in &results {
            assert!(Arc::ptr_eq(first, result.as_ref().unwrap()));
        }
        let (other, _other_dir) = fixture();
        assert!(!Arc::ptr_eq(first, &get(&other).await.unwrap()));
    }

    #[tokio::test]
    async fn commits_invalidate_but_rollbacks_and_unrelated_tags_keep_cache() {
        let (db, _dir) = fixture();
        let first = get(&db).await.unwrap();
        db.with_conn(|conn| {
            let tx = conn.unchecked_transaction()?;
            package(&tx, "B.Asset.2", "B", "Asset", 2);
            tx.rollback()?;
            conn.execute(
                "INSERT INTO package_tags(package_id,tag) VALUES ('A.Scene.1','favorite')",
                [],
            )?;
            Ok(())
        })
        .unwrap();
        assert!(Arc::ptr_eq(&first, &get(&db).await.unwrap()));
        db.with_conn(|conn| {
            package(conn, "B.Asset.2", "B", "Asset", 2);
            Ok(())
        })
        .unwrap();
        let second = get(&db).await.unwrap();
        assert!(second.revision > first.revision);
        assert!(second.missing.is_empty());
        db.with_conn(|conn| {
            conn.execute("DELETE FROM packages WHERE id='B.Asset.2'", [])?;
            Ok(())
        })
        .unwrap();
        assert_eq!(get(&db).await.unwrap().missing.len(), 2);
        db.with_conn(|conn| {
            conn.execute("UPDATE dependencies SET required_version='changed'", [])?;
            Ok(())
        })
        .unwrap();
        assert_eq!(
            get(&db).await.unwrap().missing[0].required_version,
            "changed"
        );
        db.with_conn(|conn| {
            conn.execute("DELETE FROM packages WHERE id='A.Scene.1'", [])?;
            Ok(())
        })
        .unwrap();
        assert_eq!(
            get(&db).await.unwrap().missing.len(),
            1,
            "cascade deletion invalidates"
        );
    }

    #[test]
    fn matching_releases_database_and_old_computations_cannot_publish() {
        let (db, _dir) = fixture();
        let calls = AtomicUsize::new(0);
        let result = compute(&db, |snapshot| {
            // This would deadlock if analysis still held the database mutex.
            if calls.fetch_add(1, Ordering::SeqCst) == 0 {
                db.with_conn(|conn| {
                    package(conn, "B.Asset.2", "B", "Asset", 2);
                    Ok(())
                })
                .unwrap();
            }
            snapshot.analyze()
        })
        .unwrap();
        assert_eq!(calls.load(Ordering::SeqCst), 2);
        assert!(result.missing.is_empty());
        assert!(Arc::ptr_eq(
            &result,
            &compute(&db, |_| panic!("cache should be warm")).unwrap()
        ));
    }

    #[tokio::test]
    async fn errors_are_not_cached_and_continuous_changes_have_bounded_retries() {
        let (db, _dir) = fixture();
        db.with_conn(|conn| {
            conn.execute_batch("ALTER TABLE dependencies RENAME TO saved_dependencies")?;
            Ok(())
        })
        .unwrap();
        assert!(get(&db).await.is_err());
        db.with_conn(|conn| {
            conn.execute_batch("ALTER TABLE saved_dependencies RENAME TO dependencies")?;
            Ok(())
        })
        .unwrap();
        let calls = AtomicUsize::new(0);
        assert!(compute(&db, |snapshot| {
            calls.fetch_add(1, Ordering::SeqCst);
            db.with_conn(|conn| {
                conn.execute("UPDATE dependencies SET required_version='changed'", [])?;
                Ok(())
            })
            .unwrap();
            snapshot.analyze()
        })
        .is_err());
        assert_eq!(calls.load(Ordering::SeqCst), 3);
        assert!(db.dependency_analysis.value.lock().unwrap().is_none());
        assert_eq!(get(&db).await.unwrap().missing.len(), 2);
    }

    #[test]
    #[ignore = "read-only local benchmark; requires VAM_BENCH_DB"]
    fn benchmark_local_missing_dependencies() {
        let path = std::env::var("VAM_BENCH_DB").unwrap();
        let conn =
            Connection::open_with_flags(path, rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY).unwrap();
        let tx = conn.unchecked_transaction().unwrap();
        let timer = std::time::Instant::now();
        let snapshot = Snapshot::load(&tx).unwrap();
        let read_ms = timer.elapsed().as_secs_f64() * 1000.0;
        eprintln!(
            "BENCH packages={} dependencies={} snapshot_ms={read_ms:.2}",
            snapshot.packages.len(),
            snapshot.dependencies.len()
        );
        let timer = std::time::Instant::now();
        let expected = legacy(snapshot);
        let legacy_ms = timer.elapsed().as_secs_f64() * 1000.0;
        let mut samples = Vec::new();
        for _ in 0..10 {
            let snapshot = Snapshot::load(&tx).unwrap();
            let timer = std::time::Instant::now();
            let result = snapshot.analyze();
            samples.push(timer.elapsed().as_secs_f64() * 1000.0);
            assert_eq!(result.missing, expected);
        }
        samples.sort_by(f64::total_cmp);
        eprintln!("BENCH legacy_match_ms={legacy_ms:.2} indexed_match_median_ms={:.2} indexed_match_p95_ms={:.2} missing_rows={}", samples[5], samples[9], expected.len());
    }
}
