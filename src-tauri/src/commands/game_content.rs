//! File-level preset and scene management. VAR archives are always read-only.
use crate::{db::Database, services::install_context::resolve_install_context};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::{
    collections::{BTreeMap, HashSet},
    fs::{self, File, OpenOptions},
    io::{Read, Write},
    path::{Path, PathBuf},
    sync::Mutex,
};
use tauri::State;

const APPEARANCE: &str = "Custom/Atom/Person/Appearance/";
const PRESET_TYPES: &[(&str, &str)] = &[
    ("appearance", APPEARANCE),
    ("clothing", "Custom/Atom/Person/Clothing/"),
    ("hair", "Custom/Atom/Person/Hair/"),
    ("morphs", "Custom/Atom/Person/Morphs/"),
    ("skin", "Custom/Atom/Person/Skin/"),
    ("plugins", "Custom/Atom/Person/Plugins/"),
    ("animation", "Custom/Atom/Person/AnimationPresets/"),
    ("pose", "Custom/Atom/Person/Pose/"),
];
const SCENE: &str = "Saves/scene/";
const LIMIT: u64 = 32 * 1024 * 1024;
static CONTENT_LOCK: Mutex<()> = Mutex::new(());

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ContentRef {
    pub package_id: Option<String>,
    pub path: String,
}
impl ContentRef {
    fn key(&self) -> String {
        format!(
            "{}:/{}",
            self.package_id.as_deref().unwrap_or("local"),
            self.path
        )
    }
    fn validate(&self) -> Result<(), String> {
        relative(&self.path)?;
        if let Some(id) = &self.package_id {
            component(id)?;
        }
        if preset_kind(&self.path).is_none() && !is_content(&self.path, "scene") {
            return Err("不支持的资源路径".into());
        }
        Ok(())
    }
}
#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ContentItem {
    pub source: ContentRef,
    pub key: String,
    pub name: String,
    pub alias: Option<String>,
    pub favorite: bool,
    pub available: bool,
}
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ContentCatalog {
    pub items: Vec<ContentItem>,
    pub warnings: Vec<String>,
    pub directory: String,
}
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ContentDetail {
    pub json: Value,
    pub image: Option<String>,
    pub revision: String,
    pub characters: Vec<SceneCharacter>,
}
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SceneCharacter {
    pub index: usize,
    pub name: String,
    pub character: String,
    pub clothing: usize,
    pub hair: usize,
    pub morphs: usize,
    pub error: Option<String>,
}
#[derive(Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CopyReport {
    pub copied: usize,
    pub skipped: usize,
    pub errors: Vec<String>,
    pub destination: String,
}
#[derive(Clone)]
struct PackageSource {
    id: String,
    file: PathBuf,
    entries: Vec<String>,
}
struct Context {
    root: PathBuf,
    package_roots: Vec<PathBuf>,
}

fn component(value: &str) -> Result<(), String> {
    let reserved = value.split('.').next().unwrap_or("").to_ascii_uppercase();
    if value.is_empty()
        || value.trim() != value
        || value.ends_with('.')
        || value.len() > 180
        || value
            .chars()
            .any(|c| c.is_control() || "<>:\"/\\|?*".contains(c))
        || matches!(reserved.as_str(), "CON" | "PRN" | "AUX" | "NUL")
        || (reserved.len() == 4
            && (reserved.starts_with("COM") || reserved.starts_with("LPT"))
            && matches!(reserved.as_bytes()[3], b'1'..=b'9'))
    {
        return Err("名称包含非法字符、保留名称，或过长".into());
    }
    Ok(())
}
fn relative(path: &str) -> Result<(), String> {
    if path.is_empty() || path.contains('\\') {
        return Err("无效相对路径".into());
    }
    for part in path.split('/') {
        component(part)?;
    }
    Ok(())
}
fn preset_prefix(kind: &str) -> Option<&'static str> {
    PRESET_TYPES
        .iter()
        .find(|(key, _)| *key == kind)
        .map(|(_, prefix)| *prefix)
}
fn preset_kind(path: &str) -> Option<&'static str> {
    let path = path.to_ascii_lowercase();
    if !path.ends_with(".vap") {
        return None;
    }
    PRESET_TYPES
        .iter()
        .find(|(_, prefix)| path.starts_with(&prefix.to_ascii_lowercase()))
        .map(|(kind, _)| *kind)
}
fn is_content(path: &str, kind: &str) -> bool {
    if kind == "scene" {
        let path = path.to_ascii_lowercase();
        path.starts_with(&SCENE.to_ascii_lowercase()) && path.ends_with(".json")
    } else {
        preset_kind(path) == Some(kind)
    }
}
fn is_link(meta: &fs::Metadata) -> bool {
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        meta.file_attributes() & 0x400 != 0
    }
    #[cfg(not(windows))]
    {
        meta.file_type().is_symlink()
    }
}
// Reject junctions as well as symlinks, including existing ancestors of new targets.
fn safe_path(root: &Path, rel: &str) -> Result<PathBuf, String> {
    relative(rel)?;
    let mut path = root.to_path_buf();
    for part in rel.split('/') {
        path.push(part);
        match fs::symlink_metadata(&path) {
            Ok(meta) if is_link(&meta) => {
                return Err(format!("不操作链接路径: {}", path.display()))
            }
            Ok(_) => {}
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => return Err(e.to_string()),
        }
    }
    Ok(path)
}
fn context(app: &tauri::AppHandle, expected_root: &str) -> Result<Context, String> {
    let config = resolve_install_context(app)?;
    let root = fs::canonicalize(&config.vam_root).map_err(|e| e.to_string())?;
    if root != fs::canonicalize(expected_root).map_err(|e| e.to_string())? {
        return Err("游戏实例已改变，请刷新后重试".into());
    }
    if !root.join("VaM.exe").is_file() || !root.join("AddonPackages").is_dir() {
        return Err("请先配置有效的游戏目录".into());
    }
    let package_roots = [
        PathBuf::from(config.real_addon_dir),
        PathBuf::from(config.managed_library_dir),
    ]
    .into_iter()
    .filter_map(|p| fs::canonicalize(p).ok())
    .collect();
    Ok(Context {
        root,
        package_roots,
    })
}
fn packages(
    db: &Database,
    ctx: &Context,
    source: Option<&ContentRef>,
) -> Result<Vec<PackageSource>, String> {
    if source.is_some_and(|s| s.package_id.is_none()) {
        return Ok(vec![]);
    }
    if let Some(source) = source {
        source.validate()?;
        let package = package_by_id(db, ctx, source.package_id.as_deref().unwrap())?;
        return Ok(package
            .into_iter()
            .map(|mut p| {
                // Resolve the selected archive entry at read time; a package may not
                // yet have a contents index. The archive itself remains instance-scoped.
                p.entries.push(source.path.clone());
                p
            })
            .collect());
    }
    db.with_conn(|conn| {
        let mut stmt = conn.prepare("SELECT p.id, p.file_path, c.file_path FROM packages p JOIN contents c ON c.package_id = p.id WHERE (?1 IS NULL OR p.id = ?1) AND (lower(c.file_path) LIKE 'custom/atom/person/%.vap' OR lower(c.file_path) LIKE 'saves/scene/%.json') ORDER BY p.id, c.file_path")?;
        let rows = stmt.query_map([source.and_then(|s| s.package_id.as_deref())], |r| Ok((r.get::<_,String>(0)?, r.get::<_,String>(1)?, r.get::<_,String>(2)?)))?;
        let mut grouped: BTreeMap<String, PackageSource> = BTreeMap::new();
        for row in rows {
            let (id, path, entry) = row?;
            let path = PathBuf::from(path);
            grouped.entry(id.clone()).or_insert_with(|| PackageSource { id, file: path, entries: vec![] }).entries.push(entry.replace('\\', "/"));
        }
        // Canonicalize once per package, not once per entry. Missing favorites are
        // added separately; links may only resolve inside this instance's roots.
        Ok(grouped.into_values().filter(|p| fs::canonicalize(&p.file).ok().is_some_and(|actual|
            ctx.package_roots.iter().any(|root| actual.starts_with(root)))).collect())
    }).map_err(|e| e.to_string())
}

fn package_by_id(db: &Database, ctx: &Context, id: &str) -> Result<Option<PackageSource>, String> {
    component(id)?;
    let path: Option<String> = db
        .with_conn(|conn| {
            use rusqlite::OptionalExtension;
            conn.query_row("SELECT file_path FROM packages WHERE id = ?1", [id], |r| {
                r.get(0)
            })
            .optional()
            .map_err(Into::into)
        })
        .map_err(|e| e.to_string())?;
    let Some(path) = path else {
        return Ok(None);
    };
    let Some(actual) = fs::canonicalize(path)
        .ok()
        .filter(|p| p.is_file() && ctx.package_roots.iter().any(|root| p.starts_with(root)))
    else {
        return Ok(None);
    };
    Ok(Some(PackageSource {
        id: id.into(),
        file: actual,
        entries: vec![],
    }))
}

fn package_scenes(ctx: &Context, package: &PackageSource) -> Result<ContentCatalog, String> {
    let zip = zip::ZipArchive::new(File::open(&package.file).map_err(|e| e.to_string())?)
        .map_err(|e| format!("无法读取资源包: {e}"))?;
    let mut items = vec![];
    let mut warnings = vec![];
    for path in zip.file_names().filter(|path| is_content(path, "scene")) {
        let source = ContentRef {
            package_id: Some(package.id.clone()),
            path: path.into(),
        };
        match item(&ctx.root, source, true) {
            Ok(item) => items.push(item),
            Err(e) => warnings.push(format!("{path}: {e}")),
        }
    }
    items.sort_by(|a, b| a.source.path.cmp(&b.source.path));
    Ok(ContentCatalog {
        items,
        warnings,
        directory: SCENE.into(),
    })
}

