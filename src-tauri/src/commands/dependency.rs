use serde::{Deserialize, Serialize};
use tauri::State;

use crate::db::Database;
use crate::models::dependency::{DependencyEdge, DependencyGraphData, DependencyNode};
use crate::models::resource::primary_resource_type;
use rusqlite::{Connection, OptionalExtension};
use std::collections::{HashSet, VecDeque};

#[derive(Debug, Clone)]
struct InstalledPackage {
    id: String,
    creator: String,
    name: String,
    version: i32,
}



/// A missing dependency entry
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MissingDependency {
    pub package_id: String,
    pub depends_on_id: String,
    pub required_version: String,
    pub dependent_package: String,
    pub status: String,
    pub installed_version: Option<i32>,
}



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
    db.with_conn(|conn| {
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

        let installed_packages: Vec<InstalledPackage> = nodes
            .iter()
            .map(|node| InstalledPackage {
                id: node.id.clone(),
                creator: node.creator.clone(),
                name: node.name.clone(),
                version: node.version,
            })
            .collect();

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

        let edges = raw_edges
            .into_iter()
            .filter_map(|(source, depends_on_id)| {
                resolve_installed_dependency_id(&depends_on_id, &installed_packages).map(|target| {
                    DependencyEdge {
                        source,
                        target,
                        is_transitive: false,
                    }
                })
            })
            .collect();

        Ok(DependencyGraphData { nodes, edges })
    })
    .map_err(|e| e.to_string())
}



#[tauri::command]
pub async fn get_package_dependency_relations(
    db: State<'_, Database>,
    package_id: String,
) -> Result<PackageDependencyRelations, String> {
    db.with_conn(|conn| {
        let installed_packages = load_installed_packages(conn).map_err(|e| e.to_string())?;
        let mut direct = Vec::new();
        let mut sub = Vec::new();
        let mut queue = VecDeque::from([(package_id.clone(), 0_usize)]);
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
                let satisfaction = dependency_satisfaction(&dep_id, &installed_packages);
                let resolved_id = resolve_installed_dependency_id(&dep_id, &installed_packages);
                let display_id = resolved_id.clone().unwrap_or_else(|| dep_id.clone());
                let relation = PackageDependencyRelation {
                    id: display_id,
                    tier,
                    status: satisfaction.status,
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
                    .or_else(|| best_installed_dependency_id(&dep_id, &installed_packages));
                if let Some(next_source) = next_source {
                    queue.push_back((next_source, tier));
                }
            }
        }

        direct.sort_by(|a, b| a.id.cmp(&b.id));
        sub.sort_by(|a, b| a.tier.cmp(&b.tier).then_with(|| a.id.cmp(&b.id)));

        Ok(PackageDependencyRelations { direct, sub })
    })
    .map_err(|e| e.to_string())
}

/// Get reverse dependencies for a specific package (what depends on it).
#[tauri::command]
pub async fn get_reverse_dependencies(
    db: State<'_, Database>,
    package_id: String,
) -> Result<Vec<DependencyNode>, String> {
    db.with_conn(|conn| {
        let installed_packages = load_installed_packages(conn).map_err(|e| e.to_string())?;
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
            let Some(resolved_id) = resolve_installed_dependency_id(&dep_id, &installed_packages)
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
    })
    .map_err(|e| e.to_string())
}

/// Find all missing dependencies (dependencies whose target package is not installed).
#[tauri::command]
pub async fn find_missing_dependencies(
    db: State<'_, Database>,
) -> Result<Vec<MissingDependency>, String> {
    db.with_conn(|conn| {
        let installed_packages = load_installed_packages(conn).map_err(|e| e.to_string())?;
        let mut stmt = conn
            .prepare(
                "SELECT d.package_id, d.depends_on_id, d.required_version, p.name
                 FROM dependencies d
                 JOIN packages p ON p.id = d.package_id
                 ORDER BY d.package_id",
            )
            .map_err(|e| e.to_string())?;

        let dependencies = stmt
            .query_map([], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, String>(3)?,
                ))
            })
            .map_err(|e| e.to_string())?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| e.to_string())?;

        let missing = dependencies
            .into_iter()
            .filter_map(
                |(package_id, depends_on_id, required_version, dependent_package)| {
                    let satisfaction = dependency_satisfaction(&depends_on_id, &installed_packages);
                    if satisfaction.status == "satisfied" || satisfaction.status == "higher_version"
                    {
                        return None;
                    }
                    Some(MissingDependency {
                        package_id,
                        depends_on_id,
                        required_version,
                        dependent_package,
                        status: satisfaction.status,
                        installed_version: satisfaction.installed_version,
                    })
                },
            )
            .collect();

        Ok(missing)
    })
    .map_err(|e| e.to_string())
}



