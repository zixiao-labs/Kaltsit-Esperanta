use anyhow::{Context, Result};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone)]
pub struct ExtensionManifest {
    pub id: String,
    pub name: String,
    pub version: String,
    pub schema_version: u32,
    pub description: Option<String>,
    pub authors: Vec<String>,
    pub repository: Option<String>,
}

/// Load an extension.toml manifest from a directory.
pub fn load_manifest(extension_dir: &Path) -> Result<ExtensionManifest> {
    let toml_path = extension_dir.join("extension.toml");
    let content = std::fs::read_to_string(&toml_path)
        .with_context(|| format!("Failed to read {}", toml_path.display()))?;

    let manifest: toml::Value = content.parse().context("Failed to parse extension.toml")?;

    let id = manifest
        .get("id")
        .and_then(|v| v.as_str())
        .context("Missing 'id' in extension.toml")?
        .to_string();

    let name = manifest
        .get("name")
        .and_then(|v| v.as_str())
        .unwrap_or(&id)
        .to_string();

    let version = manifest
        .get("version")
        .and_then(|v| v.as_str())
        .unwrap_or("0.0.0")
        .to_string();

    let schema_version = manifest
        .get("schema_version")
        .and_then(|v| v.as_integer())
        .unwrap_or(1) as u32;

    let description = manifest
        .get("description")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());

    let authors = manifest
        .get("authors")
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_str().map(|s| s.to_string()))
                .collect()
        })
        .unwrap_or_default();

    let repository = manifest
        .get("repository")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());

    Ok(ExtensionManifest {
        id,
        name,
        version,
        schema_version,
        description,
        authors,
        repository,
    })
}
