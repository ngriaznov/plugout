//! Wire types shared with the frontend. Serde's camelCase renaming keeps these
//! structs mirror-images of the TypeScript interfaces in `src/types.ts`.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Format {
    #[serde(rename = "AU")]
    Au,
    #[serde(rename = "VST3")]
    Vst3,
    #[serde(rename = "VST2")]
    Vst2,
    #[serde(rename = "CLAP")]
    Clap,
    #[serde(rename = "AAX")]
    Aax,
}

impl Format {
    /// The bundle extension this format uses on disk.
    pub fn extension(self) -> &'static str {
        match self {
            Format::Au => "component",
            Format::Vst3 => "vst3",
            Format::Vst2 => "vst",
            Format::Clap => "clap",
            Format::Aax => "aaxplugin",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Scope {
    User,
    System,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PluginBundle {
    pub id: String,
    pub name: String,
    pub vendor: String,
    pub version: String,
    pub format: Format,
    pub bundle_id: String,
    pub path: String,
    pub size_bytes: u64,
    pub scope: Scope,
    pub package_id: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum RemovalStatus {
    Trashed,
    Failed,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RemovalResult {
    pub id: String,
    pub path: String,
    pub status: RemovalStatus,
    pub message: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PluginDetails {
    pub files_to_trash: Vec<String>,
    pub package_id: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn format_serializes_to_expected_strings() {
        assert_eq!(serde_json::to_string(&Format::Au).unwrap(), "\"AU\"");
        assert_eq!(serde_json::to_string(&Format::Vst3).unwrap(), "\"VST3\"");
        assert_eq!(serde_json::to_string(&Scope::System).unwrap(), "\"system\"");
    }

    #[test]
    fn plugin_bundle_uses_camel_case() {
        let b = PluginBundle {
            id: "/p".into(),
            name: "N".into(),
            vendor: "V".into(),
            version: "1".into(),
            format: Format::Vst3,
            bundle_id: "com.v.n".into(),
            path: "/p".into(),
            size_bytes: 10,
            scope: Scope::User,
            package_id: None,
        };
        let j = serde_json::to_string(&b).unwrap();
        assert!(j.contains("\"sizeBytes\":10"), "got {j}");
        assert!(j.contains("\"packageId\":null"), "got {j}");
    }
}