fn load_installed_packages(conn: &Connection) -> rusqlite::Result<Vec<InstalledPackage>> {
    let mut stmt = conn.prepare("SELECT id, creator, name, version FROM packages")?;
    let packages = stmt
        .query_map([], |row| {
            Ok(InstalledPackage {
                id: row.get(0)?,
                creator: row.get(1)?,
                name: row.get(2)?,
                version: row.get(3)?,
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(packages)
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
    let version_split = package_id.rsplit_once('.');
    if let Some((base, version_text)) = version_split {
        if let Ok(version) = version_text.parse::<i32>() {
            if let Some((creator, name)) = base.split_once('.') {
                return (creator.to_string(), name.to_string(), version);
            }
        }
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

fn resolve_installed_dependency_id(
    depends_on_id: &str,
    installed_packages: &[InstalledPackage],
) -> Option<String> {
    if let Some(pkg) = installed_packages
        .iter()
        .find(|pkg| pkg.id.eq_ignore_ascii_case(depends_on_id))
    {
        return Some(pkg.id.clone());
    }

    let parts: Vec<&str> = depends_on_id.split('.').collect();
    if parts.len() < 2 {
        return None;
    }

    let creator = parts[0];
    let (name, required_version) = if parts.len() >= 3 {
        let version_part = parts.last().copied().unwrap_or_default();
        (
            parts[1..parts.len() - 1].join("."),
            parse_required_version(version_part),
        )
    } else {
        (parts[1].to_string(), None)
    };

    let mut candidates: Vec<&InstalledPackage> = installed_packages
        .iter()
        .filter(|pkg| {
            pkg.creator.eq_ignore_ascii_case(creator) && pkg.name.eq_ignore_ascii_case(&name)
        })
        .collect();

    if candidates.is_empty() {
        return None;
    }

    candidates.sort_by_key(|pkg| pkg.version);

    if let Some(required) = required_version {
        candidates
            .into_iter()
            .filter(|pkg| pkg.version >= required)
            .max_by_key(|pkg| pkg.version)
            .map(|pkg| pkg.id.clone())
    } else {
        candidates
            .into_iter()
            .max_by_key(|pkg| pkg.version)
            .map(|pkg| pkg.id.clone())
    }
}

struct DependencySatisfaction {
    status: String,
    installed_version: Option<i32>,
}

fn dependency_satisfaction(
    depends_on_id: &str,
    installed_packages: &[InstalledPackage],
) -> DependencySatisfaction {
    if resolve_installed_dependency_id(depends_on_id, installed_packages).is_some() {
        return DependencySatisfaction {
            status: "satisfied".to_string(),
            installed_version: dependency_best_installed_version(depends_on_id, installed_packages),
        };
    }

    let parsed = parse_dependency_parts(depends_on_id);
    let Some((creator, name, required_version)) = parsed else {
        return DependencySatisfaction {
            status: "missing".to_string(),
            installed_version: None,
        };
    };

    let best = installed_packages
        .iter()
        .filter(|pkg| {
            pkg.creator.eq_ignore_ascii_case(&creator) && pkg.name.eq_ignore_ascii_case(&name)
        })
        .max_by_key(|pkg| pkg.version);

    match (best, required_version) {
        (Some(pkg), Some(required)) if pkg.version < required => DependencySatisfaction {
            status: "lower_version".to_string(),
            installed_version: Some(pkg.version),
        },
        (Some(pkg), _) => DependencySatisfaction {
            status: "satisfied".to_string(),
            installed_version: Some(pkg.version),
        },
        (None, _) => DependencySatisfaction {
            status: "missing".to_string(),
            installed_version: None,
        },
    }
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

fn best_installed_dependency_id(
    depends_on_id: &str,
    installed_packages: &[InstalledPackage],
) -> Option<String> {
    if let Some(pkg) = installed_packages
        .iter()
        .find(|pkg| pkg.id.eq_ignore_ascii_case(depends_on_id))
    {
        return Some(pkg.id.clone());
    }

    let (creator, name, _) = parse_dependency_parts(depends_on_id)?;
    installed_packages
        .iter()
        .filter(|pkg| {
            pkg.creator.eq_ignore_ascii_case(&creator) && pkg.name.eq_ignore_ascii_case(&name)
        })
        .max_by_key(|pkg| pkg.version)
        .map(|pkg| pkg.id.clone())
}

fn dependency_best_installed_version(
    depends_on_id: &str,
    installed_packages: &[InstalledPackage],
) -> Option<i32> {
    let (creator, name, _) = parse_dependency_parts(depends_on_id)?;
    installed_packages
        .iter()
        .filter(|pkg| {
            pkg.creator.eq_ignore_ascii_case(&creator) && pkg.name.eq_ignore_ascii_case(&name)
        })
        .map(|pkg| pkg.version)
        .max()
}

fn parse_dependency_parts(depends_on_id: &str) -> Option<(String, String, Option<i32>)> {
    let parts: Vec<&str> = depends_on_id.split('.').collect();
    if parts.len() < 2 {
        return None;
    }

    let creator = parts[0].to_string();
    if parts.len() == 2 {
        return Some((creator, parts[1].to_string(), None));
    }

    let version_part = parts.last().copied().unwrap_or_default();
    let name = parts[1..parts.len() - 1].join(".");
    Some((creator, name, parse_required_version(version_part)))
}

fn parse_required_version(version_part: &str) -> Option<i32> {
    if version_part.eq_ignore_ascii_case("latest") {
        None
    } else {
        version_part.parse::<i32>().ok()
    }
}
