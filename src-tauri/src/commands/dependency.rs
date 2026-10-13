use serde::{Deserialize, Serialize};
use tauri::{State, Manager};

use crate::db::Database;
use crate::models::dependency::{DependencyEdge, DependencyGraphData, DependencyNode};
use crate::models::resource::primary_resource_type;
use crate::services::dependencies::{
    load_installed_packages, parse_dependency_parts, DependencyIndex, InstalledPackage,
};
use rusqlite::{Connection, OptionalExtension};
use std::collections::{HashSet, VecDeque};

pub use crate::services::dependency_analysis::MissingDependency;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PackageDependencyRelation {
    pub id: String,
    pub tier: usize,
    pub status: String,
    pub required_version: String,
    pub installed_version: Option<i32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PackageDependencyRelations {
    pub direct: Vec<PackageDependencyRelation>,
    pub sub: Vec<PackageDependencyRelation>,
}

/// Get the full dependency graph data (all nodes and edges).
#[tauri::command]
pub async fn get_dependency_graph(db: State<'_, Database>) -> Result<DependencyGraphData, String> {
    let db = db.inner().clone();
    tauri::async_runtime::spawn_blocking(move || dependency_graph_query(&db))
        .await.map_err(|e| e.to_string())?.map_err(|e| e.to_string())
}

fn dependency_graph_query(db: &Database) -> Result<DependencyGraphData, crate::errors::AppError> {
    let (nodes, raw_edges) = db.with_conn(|conn| {
        let tx = conn.unchecked_transaction()?;
        let conn = &tx;
        // Get all packages as nodes
        let mut pkg_stmt = conn
            .prepare(
                "SELECT p.id, p.creator, p.name, p.version, p.size_bytes, p.resource_types,
                        (SELECT COUNT(*) FROM dependencies d WHERE d.depends_on_id = p.id) as dep_count,
                        (SELECT COUNT(*) FROM dependencies d WHERE d.package_id = p.id) as dep_on_count
                 FROM packages p
                 ORDER BY p.creator, p.name",
            )
            .map_err(|e| e.to_string())?;

        let mut nodes: Vec<DependencyNode> = pkg_stmt
            .query_map([], |row| {
                let resource_types_json: String = row.get(5)?;
                let resource_types: Vec<String> =
                    serde_json::from_str(&resource_types_json).unwrap_or_default();
                let primary_type = primary_resource_type(resource_types);

                Ok(DependencyNode {
                    id: row.get(0)?,
                    creator: row.get(1)?,
                    name: row.get(2)?,
                    version: row.get(3)?,
                    size_bytes: row.get(4)?,
                    resource_type: primary_type,
                    dependents_count: row.get(6)?,
                    dependencies_count: row.get(7)?,
                    status: "ok".to_string(),
                })
            })
            .map_err(|e| e.to_string())?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| e.to_string())?;

        let mut physical_stmt = conn
            .prepare(
                "SELECT pp.file_path, pp.package_id, pp.size_bytes
                 FROM physical_packages pp
                 LEFT JOIN packages p ON p.id = pp.package_id
                 WHERE p.id IS NULL
                 ORDER BY pp.package_id",
            )
            .map_err(|e| e.to_string())?;

        let physical_nodes = physical_stmt
            .query_map([], |row| {
                let package_id: String = row.get(1)?;
                let size_bytes: i64 = row.get(2)?;
                Ok(physical_dependency_node(
                    package_id,
                    size_bytes.max(0) as u64,
                ))
            })
            .map_err(|e| e.to_string())?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| e.to_string())?;

        nodes.extend(physical_nodes);

        // Get all dependency edges
        let mut edge_stmt = conn
            .prepare(
                "SELECT d.package_id, d.depends_on_id
                 FROM dependencies d
                 WHERE EXISTS (SELECT 1 FROM packages WHERE id = d.package_id)",
            )
            .map_err(|e| e.to_string())?;

        let raw_edges: Vec<(String, String)> = edge_stmt
            .query_map([], |row| {
                Ok((row.get(0)?, row.get(1)?))
            })
            .map_err(|e| e.to_string())?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| e.to_string())?;

        Ok((nodes, raw_edges))
    })?;
    let installed_packages: Vec<InstalledPackage> = nodes
        .iter()
        .map(|node| InstalledPackage {
            id: node.id.clone(),
            creator: node.creator.clone(),
            name: node.name.clone(),
            version: node.version,
        })
        .collect();

    let index = DependencyIndex::new(&installed_packages);
    let mut resolved = std::collections::HashMap::new();
    let edges = raw_edges
        .into_iter()
        .filter_map(|(source, depends_on_id)| {
            resolved.entry(depends_on_id.to_ascii_lowercase())
                .or_insert_with(|| index.resolve(&depends_on_id)).resolved.map(|target| {
                DependencyEdge {
                    source,
                    target: target.to_string(),
                    is_transitive: false,
                }
            })
        })
        .collect();

    Ok(DependencyGraphData { nodes, edges })
}