#[tauri::command]
pub async fn list_package_scene_contents(
    app: tauri::AppHandle,
    db: State<'_, Database>,
    vam_root: String,
    package_id: String,
) -> Result<ContentCatalog, String> {
    let ctx = context(&app, &vam_root)?;
    let package =
        package_by_id(&db, &ctx, &package_id)?.ok_or("资源包不存在或不属于当前游戏实例")?;
    tauri::async_runtime::spawn_blocking(move || package_scenes(&ctx, &package))
        .await
        .map_err(|e| e.to_string())?
}
fn favorite_path(root: &Path, source: &ContentRef) -> Result<PathBuf, String> {
    source.validate()?;
    safe_path(
        root,
        &match &source.package_id {
            Some(id) => format!("AddonPackagesFilePrefs/{id}/{}.fav", source.path),
            None => format!("{}.fav", source.path),
        },
    )
}
fn item(root: &Path, source: ContentRef, available: bool) -> Result<ContentItem, String> {
    let favorite = favorite_path(root, &source)?.is_file();
    let name = Path::new(&source.path)
        .file_stem()
        .unwrap_or_default()
        .to_string_lossy()
        .into_owned();
    let name = if preset_kind(&source.path).is_some() {
        name.strip_prefix("Preset_")
            .filter(|name| !name.is_empty())
            .unwrap_or(&name)
            .to_string()
    } else {
        name
    };
    Ok(ContentItem {
        key: source.key(),
        source,
        name,
        favorite,
        available,
        alias: None,
    })
}
fn walk(root: &Path, rel: &str, warnings: &mut Vec<String>) -> Vec<PathBuf> {
    let start = match safe_path(root, rel) {
        Ok(p) => p,
        Err(e) => {
            warnings.push(e);
            return vec![];
        }
    };
    if !start.exists() {
        return vec![];
    }
    walkdir::WalkDir::new(start)
        .follow_links(false)
        .into_iter()
        .filter_entry(|e| e.metadata().map(|m| !is_link(&m)).unwrap_or(false))
        .filter_map(|e| match e {
            Ok(e) if e.file_type().is_file() => Some(e.into_path()),
            Ok(_) => None,
            Err(e) => {
                warnings.push(e.to_string());
                None
            }
        })
        .collect()
}
fn catalog(
    ctx: &Context,
    packages: &[PackageSource],
    kind: &str,
) -> Result<ContentCatalog, String> {
    let prefix = if kind == "scene" {
        SCENE
    } else {
        preset_prefix(kind).ok_or_else(|| "无效预设类型".to_string())?
    };
    let mut items = BTreeMap::new();
    let mut warnings = vec![];
    for package in packages {
        for path in package.entries.iter().filter(|p| is_content(p, kind)) {
            let source = ContentRef {
                package_id: Some(package.id.clone()),
                path: path.clone(),
            };
            match item(&ctx.root, source, package.file.is_file()) {
                Ok(i) => {
                    items.insert(i.key.clone(), i);
                }
                Err(e) => warnings.push(format!("{path}: {e}")),
            }
        }
    }
    for path in walk(&ctx.root, prefix.trim_end_matches('/'), &mut warnings) {
        let rel = path
            .strip_prefix(&ctx.root)
            .map_err(|e| e.to_string())?
            .to_string_lossy()
            .replace('\\', "/");
        let path = rel.strip_suffix(".fav").unwrap_or(&rel);
        if !is_content(path, kind) {
            continue;
        }
        let source = ContentRef {
            package_id: None,
            path: path.into(),
        };
        match item(&ctx.root, source, ctx.root.join(path).is_file()) {
            Ok(i) => {
                items.insert(i.key.clone(), i);
            }
            Err(e) => warnings.push(e),
        }
    }
    // Include orphan favorites so deleted/unindexed scenes can still be managed.
    if kind == "scene" {
        for path in walk(&ctx.root, "AddonPackagesFilePrefs", &mut warnings) {
            let rel = path
                .strip_prefix(ctx.root.join("AddonPackagesFilePrefs"))
                .map_err(|e| e.to_string())?
                .to_string_lossy()
                .replace('\\', "/");
            if let Some((id, path)) = rel.strip_suffix(".fav").and_then(|p| p.split_once('/')) {
                if !is_content(path, kind) {
                    continue;
                }
                let source = ContentRef {
                    package_id: Some(id.into()),
                    path: path.into(),
                };
                if !items.contains_key(&source.key()) {
                    match item(&ctx.root, source, false) {
                        Ok(i) => {
                            items.insert(i.key.clone(), i);
                        }
                        Err(e) => warnings.push(e),
                    }
                }
            }
        }
    }
    Ok(ContentCatalog {
        items: items.into_values().collect(),
        warnings,
        directory: prefix.trim_end_matches('/').into(),
    })
}
fn bounded_read(reader: impl Read, size: u64) -> Result<Vec<u8>, String> {
    if size > LIMIT {
        return Err("文件超过 32 MiB 安全读取上限".into());
    }
    let mut bytes = vec![];
    reader
        .take(LIMIT + 1)
        .read_to_end(&mut bytes)
        .map_err(|e| e.to_string())?;
    if bytes.len() as u64 > LIMIT {
        return Err("文件过大".into());
    }
    Ok(bytes)
}
fn zip_read(zip: &mut zip::ZipArchive<File>, path: &str) -> Result<Vec<u8>, String> {
    let entry = zip.by_name(path).map_err(|e| format!("{path}: {e}"))?;
    let size = entry.size();
    bounded_read(entry, size)
}
fn source_package<'a>(
    packages: &'a [PackageSource],
    source: &ContentRef,
) -> Result<&'a PackageSource, String> {
    packages
        .iter()
        .find(|p| Some(&p.id) == source.package_id.as_ref() && p.entries.contains(&source.path))
        .ok_or_else(|| "资源未被当前游戏实例索引，请重新扫描".into())
}
fn read_content(
    ctx: &Context,
    packages: &[PackageSource],
    source: &ContentRef,
) -> Result<Vec<u8>, String> {
    source.validate()?;
    if source.package_id.is_some() {
        let p = source_package(packages, source)?;
        let mut zip = zip::ZipArchive::new(File::open(&p.file).map_err(|e| e.to_string())?)
            .map_err(|e| e.to_string())?;
        zip_read(&mut zip, &source.path)
    } else {
        let file = File::open(safe_path(&ctx.root, &source.path)?).map_err(|e| e.to_string())?;
        let size = file.metadata().map_err(|e| e.to_string())?.len();
        bounded_read(file, size)
    }
}
fn parse_json(bytes: &[u8]) -> Result<Value, String> {
    let value: Value =
        serde_json::from_slice(bytes.strip_prefix(&[0xef, 0xbb, 0xbf]).unwrap_or(bytes))
            .map_err(|e| format!("无效预设/场景 JSON: {e}"))?;
    if !value.is_object() {
        return Err("资源 JSON 必须为对象".into());
    }
    Ok(value)
}
fn image_candidates(path: &str) -> Vec<String> {
    let stem = path.rsplit_once('.').map(|(s, _)| s).unwrap_or(path);
    ["jpg", "png", "jpeg"]
        .iter()
        .map(|ext| format!("{stem}.{ext}"))
        .collect()
}
fn preview_file(
    ctx: &Context,
    packages: &[PackageSource],
    source: &ContentRef,
) -> Result<Option<(String, Vec<u8>)>, String> {
    for path in image_candidates(&source.path) {
        let bytes = if source.package_id.is_some() {
            let package = source_package(packages, source)?;
            let mut zip =
                zip::ZipArchive::new(File::open(&package.file).map_err(|e| e.to_string())?)
                    .map_err(|e| e.to_string())?;
            if zip.index_for_name(&path).is_none() {
                continue;
            }
            zip_read(&mut zip, &path)?
        } else {
            let path = safe_path(&ctx.root, &path)?;
            if !path.is_file() {
                continue;
            }
            let file = File::open(path).map_err(|e| e.to_string())?;
            let size = file.metadata().map_err(|e| e.to_string())?.len();
            bounded_read(file, size)?
        };
        return Ok(Some((path, bytes)));
    }
    Ok(None)
}

fn preview(
    ctx: &Context,
    packages: &[PackageSource],
    source: &ContentRef,
) -> Result<Option<String>, String> {
    Ok(preview_file(ctx, packages, source)?.map(|(path, bytes)| {
        let mime = if path.ends_with(".png") {
            "image/png"
        } else {
            "image/jpeg"
        };
        format!("data:{mime};base64,{}", super::packages::base64(&bytes))
    }))
}

// A copied preset must continue resolving assets from its read-only source VAR.
fn qualify_refs(value: &mut Value, id: &str, parent: &str, entries: &HashSet<String>) {
    match value {
        Value::String(s) => {
            let normalized = s.replace('\\', "/");
            if let Some(path) = normalized.strip_prefix("SELF:/") {
                *s = format!("{id}:/{path}");
            } else if !normalized.contains(':') {
                if entries.contains(&normalized) {
                    *s = format!("{id}:/{normalized}");
                } else {
                    let mut parts: Vec<&str> =
                        parent.split('/').filter(|s| !s.is_empty()).collect();
                    let mut valid = true;
                    for part in normalized.split('/') {
                        match part {
                            "." => {}
                            ".." => {
                                if parts.pop().is_none() {
                                    valid = false;
                                    break;
                                }
                            }
                            p => parts.push(p),
                        }
                    }
                    let path = parts.join("/");
                    if valid && entries.contains(&path) {
                        *s = format!("{id}:/{path}");
                    }
                }
            }
        }
        Value::Array(values) => values
            .iter_mut()
            .for_each(|v| qualify_refs(v, id, parent, entries)),
        Value::Object(values) => values
            .values_mut()
            .for_each(|v| qualify_refs(v, id, parent, entries)),
        _ => {}
    }
}
fn write_new(path: &Path, data: &[u8]) -> Result<(), String> {
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .map_err(|e| format!("{}: {e}", path.display()))?;
    if let Err(error) = file.write_all(data).and_then(|_| file.sync_all()) {
        drop(file);
        let cleanup = fs::remove_file(path);
        return Err(format!("写入失败: {error}; 清理结果: {cleanup:?}"));
    }
    Ok(())
}

