use serde::{Deserialize, Serialize};

const RESOURCE_TYPE_ORDER: &[&str] = &[
    "scene",
    "appearance",
    "clothing",
    "hair",
    "morph",
    "plugin",
    "asset",
    "texture",
    "sound",
    "other",
];

/// Resource types found in VAM
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "lowercase")]
#[allow(dead_code)]
pub enum ResourceType {
    Scene,
    Appearance,
    Morph,
    Clothing,
    Hair,
    Texture,
    Plugin,
    Asset,
    Sound,
    Other,
}

#[allow(dead_code)]
impl ResourceType {
    /// Infer resource type from a file path inside a .var
    pub fn from_path(path: &str) -> Self {
        let path_lower = path.to_lowercase();

        if path_lower.contains("saves/scene") && path_lower.ends_with(".json") {
            ResourceType::Scene
        } else if path_lower.contains("appearance")
            && (path_lower.ends_with(".vap") || path_lower.ends_with(".json"))
        {
            ResourceType::Appearance
        } else if path_lower.ends_with(".vmi")
            || path_lower.ends_with(".vmb")
            || path_lower.contains("morphs")
        {
            ResourceType::Morph
        } else if path_lower.contains("cloth") {
            ResourceType::Clothing
        } else if path_lower.contains("hair") {
            ResourceType::Hair
        } else if path_lower.ends_with(".jpg")
            || path_lower.ends_with(".png")
            || path_lower.ends_with(".tif")
            || path_lower.contains("texture")
        {
            ResourceType::Texture
        } else if path_lower.ends_with(".cs")
            || path_lower.ends_with(".cslist")
            || path_lower.ends_with(".dll")
            || path_lower.contains("scripts")
        {
            ResourceType::Plugin
        } else if path_lower.ends_with(".assetbundle") {
            ResourceType::Asset
        } else if path_lower.ends_with(".wav")
            || path_lower.ends_with(".mp3")
            || path_lower.ends_with(".ogg")
            || path_lower.contains("sounds")
        {
            ResourceType::Sound
        } else {
            ResourceType::Other
        }
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            ResourceType::Scene => "scene",
            ResourceType::Appearance => "appearance",
            ResourceType::Morph => "morph",
            ResourceType::Clothing => "clothing",
            ResourceType::Hair => "hair",
            ResourceType::Texture => "texture",
            ResourceType::Plugin => "plugin",
            ResourceType::Asset => "asset",
            ResourceType::Sound => "sound",
            ResourceType::Other => "other",
        }
    }
}

impl std::fmt::Display for ResourceType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

pub fn normalize_resource_types(types: impl IntoIterator<Item = String>) -> Vec<String> {
    let mut unique = Vec::new();
    for resource_type in types {
        if !resource_type.is_empty() && !unique.contains(&resource_type) {
            unique.push(resource_type);
        }
    }

    let mut ordered = Vec::new();
    for known_type in RESOURCE_TYPE_ORDER {
        if unique
            .iter()
            .any(|resource_type| resource_type == known_type)
        {
            ordered.push((*known_type).to_string());
        }
    }

    let mut unknowns: Vec<String> = unique
        .into_iter()
        .filter(|resource_type| !RESOURCE_TYPE_ORDER.contains(&resource_type.as_str()))
        .collect();
    unknowns.sort();
    ordered.extend(unknowns);

    ordered
}

pub fn primary_resource_type(types: impl IntoIterator<Item = String>) -> String {
    normalize_resource_types(types)
        .into_iter()
        .next()
        .unwrap_or_else(|| "other".to_string())
}
