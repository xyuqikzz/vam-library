use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Represents the meta.json inside a .var file
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[allow(dead_code)]
pub struct PackageMeta {
    #[serde(default)]
    pub license_type: String,
    #[serde(default)]
    pub creator_name: String,
    #[serde(default)]
    pub package_name: String,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub credits: Option<String>,
    #[serde(default)]
    pub instructions: Option<String>,
    #[serde(default)]
    pub promotional_link: Option<String>,
    #[serde(default)]
    pub content_list: Vec<String>,
    #[serde(default)]
    pub dependencies: HashMap<String, serde_json::Value>,
    #[serde(default)]
    pub custom_options: Option<HashMap<String, String>>,
}