fn content_revision(bytes: &[u8]) -> String {
    hex::encode(Sha256::digest(bytes))
}

// Appearance preset storables observed in VaM 1.22. Clothing/hair storables use
// the geometry item's internalId, not its package-qualified resource path.
fn appearance_from_atom(atom: &Value) -> Result<Value, String> {
    if atom["type"].as_str() != Some("Person") {
        return Err("所选对象不是角色".into());
    }
    let storables = atom["storables"]
        .as_array()
        .ok_or("角色缺少 storables 数据")?;
    let geometry = storables
        .iter()
        .find(|s| s["id"] == "geometry")
        .ok_or("角色未保存完整外观数据，请在游戏内加载并重新保存场景后重试")?;
    if geometry["character"].as_str().is_none_or(|s| s.is_empty()) {
        return Err("角色缺少基础模型，无法离线生成完整外观预设".into());
    }
    let item_ids: Vec<&str> = ["clothing", "hair"]
        .into_iter()
        .filter_map(|key| geometry[key].as_array())
        .flatten()
        .filter_map(|item| {
            item["internalId"].as_str().or_else(|| {
                item["id"]
                    .as_str()
                    .filter(|id| !id.contains('/') && !id.contains('\\'))
            })
        })
        .filter(|id| !id.is_empty())
        .collect();
    let mut output = Vec::new();
    let mut ids = HashSet::new();
    for storable in storables {
        let Some(id) = storable["id"].as_str() else {
            continue;
        };
        let fixed = matches!(
            id,
            "geometry"
                | "BendFix"
                | "FemaleAnatomy"
                | "MaleAnatomy"
                | "BreastControl"
                | "GluteControl"
                | "BreastInOut"
                | "LowerPhysicsMesh"
                | "BreastPhysicsMesh"
                | "EyelidControl"
                | "AutoJawMouthMorph"
                | "AutoExpressions"
                | "FemaleEyelashes"
                | "MaleEyelashes"
                | "irises"
                | "sclera"
                | "lacrimals"
                | "SoftBodyPhysicsEnabler"
                | "rescaleObject"
                | "skin"
                | "textures"
                | "teeth"
                | "tongue"
                | "mouth"
                | "genitals"
        );
        let wearable = !id.starts_with("plugin#")
            && item_ids.iter().any(|prefix| {
                id.strip_prefix(prefix).is_some_and(|suffix| {
                    matches!(suffix, "Sim" | "ItemControl" | "WrapControl")
                        || suffix.starts_with("Material")
                        || suffix.ends_with("ScalpMaterial")
                })
            });
        if !fixed && !wearable {
            continue;
        }
        if !ids.insert(id) {
            return Err(format!("角色外观数据包含重复模块: {id}"));
        }
        let mut value = storable.clone();
        if id == "geometry" {
            // Scene geometry can carry unrelated runtime state. Keep only preset fields.
            value
                .as_object_mut()
                .ok_or("无效角色外观数据")?
                .retain(|key, _| {
                    matches!(
                        key.as_str(),
                        "id" | "character"
                            | "clothing"
                            | "hair"
                            | "morphs"
                            | "useAdvancedColliders"
                            | "useAuxBreastColliders"
                            | "disableAnatomy"
                            | "useMaleMorphsOnFemale"
                            | "useFemaleMorphsOnMale"
                    )
                });
        }
        output.push(value);
    }
    Ok(serde_json::json!({"setUnlistedParamsToDefault": "true", "storables": output}))
}

fn scene_characters(json: &Value) -> Vec<SceneCharacter> {
    json["atoms"]
        .as_array()
        .into_iter()
        .flatten()
        .enumerate()
        .filter(|(_, atom)| atom["type"] == "Person")
        .map(|(index, atom)| {
            let geometry = atom["storables"]
                .as_array()
                .and_then(|s| s.iter().find(|v| v["id"] == "geometry"));
            let count = |key: &str| {
                geometry
                    .and_then(|g| g[key].as_array())
                    .map_or(0, |values| {
                        values
                            .iter()
                            .filter(|v| v["enabled"] != false && v["enabled"] != "false")
                            .count()
                    })
            };
            SceneCharacter {
                index,
                name: atom["id"]
                    .as_str()
                    .filter(|s| !s.is_empty())
                    .map(str::to_owned)
                    .unwrap_or_else(|| format!("Person {}", index + 1)),
                character: geometry
                    .and_then(|g| g["character"].as_str())
                    .unwrap_or("")
                    .into(),
                clothing: count("clothing"),
                hair: count("hair"),
                morphs: count("morphs"),
                error: appearance_from_atom(atom).err(),
            }
        })
        .collect()
}

// Local texture paths are relative to the scene directory. Relocate them to
// game-root paths before placing the preset under Custom/Atom/Person/Appearance.
fn rebase_local_refs(value: &mut Value, root: &Path, parent: &str) -> Result<(), String> {
    match value {
        Value::String(s) => {
            let normalized = s.replace('\\', "/");
            if normalized.starts_with("SELF:/") {
                return Err(
                    "本地场景含有无法确定来源包的 SELF 资源引用，请先在游戏中重新保存场景".into(),
                );
            }
            if normalized.contains(':') || normalized.starts_with('/') {
                return Ok(());
            }
            let extension = normalized
                .rsplit_once('.')
                .map(|(_, ext)| ext.to_ascii_lowercase());
            if !extension.is_some_and(|ext| {
                matches!(
                    ext.as_str(),
                    "jpg"
                        | "jpeg"
                        | "png"
                        | "tif"
                        | "tiff"
                        | "bmp"
                        | "dds"
                        | "vam"
                        | "vmi"
                        | "vmb"
                        | "vap"
                )
            }) {
                return Ok(());
            }
            let lower = normalized.to_ascii_lowercase();
            if lower.starts_with("custom/") || lower.starts_with("saves/") {
                return Ok(());
            }
            let mut parts: Vec<&str> = parent.split('/').collect();
            for part in normalized.split('/') {
                match part {
                    "." | "" => {}
                    ".." => {
                        parts.pop().ok_or("资源引用超出游戏目录")?;
                    }
                    part => parts.push(part),
                }
            }
            let path = parts.join("/");
            // Validate even missing references, so moving a preset cannot silently
            // change where VaM resolves them. The original dependencies remain required.
            safe_path(root, &path)?;
            *s = path;
        }
        Value::Array(values) => {
            for value in values {
                rebase_local_refs(value, root, parent)?;
            }
        }
        Value::Object(values) => {
            for (key, value) in values {
                if key != "id" && key != "internalId" && key != "name" {
                    rebase_local_refs(value, root, parent)?;
                } else if key == "id"
                    && value
                        .as_str()
                        .is_some_and(|s| s.contains('/') || s.contains('\\'))
                {
                    rebase_local_refs(value, root, parent)?;
                }
            }
        }
        _ => {}
    }
    Ok(())
}

fn save_scene_appearance(
    ctx: &Context,
    packages: &[PackageSource],
    source: &ContentRef,
    revision: &str,
    atom_index: usize,
    name: &str,
) -> Result<String, String> {
    source.validate()?;
    if !is_content(&source.path, "scene") {
        return Err("请选择场景文件".into());
    }
    let filename = preset_filename(name)?;
    let bytes = read_content(ctx, packages, source)?;
    if content_revision(&bytes) != revision {
        return Err("场景已改变，请重新选择场景后再保存角色".into());
    }
    let scene = parse_json(&bytes)?;
    let atom = scene["atoms"]
        .as_array()
        .and_then(|atoms| atoms.get(atom_index))
        .ok_or("角色不存在，请重新选择场景")?;
    let mut preset = appearance_from_atom(atom)?;
    let parent = source.path.rsplit_once('/').map(|(p, _)| p).unwrap_or("");
    if source.package_id.is_some() {
        let package = source_package(packages, source)?;
        let zip = zip::ZipArchive::new(File::open(&package.file).map_err(|e| e.to_string())?)
            .map_err(|e| e.to_string())?;
        let entries = zip.file_names().map(str::to_owned).collect();
        qualify_refs(&mut preset, &package.id, parent, &entries);
    }
    // Also handle scene-relative loose assets referenced by a VAR.
    rebase_local_refs(&mut preset, &ctx.root, parent)?;
    let destination = format!("{APPEARANCE}VAM Library/Extracted/{filename}");
    let target = safe_path(&ctx.root, &destination)?;
    if target.exists()
        || ctx.root.join(format!("{destination}.fav")).exists()
        || ctx.root.join(format!("{destination}.hide")).exists()
        || image_candidates(&destination)
            .iter()
            .any(|p| ctx.root.join(p).exists())
    {
        return Err("该名称的预设、预览图或标记已存在，请换一个名称（原文件保留）".into());
    }
    let output = serde_json::to_vec_pretty(&preset).map_err(|e| e.to_string())?;
    // Use exactly the same scene preview as the extraction panel, retaining its
    // real extension. Read and validate it before creating either output file.
    let thumbnail = preview_file(ctx, packages, source)?.map(|(path, bytes)| {
        let extension = path.rsplit_once('.').unwrap().1;
        (target.with_extension(extension), bytes)
    });
    if let Some((path, _)) = &thumbnail {
        safe_path(
            &ctx.root,
            &path
                .strip_prefix(&ctx.root)
                .map_err(|e| e.to_string())?
                .to_string_lossy()
                .replace('\\', "/"),
        )?;
    }
    fs::create_dir_all(target.parent().ok_or("无效保存目录")?).map_err(|e| e.to_string())?;
    write_new(&target, &output)?;
    if let Some((path, bytes)) = thumbnail {
        if let Err(e) = write_new(&path, &bytes) {
            let cleanup = fs::remove_file(&target);
            return Err(format!("预览图保存失败: {e}; 预设回退结果: {cleanup:?}"));
        }
    }
    Ok(destination)
}

