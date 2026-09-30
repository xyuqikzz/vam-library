use std::collections::HashSet;
use std::io::Read;
use std::path::Path;
use std::time::SystemTime;

use crate::errors::AppError;
use crate::models::meta::PackageMeta;
use crate::models::resource::{normalize_resource_types, ResourceType};
use crate::models::var_package::VarPackage;

/// Parse a .var filename into (creator, package_name, version)
/// Expected format: Creator.PackageName.Version.var
fn parse_var_filename(filename: &str) -> Result<(String, String, i32), AppError> {
    let stem = filename
        .rsplit_once('.')
        .filter(|(_, extension)| extension.eq_ignore_ascii_case("var"))
        .map(|(stem, _)| stem)
        .unwrap_or(filename);
    let parts: Vec<&str> = stem.split('.').collect();

    if parts.len() < 3 {
        return Err(AppError::Parse(format!(
            "Invalid .var filename format '{}': expected Creator.Package.Version.var",
            filename
        )));
    }

    let creator = parts[0].to_string();
    let version = parts.last().unwrap().parse::<i32>().map_err(|_| {
        AppError::Parse(format!(
            "Invalid version '{}' in filename '{}'",
            parts.last().unwrap(),
            filename
        ))
    })?;

    let name = parts[1..parts.len() - 1].join(".");

    Ok((creator, name, version))
}

/// Parse a .var file and extract its metadata and resource information.
///
/// Opens the .var file as a ZIP archive, reads meta.json, parses it into
/// PackageMeta, infers resource types from the content list, and returns
/// a fully populated VarPackage struct.
pub fn parse_var_file(path: &Path) -> Result<VarPackage, AppError> {
    parse_var_file_internal(path, false).map(|(package, _)| package)
}

/// Only the download path may fall back to a basic index. Strict scans and
/// dependency analysis must still report incomplete metadata instead of guessing.
pub fn parse_downloaded_var(path: &Path) -> Result<(VarPackage, Option<String>), AppError> {
    parse_var_file_internal(path, true)
}

fn parse_var_file_internal(
    path: &Path,
    allow_metadata_warning: bool,
) -> Result<(VarPackage, Option<String>), AppError> {
    let mut warning = None;
    // Get file metadata
    let file_metadata = std::fs::metadata(path).map_err(|e| {
        AppError::Io(format!(
            "Failed to read file metadata for '{}': {}",
            path.display(),
            e
        ))
    })?;
    let size_bytes = file_metadata.len();

    // Extract filename
    let filename = path
        .file_name()
        .and_then(|f| f.to_str())
        .ok_or_else(|| AppError::Parse(format!("Invalid file path: {}", path.display())))?;

    // Parse creator, name, version from filename
    let (creator, name, version) = parse_var_filename(filename)?;
    let id = format!("{}.{}.{}", creator, name, version);

    // Open as ZIP archive
    let file = std::fs::File::open(path).map_err(|e| {
        AppError::Io(format!(
            "Failed to open .var file '{}': {}",
            path.display(),
            e
        ))
    })?;

    let mut archive = zip::ZipArchive::new(file).map_err(|e| {
        AppError::Zip(format!(
            "Failed to read ZIP archive '{}': {}",
            path.display(),
            e
        ))
    })?;

    // Read meta.json
    let meta = match archive.by_name("meta.json") {
        Ok(mut meta_file) => {
            let mut contents = String::new();
            meta_file.read_to_string(&mut contents).map_err(|e| {
                AppError::Io(format!(
                    "Failed to read meta.json from '{}': {}",
                    path.display(),
                    e
                ))
            })?;

            match serde_json::from_str::<PackageMeta>(contents.trim_start_matches('\u{feff}')) {
                Ok(meta) => Some(meta),
                Err(error) if allow_metadata_warning => {
                    warning = Some(format!("文件已保存，meta.json 格式异常（{}）；仅建立基础索引，依赖信息不可用。原文件未修改。", error));
                    None
                }
                Err(error) => {
                    return Err(AppError::Parse(format!(
                        "Failed to parse meta.json from '{}': {}",
                        path.display(),
                        error
                    )))
                }
            }
        }
        Err(zip::result::ZipError::FileNotFound) => {
            if allow_metadata_warning {
                warning =
                    Some("文件已保存，但缺少 meta.json；仅建立基础索引，依赖信息不可用。".into());
            }
            None
        }
        Err(error) => return Err(error.into()),
    };

    // Collect resource types and file contents with uncompressed sizes
    let mut contents = Vec::new();
    let mut resource_type_set = HashSet::new();

    for i in 0..archive.len() {
        if let Ok(entry) = archive.by_index(i) {
            if entry.is_file() {
                let entry_name = entry.name().to_string();
                let entry_size = entry.size();
                contents.push((entry_name.clone(), entry_size));

                let rt = ResourceType::from_path(&entry_name);
                if rt != ResourceType::Other {
                    resource_type_set.insert(rt.as_str().to_string());
                }
            }
        }
    }

    let resource_types = normalize_resource_types(resource_type_set);

    let scan_time = chrono::Utc::now().to_rfc3339();
    let modified_time = file_metadata
        .modified()
        .map(system_time_to_rfc3339)
        .unwrap_or_else(|_| scan_time.clone());
    let created_time = file_metadata
        .created()
        .or_else(|_| file_metadata.modified())
        .map(system_time_to_rfc3339)
        .unwrap_or_else(|_| scan_time.clone());

    Ok((
        VarPackage {
            id,
            creator,
            name,
            version,
            file_path: path.to_string_lossy().to_string(),
            size_bytes,
            meta,
            resource_types,
            contents,
            created_time,
            modified_time,
            scan_time,
        },
        warning,
    ))
}

