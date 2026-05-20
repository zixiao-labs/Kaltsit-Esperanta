//! Persistent configuration for the Wuling DevOps integration.
//!
//! The Zed `settings_content` crate is upstream code we don't want to fork:
//! adding a `wuling` field there would create rebase pain forever. So we
//! sidestep it and own our own `wuling.json` under `paths::config_dir()`.
//! Loaded on init, written when the user changes the server URL.

use std::path::PathBuf;

use ama10::server_url::ServerUrl;
use anyhow::{Context as _, Result};
use serde::{Deserialize, Serialize};

const CONFIG_FILE: &str = "wuling.json";

/// JSON shape on disk. New optional fields can be added without breaking
/// older Esperantas thanks to `#[serde(default)]`.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
struct FileShape {
    #[serde(default)]
    server_url: Option<String>,
}

/// Reify the on-disk config into a strongly-typed struct.
#[derive(Debug, Clone)]
pub struct WulingConfig {
    pub server: ServerUrl,
}

impl Default for WulingConfig {
    fn default() -> Self {
        Self {
            server: ServerUrl::default_saas(),
        }
    }
}

impl WulingConfig {
    fn path() -> PathBuf {
        paths::config_dir().join(CONFIG_FILE)
    }

    /// Read the config from disk. If the file is missing or malformed, the
    /// caller gets the default (SaaS) config and a `log::warn!` is emitted —
    /// the user can recover by editing settings or signing in afresh, so
    /// we don't propagate the error.
    pub fn load() -> Self {
        let path = Self::path();
        let raw = match std::fs::read_to_string(&path) {
            Ok(s) => s,
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Self::default(),
            Err(err) => {
                log::warn!("ama10: could not read {}: {err}", path.display());
                return Self::default();
            }
        };
        let parsed: FileShape = match serde_json::from_str(&raw) {
            Ok(v) => v,
            Err(err) => {
                log::warn!("ama10: malformed {}: {err}", path.display());
                return Self::default();
            }
        };
        let server = parsed
            .server_url
            .as_deref()
            .and_then(|s| ServerUrl::parse(s).ok())
            .unwrap_or_else(ServerUrl::default_saas);
        Self { server }
    }

    /// Atomically write the config to disk. Atomic because settings tend to
    /// be small and a half-written file would be worse than the previous
    /// state — write to a tempfile then rename.
    pub fn save(&self) -> Result<()> {
        let path = Self::path();
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)
                .with_context(|| format!("create config dir {}", parent.display()))?;
        }
        let payload = serde_json::to_vec_pretty(&FileShape {
            server_url: Some(self.server.as_str().to_string()),
        })?;
        let tmp = path.with_extension("json.tmp");
        std::fs::write(&tmp, payload)
            .with_context(|| format!("write temp config {}", tmp.display()))?;
        std::fs::rename(&tmp, &path).with_context(|| format!("rename to {}", path.display()))?;
        Ok(())
    }
}