#[tauri::command]
pub async fn save_game_scene_appearance(
    app: tauri::AppHandle,
    db: State<'_, Database>,
    vam_root: String,
    source: ContentRef,
    revision: String,
    atom_index: usize,
    name: String,
) -> Result<String, String> {
    let ctx = context(&app, &vam_root)?;
    let sources = packages(&db, &ctx, Some(&source))?;
    tauri::async_runtime::spawn_blocking(move || {
        let _guard = CONTENT_LOCK.lock().map_err(|e| e.to_string())?;
        save_scene_appearance(&ctx, &sources, &source, &revision, atom_index, &name)
    })
    .await
    .map_err(|e| e.to_string())?
}

fn copy_preset(
    ctx: &Context,
    package: &PackageSource,
    path: &str,
    zip: &mut zip::ZipArchive<File>,
) -> Result<bool, String> {
    component(&package.id)?;
    let kind = preset_kind(path).ok_or_else(|| "不支持的预设类型".to_string())?;
    let prefix = preset_prefix(kind).ok_or_else(|| "不支持的预设目录".to_string())?;
    relative(path)?;
    let mut json = parse_json(&zip_read(zip, path)?)?;
    if !json.get("storables").is_some_and(Value::is_array) {
        return Err("预设缺少 storables".into());
    }
    let entries = zip.file_names().map(str::to_owned).collect();
    qualify_refs(
        &mut json,
        &package.id,
        path.rsplit_once('/').map(|(p, _)| p).unwrap_or(""),
        &entries,
    );
    let relative_name = &path[prefix.len()..];
    let destination = format!("{prefix}VAM Library/{}/{relative_name}", package.id);
    let target = safe_path(&ctx.root, &destination)?;
    let mut outputs = vec![(
        target.clone(),
        serde_json::to_vec_pretty(&json).map_err(|e| e.to_string())?,
    )];
    for image in image_candidates(path) {
        if zip.index_for_name(&image).is_some() {
            let ext = image.rsplit_once('.').unwrap().1;
            let image_target = target.with_extension(ext);
            safe_path(
                &ctx.root,
                &image_target
                    .strip_prefix(&ctx.root)
                    .map_err(|e| e.to_string())?
                    .to_string_lossy()
                    .replace('\\', "/"),
            )?;
            outputs.push((image_target, zip_read(zip, &image)?));
        }
    }
    // Validate all companion conflicts before writing any file.
    let mut pending = vec![];
    for (path, bytes) in outputs {
        match fs::read(&path) {
            Ok(existing) if existing == bytes => {}
            Ok(_) => return Err(format!("同名内容不同，保留原文件: {}", path.display())),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => pending.push((path, bytes)),
            Err(e) => return Err(e.to_string()),
        }
    }
    if pending.is_empty() {
        return Ok(false);
    }
    fs::create_dir_all(target.parent().unwrap()).map_err(|e| e.to_string())?;
    let mut created = vec![];
    for (path, bytes) in pending {
        if let Err(e) = write_new(&path, &bytes) {
            let failures: Vec<_> = created
                .iter()
                .filter_map(|p| fs::remove_file(p).err())
                .collect();
            return Err(format!("{e}; 回退错误: {failures:?}"));
        }
        created.push(path);
    }
    Ok(true)
}
fn copy_all(
    ctx: &Context,
    packages: &[PackageSource],
    only: Option<&ContentRef>,
    kind: &str,
) -> CopyReport {
    let Some(prefix) = preset_prefix(kind) else {
        return CopyReport {
            errors: vec!["无效预设类型".into()],
            ..Default::default()
        };
    };
    let mut report = CopyReport {
        destination: ctx.root.join(prefix).to_string_lossy().into_owned(),
        ..Default::default()
    };
    for package in packages {
        let paths: Vec<_> = package
            .entries
            .iter()
            .filter(|p| {
                is_content(p, kind)
                    && only
                        .map(|s| s.package_id.as_ref() == Some(&package.id) && &s.path == *p)
                        .unwrap_or(true)
            })
            .collect();
        if paths.is_empty() {
            continue;
        }
        let zip = File::open(&package.file)
            .map_err(|e| e.to_string())
            .and_then(|f| zip::ZipArchive::new(f).map_err(|e| e.to_string()));
        match zip {
            Ok(mut zip) => {
                for path in paths {
                    match copy_preset(ctx, package, path, &mut zip) {
                        Ok(true) => report.copied += 1,
                        Ok(false) => report.skipped += 1,
                        Err(e) => report.errors.push(format!("{}:/{path}: {e}", package.id)),
                    }
                }
            }
            Err(e) => report.errors.push(format!("{}: {e}", package.id)),
        }
    }
    report
}
fn set_favorite(
    ctx: &Context,
    packages: &[PackageSource],
    source: &ContentRef,
    favorite: bool,
) -> Result<(), String> {
    if !is_content(&source.path, "scene") {
        return Err("只支持场景收藏".into());
    }
    let marker = favorite_path(&ctx.root, source)?;
    if favorite {
        // VaM favorites depend on file existence, not JSON size or parseability.
        // Check the exact entry without loading potentially very large scenes.
        if source.package_id.is_some() {
            let package = source_package(packages, source)?;
            let mut archive =
                zip::ZipArchive::new(File::open(&package.file).map_err(|e| e.to_string())?)
                    .map_err(|e| e.to_string())?;
            let entry = archive.by_name(&source.path).map_err(|e| e.to_string())?;
            if !entry.is_file() {
                return Err("场景文件不存在".into());
            }
        } else if !safe_path(&ctx.root, &source.path)?.is_file() {
            return Err("场景文件不存在，请刷新".into());
        }
        if marker.is_file() {
            return Ok(());
        }
        fs::create_dir_all(marker.parent().unwrap()).map_err(|e| e.to_string())?;
        write_new(&marker, b"")
    } else {
        match fs::remove_file(marker) {
            Ok(()) => Ok(()),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(e) => Err(e.to_string()),
        }
    }
}
fn preset_filename(name: &str) -> Result<String, String> {
    let name = name.trim();
    component(name)?;
    let name = name.trim_start_matches("Preset_");
    component(name)?;
    let filename = format!("Preset_{name}.vap");
    component(&filename)?;
    Ok(filename)
}
fn rename_preset(ctx: &Context, source: &ContentRef, name: &str) -> Result<ContentRef, String> {
    source.validate()?;
    if source.package_id.is_some() || preset_kind(&source.path).is_none() {
        return Err("请先复制到游戏目录，再修改本地预设名称".into());
    }
    let filename = preset_filename(name)?;
    let from = safe_path(&ctx.root, &source.path)?;
    if !from.is_file() {
        return Err("预设不存在，请刷新".into());
    }
    let to = from.with_file_name(filename);
    if from == to {
        return Ok(source.clone());
    }
    let relative_to = to
        .strip_prefix(&ctx.root)
        .map_err(|e| e.to_string())?
        .to_string_lossy()
        .replace('\\', "/");
    safe_path(&ctx.root, &relative_to)?;
    let mut pairs = vec![(from.clone(), to.clone())];
    for ext in ["jpg", "jpeg", "png", "vap.fav", "vap.hide"] {
        let old = from.with_extension(ext);
        let new = to.with_extension(ext);
        safe_path(
            &ctx.root,
            &old.strip_prefix(&ctx.root)
                .unwrap()
                .to_string_lossy()
                .replace('\\', "/"),
        )?;
        safe_path(
            &ctx.root,
            &new.strip_prefix(&ctx.root)
                .unwrap()
                .to_string_lossy()
                .replace('\\', "/"),
        )?;
        if old.is_file() {
            pairs.push((old, new));
        }
    }
    if pairs.iter().any(|(_, new)| new.exists()) {
        return Err("目标名称或预览图已存在（仅更改大小写时请先使用一个临时名称）".into());
    }
    // Hard links reserve destinations without replacing files. Originals remain until all links exist.
    let mut linked: Vec<PathBuf> = vec![];
    for (old, new) in &pairs {
        if let Err(e) = fs::hard_link(old, new) {
            let failures: Vec<_> = linked
                .iter()
                .filter_map(|p| fs::remove_file(p).err())
                .collect();
            return Err(format!("改名失败，原文件保留: {e}; 回退错误: {failures:?}"));
        }
        linked.push(new.clone());
    }
    for (old, _) in &pairs {
        if let Err(e) = fs::remove_file(old) {
            return Err(format!("新名称已创建，但旧文件清理失败，请刷新检查: {e}"));
        }
    }
    Ok(ContentRef {
        package_id: None,
        path: relative_to,
    })
}