fn system_time_to_rfc3339(time: SystemTime) -> String {
    let datetime: chrono::DateTime<chrono::Utc> = time.into();
    datetime.to_rfc3339()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::services::resource_files::tests::TestDir;
    use std::io::Write;

    #[test]
    fn downloaded_package_keeps_invalid_metadata_without_inventing_dependencies() {
        let dir = TestDir::new();
        let path = dir.0.join("Author.Package.7.var");
        let mut zip = zip::ZipWriter::new(std::fs::File::create(&path).unwrap());
        zip.start_file("meta.json", zip::write::SimpleFileOptions::default())
            .unwrap();
        zip.write_all(br#"{"dependencies":{"Other.Package.1":{}},"customOptions":{}"#)
            .unwrap();
        zip.start_file(
            "Custom/Scripts/example.cs",
            zip::write::SimpleFileOptions::default(),
        )
        .unwrap();
        zip.write_all(b"// fixture").unwrap();
        zip.finish().unwrap();
        let before = std::fs::read(&path).unwrap();
        assert!(parse_var_file(&path).is_err());
        crate::services::downloader::validate_var_file(&path).unwrap();
        let (package, warning) = parse_downloaded_var(&path).unwrap();
        assert!(package.meta.is_none());
        assert_eq!(package.contents.len(), 2);
        assert!(warning.unwrap().contains("依赖信息不可用"));
        assert_eq!(std::fs::read(&path).unwrap(), before);
    }

    #[test]
    fn malformed_archive_is_not_downgraded_to_a_metadata_warning() {
        let dir = TestDir::new();
        let path = dir.write("Author.Bad.1.var", b"PKxxtruncated");
        assert!(parse_downloaded_var(&path).is_err());
        assert!(crate::services::downloader::validate_var_file(&path).is_err());
        assert!(path.exists());
    }

    #[test]
    fn archive_crc_failure_is_detected() {
        let dir = TestDir::new();
        let path = dir.0.join("Author.Bad.1.var");
        let mut zip = zip::ZipWriter::new(std::fs::File::create(&path).unwrap());
        zip.start_file(
            "meta.json",
            zip::write::SimpleFileOptions::default()
                .compression_method(zip::CompressionMethod::Stored),
        )
        .unwrap();
        zip.write_all(b"{}").unwrap();
        zip.finish().unwrap();
        let offset = {
            let mut archive = zip::ZipArchive::new(std::fs::File::open(&path).unwrap()).unwrap();
            let entry = archive.by_index(0).unwrap();
            entry.data_start() as usize
        };
        let mut bytes = std::fs::read(&path).unwrap();
        bytes[offset] = b'X';
        std::fs::write(&path, bytes).unwrap();
        assert!(crate::services::downloader::validate_var_file(&path)
            .unwrap_err()
            .contains("CRC"));
    }

    #[test]
    fn test_parse_uppercase_var_extension() {
        assert_eq!(
            parse_var_filename("Author.Resource.Name.12.VAR").unwrap(),
            ("Author".into(), "Resource.Name".into(), 12)
        );
    }

    #[test]
    fn test_parse_var_filename_valid() {
        let (creator, name, version) = parse_var_filename("AuthorName.PackageName.3.var").unwrap();
        assert_eq!(creator, "AuthorName");
        assert_eq!(name, "PackageName");
        assert_eq!(version, 3);
    }

    #[test]
    fn test_parse_var_filename_invalid_format() {
        let result = parse_var_filename("BadName.var");
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_var_filename_with_dots() {
        let (creator, name, version) =
            parse_var_filename("Author.Package.Sub.Name.15.var").unwrap();
        assert_eq!(creator, "Author");
        assert_eq!(name, "Package.Sub.Name");
        assert_eq!(version, 15);
    }

    #[test]
    fn test_parse_var_filename_invalid_version() {
        let result = parse_var_filename("Author.Package.abc.var");
        assert!(result.is_err());
    }
}
