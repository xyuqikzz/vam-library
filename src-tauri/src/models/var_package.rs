use serde::{Deserialize, Serialize};

use super::meta::PackageMeta;

/// Represents a parsed .var package
#[derive(Debug, Clone, Serialize, Deserialize)]
#[allow(dead_code)]
pub struct VarPackage {
    /// Unique identifier in "Creator.Package.Version" format
    pub id: String,
    /// Creator/author name
    pub creator: String,
    /// Package name
    pub name: String,
    /// Package version number
    pub version: i32,
    /// Absolute path to the .var file on disk
    pub file_path: String,
    /// File size in bytes
    pub size_bytes: u64,
    /// Parsed meta.json contents
    pub meta: Option<PackageMeta>,
    /// List of resource type strings found in this package
    pub resource_types: Vec<String>,
    /// List of file paths inside the package and their uncompressed sizes
    pub contents: Vec<(String, u64)>,
    /// ISO 时间戳 - 包文件在磁盘上的创建时间
    pub created_time: String,
    /// ISO timestamp of when the package was scanned
    pub scan_time: String,
}

/// Summary for list views (lighter weight than full VarPackage)
#[derive(Debug, Clone, Serialize, Deserialize)]
#[allow(dead_code)]
pub struct VarPackageSummary {
    pub id: String,
    pub creator: String,
    pub name: String,
    pub version: i32,
    pub file_path: String,
    pub size_bytes: u64,
    pub resource_types: Vec<String>,
    /// 此包依赖了多少个其他包
    pub dependency_count: usize,
    /// 有多少个其他包依赖此包
    pub dependents_count: usize,
    /// 包内文件数量
    pub content_count: usize,
    /// ISO 时间戳 - 文件创建时间
    pub created_time: String,
    /// ISO 时间戳 - 入库（扫描）时间
    pub scan_time: String,
    /// 用户自定义标签
    pub tags: Vec<String>,
}