#[tauri::command]
pub async fn list_game_contents(
    app: tauri::AppHandle,
    db: State<'_, Database>,
    vam_root: String,
    kind: String,
) -> Result<ContentCatalog, String> {
    let ctx = context(&app, &vam_root)?;
    let sources = packages(&db, &ctx, None)?;
    let aliases: BTreeMap<String, String> = db
        .with_conn(|conn| {
            let mut stmt = conn
                .prepare("SELECT resource_key, name FROM game_content_names WHERE vam_root = ?1")?;
            let rows = stmt.query_map([ctx.root.to_string_lossy().as_ref()], |r| {
                Ok((r.get(0)?, r.get(1)?))
            })?;
            Ok(rows.collect::<Result<_, _>>()?)
        })
        .map_err(|e| e.to_string())?;
    tauri::async_runtime::spawn_blocking(move || {
        let _guard = CONTENT_LOCK.lock().map_err(|e| e.to_string())?;
        let mut result = catalog(&ctx, &sources, &kind)?;
        for item in &mut result.items {
            item.alias = aliases.get(&item.key).cloned();
        }
        Ok(result)
    })
    .await
    .map_err(|e| e.to_string())?
}
#[tauri::command]
pub async fn get_game_content_detail(
    app: tauri::AppHandle,
    db: State<'_, Database>,
    vam_root: String,
    source: ContentRef,
) -> Result<ContentDetail, String> {
    let ctx = context(&app, &vam_root)?;
    let sources = packages(&db, &ctx, Some(&source))?;
    tauri::async_runtime::spawn_blocking(move || {
        let bytes = read_content(&ctx, &sources, &source)?;
        let json = parse_json(&bytes)?;
        Ok(ContentDetail {
            revision: content_revision(&bytes),
            characters: if is_content(&source.path, "scene") {
                scene_characters(&json)
            } else {
                vec![]
            },
            json,
            image: preview(&ctx, &sources, &source)?,
        })
    })
    .await
    .map_err(|e| e.to_string())?
}
#[tauri::command]
pub async fn get_game_content_image(
    app: tauri::AppHandle,
    db: State<'_, Database>,
    vam_root: String,
    source: ContentRef,
) -> Result<Option<String>, String> {
    source.validate()?;
    let ctx = context(&app, &vam_root)?;
    let sources = packages(&db, &ctx, Some(&source))?;
    tauri::async_runtime::spawn_blocking(move || preview(&ctx, &sources, &source))
        .await
        .map_err(|e| e.to_string())?
}
#[tauri::command]
pub async fn copy_game_presets(
    app: tauri::AppHandle,
    db: State<'_, Database>,
    vam_root: String,
    kind: String,
    source: Option<ContentRef>,
) -> Result<CopyReport, String> {
    preset_prefix(&kind).ok_or_else(|| "无效预设类型".to_string())?;
    let ctx = context(&app, &vam_root)?;
    let sources = packages(&db, &ctx, source.as_ref())?;
    if let Some(source) = &source {
        source.validate()?;
        source_package(&sources, source)?;
        if !is_content(&source.path, &kind) {
            return Err("所选文件与当前预设类型不匹配".into());
        }
    }
    tauri::async_runtime::spawn_blocking(move || {
        let _guard = CONTENT_LOCK.lock().map_err(|e| e.to_string())?;
        Ok(copy_all(&ctx, &sources, source.as_ref(), &kind))
    })
    .await
    .map_err(|e| e.to_string())?
}
#[tauri::command]
pub async fn set_game_scene_favorite(
    app: tauri::AppHandle,
    db: State<'_, Database>,
    vam_root: String,
    source: ContentRef,
    favorite: bool,
) -> Result<(), String> {
    let ctx = context(&app, &vam_root)?;
    let sources = packages(&db, &ctx, Some(&source))?;
    tauri::async_runtime::spawn_blocking(move || {
        let _guard = CONTENT_LOCK.lock().map_err(|e| e.to_string())?;
        set_favorite(&ctx, &sources, &source, favorite)
    })
    .await
    .map_err(|e| e.to_string())?
}
#[tauri::command]
pub async fn rename_game_preset(
    app: tauri::AppHandle,
    vam_root: String,
    source: ContentRef,
    name: String,
) -> Result<ContentRef, String> {
    let ctx = context(&app, &vam_root)?;
    tauri::async_runtime::spawn_blocking(move || {
        let _guard = CONTENT_LOCK.lock().map_err(|e| e.to_string())?;
        rename_preset(&ctx, &source, name.trim())
    })
    .await
    .map_err(|e| e.to_string())?
}
#[tauri::command]
pub async fn set_game_scene_name(
    app: tauri::AppHandle,
    db: State<'_, Database>,
    vam_root: String,
    source: ContentRef,
    name: String,
) -> Result<(), String> {
    let ctx = context(&app, &vam_root)?;
    source.validate()?;
    if !is_content(&source.path, "scene") {
        return Err("只支持场景显示名称".into());
    }
    let name = name.trim();
    if name.chars().count() > 120 || name.chars().any(char::is_control) {
        return Err("显示名称无效或超过 120 字".into());
    }
    db.with_conn(|conn| {
        if name.is_empty() { conn.execute("DELETE FROM game_content_names WHERE vam_root=?1 AND resource_key=?2", rusqlite::params![ctx.root.to_string_lossy(), source.key()])?; }
        else { conn.execute("INSERT INTO game_content_names(vam_root, resource_key, name) VALUES (?1,?2,?3) ON CONFLICT(vam_root, resource_key) DO UPDATE SET name=excluded.name", rusqlite::params![ctx.root.to_string_lossy(), source.key(), name])?; }
        Ok(())
    }).map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::sync::atomic::{AtomicU64, Ordering};
    static SEQUENCE: AtomicU64 = AtomicU64::new(0);
    struct Fixture {
        ctx: Context,
    }
    impl Fixture {
        fn new() -> Self {
            let root = std::env::temp_dir().join(format!(
                "vamlibrary-content-{}-{}-{}",
                std::process::id(),
                chrono::Utc::now().timestamp_nanos_opt().unwrap(),
                SEQUENCE.fetch_add(1, Ordering::Relaxed)
            ));
            fs::create_dir_all(root.join("AddonPackages")).unwrap();
            let root = fs::canonicalize(root).unwrap();
            Self {
                ctx: Context {
                    package_roots: vec![root.join("AddonPackages")],
                    root,
                },
            }
        }
        fn put(&self, path: &str, bytes: &[u8]) {
            let target = self.ctx.root.join(path);
            fs::create_dir_all(target.parent().unwrap()).unwrap();
            fs::write(target, bytes).unwrap();
        }
        fn package(&self, id: &str, files: &[(&str, &[u8])]) -> PackageSource {
            let file = self
                .ctx
                .root
                .join("AddonPackages")
                .join(format!("{id}.var"));
            let mut zip = zip::ZipWriter::new(File::create(&file).unwrap());
            for (path, data) in files {
                zip.start_file(*path, zip::write::SimpleFileOptions::default())
                    .unwrap();
                zip.write_all(data).unwrap();
            }
            zip.finish().unwrap();
            PackageSource {
                id: id.into(),
                file,
                entries: files.iter().map(|(p, _)| p.to_string()).collect(),
            }
        }
    }
    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.ctx.root);
        }
    }
    fn local(path: &str) -> ContentRef {
        ContentRef {
            package_id: None,
            path: path.into(),
        }
    }
    const PRESET: &str = "Custom/Atom/Person/Appearance/Folder/Preset_Test.vap";

    #[test]
    fn package_panel_reads_scenes_without_contents_index_and_exports_selected_scene() {
        let f = Fixture::new();
        let scene = serde_json::to_vec(&scene_fixture()).unwrap();
        let package = f.package(
            "Author.Unindexed.1",
            &[
                ("Saves/scene/Folder/Two.json", &scene),
                ("Saves/scene/One.json", &scene),
                ("Saves/scene/One.json.fav", b""),
                ("Custom/Other.json", b"{}"),
            ],
        );
        let db = Database::new(&f.ctx.root.join("test.db")).unwrap();
        db.with_conn(|conn| {
            conn.execute("INSERT INTO packages(id,creator,name,version,file_path,scan_time) VALUES (?1,'Author','Unindexed',1,?2,'now')", rusqlite::params![package.id, package.file.to_string_lossy()])?;
            Ok(())
        }).unwrap();
        let resolved = package_by_id(&db, &f.ctx, &package.id).unwrap().unwrap();
        let list = package_scenes(&f.ctx, &resolved).unwrap();
        assert_eq!(list.items.len(), 2);
        assert!(list.warnings.is_empty());
        let source = &list.items[0].source;
        let sources = packages(&db, &f.ctx, Some(source)).unwrap();
        assert_eq!(read_content(&f.ctx, &sources, source).unwrap(), scene);
        let target = save_scene_appearance(
            &f.ctx,
            &sources,
            source,
            &content_revision(&scene),
            2,
            "FromPackagePanel",
        )
        .unwrap();
        assert!(f.ctx.root.join(target).is_file());
        let outside = Fixture::new();
        assert!(package_by_id(&db, &outside.ctx, &package.id)
            .unwrap()
            .is_none());
        assert!(package_by_id(&db, &f.ctx, "Missing.Package.1")
            .unwrap()
            .is_none());
        let missing = ContentRef {
            package_id: Some(package.id.clone()),
            path: "Saves/scene/Missing.json".into(),
        };
        assert!(read_content(
            &f.ctx,
            &packages(&db, &f.ctx, Some(&missing)).unwrap(),
            &missing
        )
        .is_err());
        let empty = f.package("Author.NoScenes.1", &[("meta.json", b"{}")]);
        assert!(package_scenes(&f.ctx, &empty).unwrap().items.is_empty());
        fs::write(&empty.file, b"not a zip").unwrap();
        assert!(package_scenes(&f.ctx, &empty).is_err());
    }

    fn scene_fixture() -> Value {
        json!({"atoms": [
            {"id":"Light", "type":"InvisibleLight", "storables":[]},
            {"id":"Alice", "type":"Person", "position":{"x":5}, "storables":[
                {"id":"geometry", "character":"Female Custom", "sceneOnly":true,
                 "clothing":[{"id":"SELF:/Custom/Clothing/top.vam", "internalId":"Author:Top", "enabled":"true"}],
                 "hair":[{"id":"BuiltInHair", "enabled":true}],
                 "morphs":[{"uid":"Other.Morphs.1:/Custom/head.vmi", "value":"1"}]},
                {"id":"skin", "Gloss":"0.5"},
                {"id":"textures", "faceDiffuseUrl":"textures/face.png"},
                {"id":"Author:TopMaterialFabric", "customTexture":"SELF:/Custom/fabric.png"},
                {"id":"Author:TopSim", "enabled":"true"},
                {"id":"BuiltInHairSim", "density":"20"},
                {"id":"rescaleObject", "scale":"0.95"},
                {"id":"hipControl", "position":{"x":5}},
                {"id":"hip", "rotation":{"x":2}},
                {"id":"PluginManager", "plugins":{"plugin#0":"Custom/Scripts/test.cs"}},
                {"id":"plugin#0_Expressions", "enabled":true},
                {"id":"Animation", "time":3},
                {"id":"Other:TopSim", "enabled":true}
            ]},
            {"id":"Bob", "type":"Person", "storables":[{"id":"geometry", "character":"Male Custom"}]},
            {"id":"Incomplete", "type":"Person", "storables":[]}
        ]})
    }

    #[test]
    fn scene_characters_keep_atom_indices_and_export_only_appearance() {
        let scene = scene_fixture();
        let people = scene_characters(&scene);
        assert_eq!(people.len(), 3);
        assert_eq!(people[0].index, 1);
        assert_eq!(people[0].name, "Alice");
        assert_eq!(
            (people[0].clothing, people[0].hair, people[0].morphs),
            (1, 1, 1)
        );
        assert!(people[2].error.is_some());
        let preset = appearance_from_atom(&scene["atoms"][1]).unwrap();
        let ids: Vec<_> = preset["storables"]
            .as_array()
            .unwrap()
            .iter()
            .map(|s| s["id"].as_str().unwrap())
            .collect();
        assert_eq!(
            ids,
            [
                "geometry",
                "skin",
                "textures",
                "Author:TopMaterialFabric",
                "Author:TopSim",
                "BuiltInHairSim",
                "rescaleObject"
            ]
        );
        assert_eq!(preset["storables"][0]["sceneOnly"], Value::Null);
        assert_eq!(preset["setUnlistedParamsToDefault"], "true");
        assert_eq!(scene_characters(&json!({"atoms":[]})).len(), 0);
        assert!(appearance_from_atom(&scene["atoms"][0]).is_err());
    }

    #[test]
    fn packaged_scene_exports_selected_character_and_preserves_asset_references() {
        let f = Fixture::new();
        let scene = serde_json::to_vec(&scene_fixture()).unwrap();
        let path = "Saves/scene/Example.json";
        let package = f.package(
            "Author.Scene.1",
            &[
                (path, &scene),
                ("Saves/scene/Example.jpg", b"default scene preview"),
                ("Saves/scene/Example.png", b"secondary preview"),
                ("Saves/scene/textures/face.png", b"image"),
                ("Custom/Clothing/top.vam", b"clothing"),
                ("Custom/fabric.png", b"image"),
            ],
        );
        let original = fs::read(&package.file).unwrap();
        let source = ContentRef {
            package_id: Some(package.id.clone()),
            path: path.into(),
        };
        let packages = [package.clone()];
        let target = save_scene_appearance(
            &f.ctx,
            &packages,
            &source,
            &content_revision(&scene),
            1,
            "Preset_Alice",
        )
        .unwrap();
        assert!(target.ends_with("/Preset_Alice.vap"));
        assert_eq!(
            fs::read(f.ctx.root.join(&target).with_extension("jpg")).unwrap(),
            b"default scene preview"
        );
        assert!(!f.ctx.root.join(&target).with_extension("png").exists());
        let output = fs::read(f.ctx.root.join(&target)).unwrap();
        let preset = parse_json(&output).unwrap();
        assert_eq!(
            preset["storables"][0]["clothing"][0]["id"],
            "Author.Scene.1:/Custom/Clothing/top.vam"
        );
        assert_eq!(
            preset["storables"][0]["morphs"][0]["uid"],
            "Other.Morphs.1:/Custom/head.vmi"
        );
        assert_eq!(
            preset["storables"][2]["faceDiffuseUrl"],
            "Author.Scene.1:/Saves/scene/textures/face.png"
        );
        assert_eq!(preset["storables"][3]["id"], "Author:TopMaterialFabric");
        assert_eq!(
            preset["storables"][3]["customTexture"],
            "Author.Scene.1:/Custom/fabric.png"
        );
        assert_eq!(fs::read(&package.file).unwrap(), original);
        assert!(save_scene_appearance(
            &f.ctx,
            &packages,
            &source,
            &content_revision(&scene),
            2,
            "Alice"
        )
        .is_err());
        assert_eq!(fs::read(f.ctx.root.join(&target)).unwrap(), output);
        let bob = save_scene_appearance(
            &f.ctx,
            &packages,
            &source,
            &content_revision(&scene),
            2,
            "Bob",
        )
        .unwrap();
        assert_eq!(
            parse_json(&fs::read(f.ctx.root.join(bob)).unwrap()).unwrap()["storables"][0]
                ["character"],
            "Male Custom"
        );
        assert_eq!(catalog(&f.ctx, &[], "appearance").unwrap().items.len(), 2);
    }

    #[test]
    fn extracted_previews_keep_format_and_absent_or_unreadable_images_do_not_leave_partial_presets()
    {
        let f = Fixture::new();
        let bytes = serde_json::to_vec(&scene_fixture()).unwrap();
        let revision = content_revision(&bytes);
        for ext in ["png", "jpeg"] {
            let source = local(&format!("Saves/scene/{ext}.json"));
            f.put(&source.path, &bytes);
            f.put(&format!("Saves/scene/{ext}.{ext}"), ext.as_bytes());
            let result = save_scene_appearance(&f.ctx, &[], &source, &revision, 2, ext).unwrap();
            let path = f.ctx.root.join(&result);
            assert_eq!(fs::read(path.with_extension(ext)).unwrap(), ext.as_bytes());
            assert!(preview(&f.ctx, &[], &local(&result)).unwrap().is_some());
        }
        let source = local("Saves/scene/NoImage.json");
        f.put(&source.path, &bytes);
        let target = save_scene_appearance(&f.ctx, &[], &source, &revision, 2, "NoImage").unwrap();
        assert!(preview(&f.ctx, &[], &local(&target)).unwrap().is_none());
        let bad = local("Saves/scene/Oversized.json");
        f.put(&bad.path, &bytes);
        File::create(f.ctx.root.join("Saves/scene/Oversized.jpg"))
            .unwrap()
            .set_len(LIMIT + 1)
            .unwrap();
        assert!(save_scene_appearance(&f.ctx, &[], &bad, &revision, 2, "BadImage").is_err());
        assert!(!f
            .ctx
            .root
            .join(format!(
                "{APPEARANCE}VAM Library/Extracted/Preset_BadImage.vap"
            ))
            .exists());
    }

    #[test]
    fn extraction_rejects_stale_selections_bad_names_and_incomplete_people_without_writes() {
        let f = Fixture::new();
        let scene = serde_json::to_vec(&scene_fixture()).unwrap();
        let source = local("Saves/scene/Example.json");
        f.put(&source.path, &scene);
        let revision = content_revision(&scene);
        for (rev, index, name) in [
            ("old", 1, "Alice"),
            (&*revision, 1, "../escape"),
            (&*revision, 0, "Light"),
            (&*revision, 3, "Incomplete"),
            (&*revision, 100, "Missing"),
        ] {
            assert!(save_scene_appearance(&f.ctx, &[], &source, rev, index, name).is_err());
        }
        assert!(!f.ctx.root.join(APPEARANCE).exists());
        f.put(
            "Custom/Atom/Person/Appearance/VAM Library/Extracted/Preset_Alice.png",
            b"existing preview",
        );
        assert!(
            save_scene_appearance(&f.ctx, &[], &source, &revision, 2, "Alice")
                .unwrap_err()
                .contains("已存在")
        );
        f.put(
            "Custom/Atom/Person/Appearance/VAM Library/Extracted/Preset_Hidden.vap.hide",
            b"",
        );
        assert!(
            save_scene_appearance(&f.ctx, &[], &source, &revision, 2, "Hidden")
                .unwrap_err()
                .contains("已存在")
        );
        assert!(
            save_scene_appearance(&f.ctx, &[], &source, &revision, 1, "Unresolved")
                .unwrap_err()
                .contains("SELF")
        );
        assert_eq!(fs::read(f.ctx.root.join(&source.path)).unwrap(), scene);
    }

    #[test]
    fn local_scene_assets_are_rebased_without_changing_names_or_external_packages() {
        let f = Fixture::new();
        let mut scene = scene_fixture();
        scene["atoms"][1]["storables"][0]["clothing"][0]["id"] =
            json!("../../../Custom/Clothing/top.vam");
        scene["atoms"][1]["storables"][2]["faceDiffuseUrl"] = json!("../textures/face.png");
        scene["atoms"][1]["storables"][3]["customTexture"] = json!("Custom/fabric.png");
        let bytes = serde_json::to_vec(&scene).unwrap();
        let source = local("Saves/scene/Folder/Example.json");
        f.put(&source.path, &bytes);
        let path =
            save_scene_appearance(&f.ctx, &[], &source, &content_revision(&bytes), 1, "Local")
                .unwrap();
        let preset = parse_json(&fs::read(f.ctx.root.join(path)).unwrap()).unwrap();
        assert_eq!(
            preset["storables"][0]["clothing"][0]["id"],
            "Custom/Clothing/top.vam"
        );
        assert_eq!(
            preset["storables"][2]["faceDiffuseUrl"],
            "Saves/scene/textures/face.png"
        );
        assert_eq!(preset["storables"][3]["customTexture"], "Custom/fabric.png");
        assert_eq!(preset["storables"][3]["id"], "Author:TopMaterialFabric");
        assert!(rebase_local_refs(
            &mut json!("../../../../../escape.png"),
            &f.ctx.root,
            "Saves/scene"
        )
        .is_err());
    }

    #[test]
    fn all_eight_preset_types_list_copy_and_rename_in_their_own_directories() {
        let f = Fixture::new();
        let paths: Vec<String> = PRESET_TYPES
            .iter()
            .map(|(_, prefix)| format!("{prefix}Folder/Preset_Test.vap"))
            .collect();
        let mut files: Vec<(&str, &[u8])> = paths.iter().map(|path| (path.as_str(), b"{\"storables\":[{\"id\":\"test\",\"resource\":\"SELF:/Custom/Shared/asset.vam\"}]}".as_slice())).collect();
        files.push((
            "Custom/Atom/Person/General/Preset_Other.vap",
            b"{\"storables\":[]}",
        ));
        let package = f.package("Author.Mixed.1", &files);
        for (kind, prefix) in PRESET_TYPES {
            let path = format!("{prefix}Folder/Preset_Test.vap");
            f.put(&format!("{prefix}Preset_Local.vap"), b"{\"storables\":[]}");
            let list = catalog(&f.ctx, &[package.clone()], kind).unwrap();
            assert_eq!(list.directory, prefix.trim_end_matches('/'));
            assert_eq!(list.items.len(), 2, "{kind}");
            assert!(list.items.iter().all(|i| is_content(&i.source.path, kind)));
            assert!(ContentRef {
                package_id: Some(package.id.clone()),
                path: path.clone()
            }
            .validate()
            .is_ok());
            let report = copy_all(&f.ctx, &[package.clone()], None, kind);
            assert_eq!(report.copied, 1, "{kind}: {:?}", report.errors);
            assert!(report.errors.is_empty());
            let destination = format!("{prefix}VAM Library/Author.Mixed.1/Folder/Preset_Test.vap");
            let json = parse_json(&fs::read(f.ctx.root.join(&destination)).unwrap()).unwrap();
            assert_eq!(
                json["storables"][0]["resource"],
                "Author.Mixed.1:/Custom/Shared/asset.vam"
            );
            assert_eq!(copy_all(&f.ctx, &[package.clone()], None, kind).skipped, 1);
            let renamed = rename_preset(&f.ctx, &local(&destination), "自定义预设").unwrap();
            assert!(renamed.path.starts_with(prefix));
            assert!(f.ctx.root.join(renamed.path).is_file());
        }
        assert!(catalog(&f.ctx, &[package.clone()], "unknown").is_err());
        assert_eq!(
            copy_all(&f.ctx, &[package], None, "unknown").errors.len(),
            1
        );
        assert!(!f
            .ctx
            .root
            .join("Custom/Atom/Person/General/VAM Library")
            .exists());
    }

    #[test]
    fn preset_types_do_not_include_raw_assets_or_neighboring_folders() {
        for path in [
            "Custom/Clothing/Female/Item.vam",
            "Custom/Hair/Female/Item.vam",
            "Custom/Atom/Person/Morphs/female/Morph.vmi",
            "Custom/Atom/Person/Morphs/female/Morph.vmb",
            "Custom/Scripts/Plugin.cs",
            "Custom/Atom/Person/AppearanceExtra/Preset.vap",
            "Custom/Atom/Person/General/Preset.vap",
            "Custom/Atom/Person/Animation/Preset.vap",
        ] {
            assert!(preset_kind(path).is_none(), "{path}");
        }
        assert_eq!(
            preset_kind("custom/atom/person/ANIMATIONPRESETS/Test.VAP"),
            Some("animation")
        );
        assert_eq!(
            preset_kind("Custom/Atom/Person/Morphs/Folder/Preset.vap"),
            Some("morphs")
        );
    }

    #[test]
    fn copy_preserves_dependencies_thumbnails_and_original_archive() {
        let f = Fixture::new();
        let data = serde_json::to_vec(&json!({"storables":[{"id":"geometry", "character":"Female Custom", "texture":"SELF:/Custom/Textures/skin.png", "other":"Other.Package.1:/Custom/test.vam", "relative":"skin.jpg", "absolute":"Custom/Textures/skin.png"}]})).unwrap();
        let package = f.package(
            "Author.Look.1",
            &[
                (PRESET, &data),
                (
                    "Custom/Atom/Person/Appearance/Folder/Preset_Test.jpg",
                    b"preview",
                ),
                ("Custom/Textures/skin.png", b"skin"),
                ("Custom/Atom/Person/Appearance/Folder/skin.jpg", b"relative"),
            ],
        );
        let original = fs::read(&package.file).unwrap();
        let result = copy_all(&f.ctx, &[package.clone()], None, "appearance");
        assert_eq!(result.copied, 1, "{:?}", result.errors);
        let path = f.ctx.root.join(format!(
            "{APPEARANCE}VAM Library/Author.Look.1/Folder/Preset_Test.vap"
        ));
        let value = parse_json(&fs::read(&path).unwrap()).unwrap();
        let geometry = &value["storables"][0];
        assert_eq!(geometry["character"], "Female Custom");
        assert_eq!(
            geometry["texture"],
            "Author.Look.1:/Custom/Textures/skin.png"
        );
        assert_eq!(
            geometry["absolute"],
            "Author.Look.1:/Custom/Textures/skin.png"
        );
        assert_eq!(
            geometry["relative"],
            "Author.Look.1:/Custom/Atom/Person/Appearance/Folder/skin.jpg"
        );
        assert_eq!(geometry["other"], "Other.Package.1:/Custom/test.vam");
        assert_eq!(fs::read(path.with_extension("jpg")).unwrap(), b"preview");
        assert_eq!(
            copy_all(&f.ctx, &[package.clone()], None, "appearance").skipped,
            1
        );
        fs::write(&path, b"user changed this").unwrap();
        let conflict = copy_all(&f.ctx, &[package.clone()], None, "appearance");
        assert_eq!(conflict.errors.len(), 1);
        assert_eq!(fs::read(path).unwrap(), b"user changed this");
        assert_eq!(fs::read(package.file).unwrap(), original);
    }
    #[test]
    fn bad_presets_do_not_block_other_packages_or_create_partial_files() {
        let f = Fixture::new();
        let broken = f.package("Bad.Look.1", &[(PRESET, b"broken")]);
        let good = f.package("Good.Look.1", &[(PRESET, b"{\"storables\":[]}")]);
        let result = copy_all(&f.ctx, &[broken, good], None, "appearance");
        assert_eq!(result.copied, 1);
        assert_eq!(result.errors.len(), 1);
        assert!(!f
            .ctx
            .root
            .join(format!(
                "{APPEARANCE}VAM Library/Bad.Look.1/Folder/Preset_Test.vap"
            ))
            .exists());
    }
    #[test]
    fn thumbnail_conflict_leaves_preset_unwritten() {
        let f = Fixture::new();
        let package = f.package(
            "Author.Look.1",
            &[
                (PRESET, b"{\"storables\":[]}"),
                (
                    "Custom/Atom/Person/Appearance/Folder/Preset_Test.jpg",
                    b"new",
                ),
            ],
        );
        let target = format!("{APPEARANCE}VAM Library/Author.Look.1/Folder/Preset_Test");
        f.put(&format!("{target}.jpg"), b"user image");
        assert_eq!(
            copy_all(&f.ctx, &[package], None, "appearance")
                .errors
                .len(),
            1
        );
        assert!(!f.ctx.root.join(format!("{target}.vap")).exists());
        assert_eq!(
            fs::read(f.ctx.root.join(format!("{target}.jpg"))).unwrap(),
            b"user image"
        );
    }
    #[test]
    fn favorites_round_trip_and_orphan_removal_keep_scenes() {
        let f = Fixture::new();
        let path = "Saves/scene/Folder/Scene.json";
        f.put(path, b"{\"atoms\":[]}");
        let package = f.package("Author.Scene.1", &[(path, b"{\"atoms\":[]}")]);
        let packaged = ContentRef {
            package_id: Some(package.id.clone()),
            path: path.into(),
        };
        for source in [local(path), packaged.clone()] {
            set_favorite(&f.ctx, &[package.clone()], &source, true).unwrap();
            set_favorite(&f.ctx, &[package.clone()], &source, true).unwrap();
            assert!(favorite_path(&f.ctx.root, &source).unwrap().is_file());
        }
        let list = catalog(&f.ctx, &[package.clone()], "scene").unwrap();
        assert_eq!(list.items.iter().filter(|i| i.favorite).count(), 2);
        let orphan = catalog(&f.ctx, &[], "scene").unwrap();
        assert!(orphan
            .items
            .iter()
            .any(|i| i.source.package_id.is_some() && i.favorite && !i.available));
        set_favorite(&f.ctx, &[], &packaged, false).unwrap();
        set_favorite(&f.ctx, &[], &packaged, false).unwrap();
        set_favorite(&f.ctx, &[], &local(path), false).unwrap();
        assert_eq!(fs::read(f.ctx.root.join(path)).unwrap(), b"{\"atoms\":[]}");
        assert!(package.file.is_file());
        assert!(set_favorite(&f.ctx, &[], &local("Saves/scene/missing.json"), true).is_err());
        let large = f.ctx.root.join("Saves/scene/Large.json");
        File::create(&large).unwrap().set_len(LIMIT + 1).unwrap();
        set_favorite(&f.ctx, &[], &local("Saves/scene/Large.json"), true).unwrap();
        assert!(large.with_extension("json.fav").is_file());
    }
    #[test]
    fn rename_moves_preset_preview_and_flags_without_overwriting() {
        let f = Fixture::new();
        f.put(PRESET, b"{\"storables\":[]}");
        for ext in ["jpg", "png", "vap.fav", "vap.hide"] {
            f.put(&PRESET.replace(".vap", &format!(".{ext}")), ext.as_bytes());
        }
        let renamed = rename_preset(&f.ctx, &local(PRESET), "新角色").unwrap();
        assert!(renamed.path.ends_with("Preset_新角色.vap"));
        assert!(!f.ctx.root.join(PRESET).exists());
        for ext in ["jpg", "png", "vap.fav", "vap.hide"] {
            assert_eq!(
                fs::read(f.ctx.root.join(&renamed.path).with_extension(ext)).unwrap(),
                ext.as_bytes()
            );
        }
        f.put(&PRESET.replace("Preset_Test", "Preset_Taken"), b"keep");
        assert!(rename_preset(&f.ctx, &renamed, "Taken").is_err());
        assert!(f.ctx.root.join(&renamed.path).is_file());
        for bad in ["../escape", "CON", "name.", "x:y", "LPT1", "", "a/b"] {
            assert!(rename_preset(&f.ctx, &renamed, bad).is_err());
        }
    }

    #[test]
    fn preset_names_display_without_prefix_and_rename_with_one_prefix() {
        let f = Fixture::new();
        f.put(PRESET, b"{\"storables\":[]}");
        assert_eq!(item(&f.ctx.root, local(PRESET), true).unwrap().name, "Test");
        assert_eq!(
            item(&f.ctx.root, local("Saves/scene/Preset_Scene.json"), true)
                .unwrap()
                .name,
            "Preset_Scene"
        );
        for input in ["xxx", "Preset_xxx", "Preset_Preset_xxx", " xxx "] {
            assert_eq!(preset_filename(input).unwrap(), "Preset_xxx.vap");
        }
        for input in [
            "",
            " ",
            "Preset_",
            "Preset_Preset_",
            "../bad",
            "Preset_../bad",
        ] {
            assert!(preset_filename(input).is_err(), "{input}");
        }
        let renamed = rename_preset(&f.ctx, &local(PRESET), "xxx").unwrap();
        assert!(renamed.path.ends_with("/Preset_xxx.vap"));
        assert_eq!(
            item(&f.ctx.root, renamed.clone(), true).unwrap().name,
            "xxx"
        );
        assert_eq!(
            rename_preset(&f.ctx, &renamed, "Preset_xxx").unwrap().path,
            renamed.path
        );
    }
    #[test]
    fn rejects_traversal_absolute_paths_and_oversized_reads() {
        let f = Fixture::new();
        for path in [
            "../escape",
            "Saves/scene/../../other.json",
            "C:/other.json",
            "/other.json",
            "Saves\\scene\\x.json",
        ] {
            assert!(safe_path(&f.ctx.root, path).is_err());
        }
        let source = ContentRef {
            package_id: Some("../outside".into()),
            path: "Saves/scene/a.json".into(),
        };
        assert!(favorite_path(&f.ctx.root, &source).is_err());
        assert!(bounded_read(&b"small"[..], LIMIT + 1).is_err());
        assert!(parse_json(b"[]").is_err());
    }
    #[test]
    fn database_sources_are_scoped_to_the_current_instance() {
        let first = Fixture::new();
        let second = Fixture::new();
        let a = first.package("A.Look.1", &[(PRESET, b"{\"storables\":[]}")]);
        let b = second.package("B.Look.1", &[(PRESET, b"{\"storables\":[]}")]);
        let db = Database::new(&first.ctx.root.join("test.db")).unwrap();
        db.with_conn(|conn| {
            for p in [&a, &b] {
                conn.execute("INSERT INTO packages(id,creator,name,version,file_path,scan_time) VALUES (?1,'a','b',1,?2,'now')", rusqlite::params![p.id, p.file.to_string_lossy()])?;
                conn.execute("INSERT INTO contents(package_id,file_path) VALUES (?1,?2)", rusqlite::params![p.id, PRESET])?;
            }
            Ok(())
        }).unwrap();
        let result = packages(&db, &first.ctx, None).unwrap();
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].id, a.id);
        assert!(packages(
            &db,
            &first.ctx,
            Some(&ContentRef {
                package_id: Some(b.id),
                path: PRESET.into()
            })
        )
        .unwrap()
        .is_empty());
    }
    #[cfg(windows)]
    #[test]
    fn rejects_directory_junctions() {
        use std::os::windows::process::CommandExt;
        let f = Fixture::new();
        let outside = Fixture::new();
        let link = f.ctx.root.join("Saves");
        let result = std::process::Command::new("cmd.exe")
            .args(["/c", "mklink", "/J"])
            .arg(&link)
            .arg(&outside.ctx.root)
            .creation_flags(0x08000000)
            .output()
            .unwrap();
        assert!(
            result.status.success(),
            "{}",
            String::from_utf8_lossy(&result.stderr)
        );
        assert!(safe_path(&f.ctx.root, "Saves/scene/test.json.fav").is_err());
        fs::remove_dir(&link).unwrap();
    }
    #[test]
    #[ignore = "reads local game only; set VAM_CONTENT_TEST_ROOT"]
    fn local_game_read_only_compatibility() {
        let root = fs::canonicalize(std::env::var("VAM_CONTENT_TEST_ROOT").unwrap()).unwrap();
        let ctx = Context {
            package_roots: vec![root.join("AddonPackages")],
            root,
        };
        let appearances = catalog(&ctx, &[], "appearance").unwrap();
        assert!(
            appearances.warnings.is_empty(),
            "{:?}",
            appearances.warnings
        );
        assert!(!appearances.items.is_empty());
        let source = &appearances
            .items
            .iter()
            .find(|i| i.available)
            .unwrap()
            .source;
        let data = parse_json(&read_content(&ctx, &[], source).unwrap()).unwrap();
        assert!(data["storables"].is_array());
        assert!(preview(&ctx, &[], source).unwrap().is_some());
        let scenes = catalog(&ctx, &[], "scene").unwrap();
        assert!(scenes.items.iter().any(|i| i.favorite));
        eprintln!(
            "Read-only game check: {} local presets, {} scene entries, {} favorites",
            appearances.items.len(),
            scenes.items.len(),
            scenes.items.iter().filter(|i| i.favorite).count()
        );
    }

    #[test]
    #[ignore = "reads real scenes; extracts only into an isolated temporary fixture"]
    fn real_scene_extraction_compatibility() {
        let game = fs::canonicalize(std::env::var("VAM_CONTENT_TEST_ROOT").unwrap()).unwrap();
        let conn = rusqlite::Connection::open_with_flags(
            std::env::var("VAM_CONTENT_TEST_DB").unwrap(),
            rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY,
        )
        .unwrap();
        let mut stmt = conn.prepare("SELECT p.id,p.file_path,c.file_path FROM packages p JOIN contents c ON p.id=c.package_id WHERE lower(c.file_path) LIKE 'saves/scene/%.json' ORDER BY p.id,c.file_path LIMIT 50").unwrap();
        let rows = stmt
            .query_map([], |r| {
                Ok((
                    r.get::<_, String>(0)?,
                    r.get::<_, String>(1)?,
                    r.get::<_, String>(2)?,
                ))
            })
            .unwrap();
        let f = Fixture::new();
        let mut verified = 0;
        for row in rows {
            let (id, file, path) = row.unwrap();
            if !fs::canonicalize(&file)
                .ok()
                .is_some_and(|p| p.starts_with(&game))
            {
                continue;
            }
            let package = PackageSource {
                id: id.clone(),
                file: PathBuf::from(file),
                entries: vec![path.clone()],
            };
            let before = fs::metadata(&package.file).unwrap();
            let source = ContentRef {
                package_id: Some(id),
                path,
            };
            let sources = [package];
            let Ok(bytes) = read_content(&f.ctx, &sources, &source) else {
                continue;
            };
            let json = parse_json(&bytes).unwrap();
            let Some(person) = scene_characters(&json)
                .into_iter()
                .find(|p| p.error.is_none())
            else {
                continue;
            };
            let target = save_scene_appearance(
                &f.ctx,
                &sources,
                &source,
                &content_revision(&bytes),
                person.index,
                &format!("RealScene{verified}"),
            )
            .unwrap();
            let preset = parse_json(&fs::read(f.ctx.root.join(target)).unwrap()).unwrap();
            assert_eq!(preset["setUnlistedParamsToDefault"], "true");
            assert!(preset["storables"].as_array().unwrap().iter().all(|s| {
                let id = s["id"].as_str().unwrap();
                id != "PluginManager" && !id.starts_with("plugin#") && id != "hipControl"
            }));
            assert_eq!(
                preset["storables"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .find(|s| s["id"] == "geometry")
                    .unwrap()["character"],
                person.character
            );
            assert_eq!(before.len(), fs::metadata(&sources[0].file).unwrap().len());
            assert_eq!(
                before.modified().unwrap(),
                fs::metadata(&sources[0].file).unwrap().modified().unwrap()
            );
            verified += 1;
            if verified == 3 {
                break;
            }
        }
        assert_eq!(
            verified, 3,
            "expected three real scenes with directly saved people"
        );
        eprintln!("Extracted 3 real VAR scene characters into temporary fixtures; original archives unchanged.");
    }

    #[test]
    #[ignore = "reads a real VAR; copies presets only into an isolated temporary fixture"]
    fn local_var_copy_compatibility() {
        let db_path = std::env::var("VAM_CONTENT_TEST_DB").unwrap();
        let game = fs::canonicalize(std::env::var("VAM_CONTENT_TEST_ROOT").unwrap()).unwrap();
        let conn = rusqlite::Connection::open_with_flags(
            db_path,
            rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY,
        )
        .unwrap();
        let mut stmt = conn.prepare("SELECT p.id,p.file_path,c.file_path FROM packages p JOIN contents c ON p.id=c.package_id WHERE lower(c.file_path) LIKE 'custom/atom/person/appearance/%.vap'").unwrap();
        let rows = stmt
            .query_map([], |r| {
                Ok((
                    r.get::<_, String>(0)?,
                    r.get::<_, String>(1)?,
                    r.get::<_, String>(2)?,
                ))
            })
            .unwrap();
        let (id, file, path) = rows
            .map(Result::unwrap)
            .find(|(_, file, _)| {
                fs::canonicalize(file)
                    .ok()
                    .is_some_and(|p| p.starts_with(&game))
            })
            .expect("an indexed preset in the test game");
        let source = PackageSource {
            id,
            file: PathBuf::from(file),
            entries: vec![path],
        };
        let before = fs::metadata(&source.file).unwrap();
        let f = Fixture::new();
        let result = copy_all(&f.ctx, &[source.clone()], None, "appearance");
        assert_eq!(result.copied, 1, "{:?}", result.errors);
        assert!(result.errors.is_empty());
        let local = catalog(&f.ctx, &[], "appearance").unwrap();
        assert_eq!(local.items.len(), 1);
        let json = parse_json(&read_content(&f.ctx, &[], &local.items[0].source).unwrap()).unwrap();
        assert!(json["storables"].as_array().unwrap().len() > 0);
        assert!(preview(&f.ctx, &[], &local.items[0].source)
            .unwrap()
            .is_some());
        assert_eq!(
            copy_all(&f.ctx, &[source.clone()], None, "appearance").skipped,
            1
        );
        let after = fs::metadata(source.file).unwrap();
        assert_eq!(before.len(), after.len());
        assert_eq!(before.modified().unwrap(), after.modified().unwrap());
        eprintln!(
            "Actual VAR preset and preview copied into temporary fixture; original VAR unchanged"
        );
    }
}
