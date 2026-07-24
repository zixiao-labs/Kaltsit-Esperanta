//! PerllicaScript extension bridge for the Zed editor.
//!
//! This crate provides the bridge between PerllicaScript extensions and
//! Zed's extension system. It compiles .pscript files to bytecode and
//! runs them in the PerllicaScript VM, exposing the Zed API to scripts.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use anyhow::{Context, Result};
use extension::{Extension, ExtensionManifest};
use gpui::AsyncApp;

mod api;
mod loader;
mod runtime;

pub use api::PerlicascriptApi;
pub use loader::PerlicascriptExtensionLoader;
pub use runtime::PerlicascriptRuntime;

/// A PerllicaScript extension instance.
pub struct PerlicascriptExtension {
    manifest: ExtensionManifest,
    work_dir: PathBuf,
    runtime: PerlicascriptRuntime,
}

impl PerlicascriptExtension {
    /// Load a PerllicaScript extension from a directory.
    pub async fn load(extension_dir: &Path, cx: &AsyncApp) -> Result<Self> {
        let manifest = loader::load_manifest(extension_dir)
            .context("Failed to load extension manifest")?;

        let runtime = PerlicascriptRuntime::new()
            .context("Failed to create runtime")?;

        let mut ext = Self {
            manifest: ExtensionManifest {
                id: manifest.id.clone(),
                name: manifest.name.clone(),
                version: manifest.version.clone(),
                schema_version: manifest.schema_version,
                lib: extension::LibManifestEntry {
                    kind: extension::LibKind::PerlicaScript,
                    version: manifest.version.clone(),
                },
                ..Default::default()
            },
            work_dir: extension_dir.to_path_buf(),
            runtime,
        };

        // Load and compile the main script
        let script_path = extension_dir.join("extension.pscript");
        if script_path.exists() {
            ext.load_script(&script_path, cx).await?;
        }

        Ok(ext)
    }

    /// Load and compile a PerllicaScript file.
    async fn load_script(&mut self, path: &Path, cx: &AsyncApp) -> Result<()> {
        let source = std::fs::read_to_string(path)
            .with_context(|| format!("Failed to read {}", path.display()))?;

        let compiler = perlica_compiler::Compiler::new(&source);
        let compiled = compiler.compile()
            .context("Failed to compile PerllicaScript")?;

        self.runtime.load_module(compiled.bytecode, compiled.type_system)?;
        Ok(())
    }

    /// Run an exported function from the extension.
    pub async fn call_function(
        &mut self,
        name: &str,
        args: Vec<perlica_runtime::value::Value>,
    ) -> Result<perlica_runtime::value::Value> {
        self.runtime.call_function(name, args)
    }
}

/// Register PerllicaScript as an extension type with the extension store.
pub fn register_extension_type() {
    // Register the .pscript file extension
    // Register PerllicaScript as a valid extension kind
}

/// The API surface exposed to PerllicaScript extensions.
///
/// This provides access to Zed's editor features from PerllicaScript.
pub struct PerlicascriptHost {
    extensions: HashMap<String, PerlicascriptExtension>,
}

impl PerlicascriptHost {
    pub fn new() -> Self {
        Self {
            extensions: HashMap::new(),
        }
    }

    /// Load all PerllicaScript extensions from a directory.
    pub async fn load_extensions(
        &mut self,
        extensions_dir: &Path,
        cx: &AsyncApp,
    ) -> Result<()> {
        let entries = std::fs::read_dir(extensions_dir)
            .context("Failed to read extensions directory")?;

        for entry in entries.flatten() {
            let path = entry.path();
            if path.join("extension.pscript").exists() {
                match PerlicascriptExtension::load(&path, cx).await {
                    Ok(ext) => {
                        log::info!("Loaded PerllicaScript extension: {}", ext.manifest.id);
                        self.extensions.insert(ext.manifest.id.clone(), ext);
                    }
                    Err(e) => {
                        log::error!("Failed to load extension from {}: {}", path.display(), e);
                    }
                }
            }
        }

        Ok(())
    }

    /// Get an extension by ID.
    pub fn get_extension(&self, id: &str) -> Option<&PerlicascriptExtension> {
        self.extensions.get(id)
    }

    /// Get a mutable reference to an extension by ID.
    pub fn get_extension_mut(&mut self, id: &str) -> Option<&mut PerlicascriptExtension> {
        self.extensions.get_mut(id)
    }
}

impl Default for PerlicascriptHost {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_load_manifest() {
        // TODO: implement manifest loading tests
    }
}