#[tauri::command]
pub async fn get_package_dependency_relations(
    app: tauri::AppHandle,
    package_id: String,
) -> Result<PackageDependencyRelations, String> {
    let _timer = crate::services::perf::CommandTimer::new("get_package_dependency_relations");
    tauri::async_runtime::spawn_blocking(move || {
        app.state::<Database>().with_conn(|conn| package_relations_query(conn, &package_id)).map_err(|e| e.to_string())
    }).await.map_err(|e| e.to_string())?
}

fn package_relations_query(conn: &Connection, package_id: &str) -> Result<PackageDependencyRelations, crate::errors::AppError> {
    let installed_packages = load_installed_packages(conn).map_err(|e| e.to_string())?;
    let index = DependencyIndex::new(&installed_packages);
    let mut direct = Vec::new();
    let mut sub = Vec::new();
    let mut queue = VecDeque::from([(package_id.to_string(), 0_usize)]);
    let mut visited_sources = HashSet::new();
    let mut direct_keys = HashSet::new();
    let mut sub_keys = HashSet::new();

    while let Some((source_id, source_tier)) = queue.pop_front() {
        if !visited_sources.insert(source_id.to_lowercase()) {
            continue;
        }

        let dependency_ids =
            load_dependency_ids(conn, &source_id).map_err(|e| e.to_string())?;
        for dep_id in dependency_ids {
            let tier = source_tier + 1;
            let satisfaction = index.resolve(&dep_id);
            let resolved_id = satisfaction.resolved.map(str::to_string);
            let display_id = resolved_id.clone().unwrap_or_else(|| dep_id.clone());
            let relation = PackageDependencyRelation {
                id: display_id,
                tier,
                status: satisfaction.status.to_string(),
                required_version: dependency_required_version(&dep_id),
                installed_version: satisfaction.installed_version,
            };

            let relation_key = dependency_identity_key(&dep_id);
            if tier == 1 {
                if direct_keys.insert(relation_key.clone()) {
                    direct.push(relation);
                }
            } else if !direct_keys.contains(&relation_key) && sub_keys.insert(relation_key) {
                sub.push(relation);
            }

            let next_source = resolved_id
                .or_else(|| satisfaction.best.map(str::to_string));
            if let Some(next_source) = next_source {
                queue.push_back((next_source, tier));
            }
        }
    }

    direct.sort_by(|a, b| a.id.cmp(&b.id));
    sub.sort_by(|a, b| a.tier.cmp(&b.tier).then_with(|| a.id.cmp(&b.id)));

    Ok(PackageDependencyRelations { direct, sub })
}

/// Get reverse dependencies for a specific package (what depends on it).
#[tauri::command]
pub async fn get_reverse_dependencies(
    app: tauri::AppHandle,
    package_id: String,
) -> Result<Vec<DependencyNode>, String> {
    let _timer = crate::services::perf::CommandTimer::new("get_reverse_dependencies");
    tauri::async_runtime::spawn_blocking(move || {
        app.state::<Database>().with_conn(|conn| reverse_dependencies_query(conn, &package_id)).map_err(|e| e.to_string())
    }).await.map_err(|e| e.to_string())?
}

