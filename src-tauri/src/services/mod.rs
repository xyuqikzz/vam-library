pub(crate) mod dependencies;
pub(crate) mod dependency_analysis;
pub mod download_http;
pub mod downloader;
pub mod install_context;
pub mod resource_dedup;
pub(crate) mod dedup_cache;
pub(crate) mod dedup_analysis;
pub mod resource_files;
pub mod resource_cleanup;
mod cleanup_reference_cache;
mod cleanup_json_refs;
pub mod cleanup_indexer;
pub mod scanner;
pub mod scene_browser_index;
pub mod scene_launch;
pub mod var_parser;
pub(crate) mod package_identity;
pub mod watcher;

pub mod perf;
pub mod preview_cache;

pub mod game_process;
