//! Async Deno/JS extension session.

use std::path::PathBuf;
use std::sync::Arc;

use anyhow::{Result, anyhow};
use async_channel::bounded;
use async_host_runtime::{HostLifecycleCell, HostSession};

use crate::ensure_embedded_js_allowed;
use crate::runtime::{
    CapabilityCheck, DenoHostCommand, DenoHostEvent, SecureJsHost, run_host_loop,
};

#[derive(Clone, Debug)]
pub struct DenoExtensionSettings {
    pub enabled: bool,
    pub allow_remote_import: bool,
    pub max_heap_bytes: usize,
}

impl Default for DenoExtensionSettings {
    fn default() -> Self {
        Self {
            enabled: true,
            allow_remote_import: false,
            max_heap_bytes: 64 * 1024 * 1024,
        }
    }
}

pub struct AsyncDenoExtension {
    session: HostSession<DenoHostCommand, DenoHostEvent>,
}

impl AsyncDenoExtension {
    pub fn spawn(extension_root: PathBuf, settings: DenoExtensionSettings) -> Self {
        let capability_check: CapabilityCheck = Arc::new(|_| {
            Err(anyhow!(
                "capability not granted (manifest ∩ host granter required)"
            ))
        });
        Self::spawn_with_capabilities(extension_root, settings, capability_check)
    }

    pub fn spawn_with_capabilities(
        extension_root: PathBuf,
        settings: DenoExtensionSettings,
        capability_check: CapabilityCheck,
    ) -> Self {
        let session = HostSession::spawn_thread(
            "deno-extension",
            move || {
                ensure_embedded_js_allowed(&settings)?;
                if settings.allow_remote_import {
                    anyhow::bail!("allow_remote_import is forbidden in the embedded JS runtime");
                }
                let _ = settings.max_heap_bytes;
                SecureJsHost::load(extension_root, capability_check)
            },
            run_host_loop,
        );
        Self { session }
    }

    pub fn lifecycle(&self) -> &HostLifecycleCell {
        self.session.lifecycle()
    }

    pub async fn call_activate(&self) -> Result<()> {
        let (reply_tx, reply_rx) = bounded(1);
        self.session
            .send(DenoHostCommand::Activate { reply: reply_tx })
            .await?;
        reply_rx
            .recv()
            .await
            .map_err(|_| anyhow!("deno host closed"))?
    }

    pub async fn call_op(
        &self,
        name: impl Into<String>,
        args_json: impl Into<String>,
    ) -> Result<String> {
        let (reply_tx, reply_rx) = bounded(1);
        self.session
            .send(DenoHostCommand::CallOp {
                name: name.into(),
                args_json: args_json.into(),
                reply: reply_tx,
            })
            .await?;
        reply_rx
            .recv()
            .await
            .map_err(|_| anyhow!("deno host closed"))?
    }

    pub async fn import_module(&self, specifier: impl Into<String>) -> Result<()> {
        let (reply_tx, reply_rx) = bounded(1);
        self.session
            .send(DenoHostCommand::ImportModule {
                specifier: specifier.into(),
                reply: reply_tx,
            })
            .await?;
        reply_rx
            .recv()
            .await
            .map_err(|_| anyhow!("deno host closed"))?
    }
}