fn reverse_dependencies_query(conn: &Connection, package_id: &str) -> Result<Vec<DependencyNode>, crate::errors::AppError> {
    let installed_packages = load_installed_packages(conn).map_err(|e| e.to_string())?;
    let index = DependencyIndex::new(&installed_packages);
    let mut dep_stmt = conn
        .prepare("SELECT package_id, depends_on_id FROM dependencies ORDER BY package_id")
        .map_err(|e| e.to_string())?;
    let dep_pairs = dep_stmt
        .query_map([], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        })
        .map_err(|e| e.to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())?;

    let mut deps = Vec::new();
    let mut seen = std::collections::HashSet::new();
    for (source_id, dep_id) in dep_pairs {
        let Some(resolved_id) = index.resolve(&dep_id).resolved
        else {
            continue;
        };
        if resolved_id == package_id && seen.insert(source_id.clone()) {
            if let Some(node) =
                load_dependency_node(conn, &source_id).map_err(|e| e.to_string())?
            {
                deps.push(node);
            }
        }
    }

    Ok(deps)
}

/// Find all missing dependencies (dependencies whose target package is not installed).
#[tauri::command]
pub async fn find_missing_dependencies(
    db: State<'_, Database>,
) -> Result<Vec<MissingDependency>, String> {
    let _timer = crate::services::perf::CommandTimer::new("find_missing_dependencies");
    Ok(crate::services::dependency_analysis::get(&db).await?.missing.clone())
}

#[cfg(test)]
fn missing_dependencies_query(conn: &Connection) -> Result<Vec<MissingDependency>, crate::errors::AppError> {
    Ok(crate::services::dependency_analysis::Snapshot::load(conn)?.analyze().missing)
}

fn physical_dependency_node(package_id: String, size_bytes: u64) -> DependencyNode {
    let (creator, name, version) = package_identity_from_id(&package_id);

    DependencyNode {
        id: package_id,
        creator,
        name,
        version,
        size_bytes,
        resource_type: "other".to_string(),
        dependents_count: 0,
        dependencies_count: 0,
        status: "failed".to_string(),
    }
}

fn package_identity_from_id(package_id: &str) -> (String, String, i32) {
    if let Some(identity) = crate::services::package_identity::from_id(package_id) {
        return (identity.creator.into(), identity.name.into(), identity.version.unwrap_or(0));
    }

    (String::new(), package_id.to_string(), 0)
}

fn load_dependency_node(conn: &Connection, id: &str) -> rusqlite::Result<Option<DependencyNode>> {
    conn.query_row(
        "SELECT p.id, p.creator, p.name, p.version, p.size_bytes, p.resource_types,
                (SELECT COUNT(*) FROM dependencies d WHERE d.depends_on_id = p.id) as dep_count,
                (SELECT COUNT(*) FROM dependencies d WHERE d.package_id = p.id) as dep_on_count
         FROM packages p
         WHERE p.id = ?1",
        [id],
        |row| {
            let resource_types_json: String = row.get(5)?;
            let resource_types: Vec<String> =
                serde_json::from_str(&resource_types_json).unwrap_or_default();
            let primary_type = primary_resource_type(resource_types);

            Ok(DependencyNode {
                id: row.get(0)?,
                creator: row.get(1)?,
                name: row.get(2)?,
                version: row.get(3)?,
                size_bytes: row.get(4)?,
                resource_type: primary_type,
                dependents_count: row.get(6)?,
                dependencies_count: row.get(7)?,
                status: "ok".to_string(),
            })
        },
    )
    .optional()
}

