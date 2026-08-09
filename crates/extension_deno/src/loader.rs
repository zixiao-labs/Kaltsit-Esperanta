//! Module loading policy for embedded JS extensions.

use anyhow::{Result, bail};
use std::path::{Path, PathBuf};

#[derive(Clone, Debug)]
pub struct ModuleSpec {
    pub path: PathBuf,
    pub source: String,
}

/// Reject remote / absolute `file:` imports — only relative extension-dir modules.
pub fn validate_import_specifier(specifier: &str) -> Result<()> {
    let lower = specifier.to_ascii_lowercase();
    if lower.starts_with("http://")
        || lower.starts_with("https://")
        || lower.starts_with("file:")
        || lower.starts_with("data:")
        || lower.starts_with("node:")
        || lower.starts_with("npm:")
        || lower.starts_with("jsr:")
    {
        bail!("remote or absolute import is forbidden: {specifier}");
    }
    if Path::new(specifier).is_absolute() {
        bail!("absolute path import is forbidden: {specifier}");
    }
    Ok(())
}

pub fn resolve_local_module(extension_root: &Path, specifier: &str) -> Result<ModuleSpec> {
    validate_import_specifier(specifier)?;
    let path = extension_root.join(specifier.trim_start_matches("./"));
    let canonical = path.canonicalize().unwrap_or(path.clone());
    let root = extension_root
        .canonicalize()
        .unwrap_or_else(|_| extension_root.to_path_buf());
    if !canonical.starts_with(&root) {
        bail!(
            "module escape blocked: {} is outside {}",
            canonical.display(),
            root.display()
        );
    }
    let source = std::fs::read_to_string(&canonical)
        .map_err(|error| anyhow::anyhow!("failed to read {}: {error}", canonical.display()))?;
    Ok(ModuleSpec {
        path: canonical,
        source,
    })
}
