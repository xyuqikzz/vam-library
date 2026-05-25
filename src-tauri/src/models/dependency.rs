use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[allow(dead_code)]
pub struct DependencyInfo {
    pub package_id: String,
    pub status: DependencyStatus,
    pub installed_version: Option<i32>,
    pub required_version: String, // number or "latest"
    pub size_bytes: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
#[allow(dead_code)]
pub enum DependencyStatus {
    Installed,
    Missing,
    VersionMismatch,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[allow(dead_code)]
pub struct DependencyGraphData {
    pub nodes: Vec<DependencyNode>,
    pub edges: Vec<DependencyEdge>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[allow(dead_code)]
pub struct DependencyNode {
    pub id: String,
    pub creator: String,
    pub name: String,
    pub version: i32,
    pub resource_type: String,
    pub size_bytes: u64,
    pub dependents_count: usize,
    pub dependencies_count: usize,
    pub status: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[allow(dead_code)]
pub struct DependencyEdge {
    pub source: String,
    pub target: String,
    pub is_transitive: bool,
}