fn load_dependency_ids(conn: &Connection, package_id: &str) -> rusqlite::Result<Vec<String>> {
    let mut stmt = conn.prepare("SELECT depends_on_id FROM dependencies WHERE package_id = ?1")?;
    let rows = stmt.query_map([package_id], |row| row.get::<_, String>(0))?;
    rows.collect()
}

fn dependency_required_version(depends_on_id: &str) -> String {
    let parts: Vec<&str> = depends_on_id.split('.').collect();
    if parts.len() >= 3 {
        parts.last().copied().unwrap_or("latest").to_string()
    } else {
        "latest".to_string()
    }
}

fn dependency_identity_key(depends_on_id: &str) -> String {
    if let Some((creator, name, _)) = parse_dependency_parts(depends_on_id) {
        format!("{}.{}", creator, name).to_lowercase()
    } else {
        depends_on_id.to_lowercase()
    }
}

#[cfg(test)]
mod performance_measurement {
    use super::*;
    #[test]
    fn graph_and_details_keep_exact_versions_and_physical_nodes() {
        let dir = crate::services::resource_files::tests::TestDir::new();
        let db = Database::new(&dir.0.join("test.db")).unwrap();
        db.with_conn(|conn| {
            conn.execute_batch("INSERT INTO packages(id,creator,name,version,file_path,scan_time) VALUES
                ('A.Scene.1','A','Scene',1,'scene','now'),
                ('B.Asset.1','B','Asset',1,'asset1','now'),
                ('B.Asset.10','B','Asset',10,'asset10','now');
                INSERT INTO physical_packages(file_path,package_id,scan_time,scan_status)
                    VALUES ('broken','C.Broken.3','now','failed');
                INSERT INTO dependencies(package_id,depends_on_id) VALUES
                    ('A.Scene.1','B.Asset.1'),('A.Scene.1','B.Asset.latest'),
                    ('A.Scene.1','B.Asset.11'),('A.Scene.1','C.Broken.3');")?;
            Ok(())
        }).unwrap();
        let graph = dependency_graph_query(&db).unwrap();
        let targets: HashSet<_> = graph.edges.iter().map(|edge| edge.target.as_str()).collect();
        assert_eq!(targets, HashSet::from(["B.Asset.1", "B.Asset.10", "C.Broken.3"]));
        assert_eq!(graph.nodes.len(), 4);
        db.with_conn(|conn| {
            let relations = package_relations_query(conn, "A.Scene.1")?;
            let asset = relations.direct.iter().find(|row| row.id == "B.Asset.1").unwrap();
            assert_eq!(asset.status, "satisfied");
            assert_eq!(asset.installed_version, Some(10));
            assert_eq!(reverse_dependencies_query(conn, "B.Asset.1")?.len(), 1);
            assert_eq!(reverse_dependencies_query(conn, "B.Asset.10")?.len(), 1);
            Ok(())
        }).unwrap();
    }
    #[test]
    #[ignore = "read-only local performance sample; requires VAM_BENCH_DB"]
    fn measure_details_queries() {
        let db = std::env::var("VAM_BENCH_DB").unwrap();
        let conn = Connection::open_with_flags(db, rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY).unwrap();
        let id = std::env::var("VAM_BENCH_PACKAGE").unwrap_or("Solerrain.Caroline.1".into());
        for _ in 0..3 {
            let t = std::time::Instant::now();
            let relations = package_relations_query(&conn, &id).unwrap();
            let relation_ms = t.elapsed().as_secs_f64()*1000.0;
            eprintln!("MEASURE details initial_dependencies={relation_ms:.2}ms direct={} sub={}", relations.direct.len(), relations.sub.len());
            if std::env::var_os("VAM_BENCH_LEGACY").is_some() {
                let t = std::time::Instant::now();
                reverse_dependencies_query(&conn, &id).unwrap();
                missing_dependencies_query(&conn).unwrap();
                missing_dependencies_query(&conn).unwrap();
                eprintln!("MEASURE legacy_extra_queries={:.2}ms", t.elapsed().as_secs_f64()*1000.0);
            }
        }
    }
}
