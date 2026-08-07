//! Secure JS host loop (stub or deno_core).

use std::path::PathBuf;
use std::sync::Arc;

use anyhow::{Result, anyhow};
use async_channel::Sender;
use async_host_runtime::{HostCommand, HostLifecycleCell, handle_command_result};

use crate::loader::{resolve_local_module, validate_import_specifier};

#[derive(Debug)]
pub enum DenoHostCommand {
    Activate {
        reply: Sender<Result<()>>,
    },
    CallOp {
        name: String,
        args_json: String,
        reply: Sender<Result<String>>,
    },
    ImportModule {
        specifier: String,
        reply: Sender<Result<()>>,
    },
}

#[derive(Debug)]
pub enum DenoHostEvent {
    Log(String),
    Activated,
}

/// Capability check callback supplied by the extension host.
pub type CapabilityCheck = Arc<dyn Fn(&str) -> Result<()> + Send + Sync>;

pub struct SecureJsHost {
    extension_root: PathBuf,
    entry_source: String,
    activated: bool,
    capability_check: CapabilityCheck,
}

impl SecureJsHost {
    pub fn load(extension_root: PathBuf, capability_check: CapabilityCheck) -> Result<Self> {
        let entry = ["extension.js", "extension.ts", "extension.mjs"]
            .iter()
            .map(|name| extension_root.join(name))
            .find(|path| path.is_file())
            .ok_or_else(|| anyhow!("no extension.js/ts entry in {}", extension_root.display()))?;
        let entry_source = std::fs::read_to_string(&entry)?;
        Ok(Self {
            extension_root,
            entry_source,
            activated: false,
            capability_check,
        })
    }

    pub fn activate(&mut self, events: &Sender<DenoHostEvent>) -> Result<()> {
        let _ = &self.entry_source;
        #[cfg(feature = "deno-core")]
        {
            self.activate_with_deno_core()?;
        }
        self.activated = true;
        let _ = events.send_blocking(DenoHostEvent::Activated);
        let _ = events.send_blocking(DenoHostEvent::Log(
            "secure JS extension activated (zero ambient I/O)".into(),
        ));
        Ok(())
    }

    #[cfg(feature = "deno-core")]
    fn activate_with_deno_core(&mut self) -> Result<()> {
        use deno_core::{JsRuntime, RuntimeOptions};
        let mut runtime = JsRuntime::new(RuntimeOptions {
            module_loader: None,
            ..Default::default()
        });
        let _ = runtime.execute_script("<extension>", self.entry_source.clone())?;
        Ok(())
    }

    pub fn import_module(&self, specifier: &str) -> Result<()> {
        validate_import_specifier(specifier)?;
        let _module = resolve_local_module(&self.extension_root, specifier)?;
        Ok(())
    }

    pub fn call_op(&self, name: &str, args_json: &str) -> Result<String> {
        if !self.activated {
            return Err(anyhow!("extension is not activated"));
        }
        match name {
            "process:exec" | "download_file" | "npm:install" => {
                (self.capability_check)(name)?;
                Err(anyhow!(
                    "{name} is capability-gated and not implemented in stub host (args={args_json})"
                ))
            }
            "log" => Ok(format!("logged:{args_json}")),
            other => Err(anyhow!("unknown op `{other}`")),
        }
    }
}

pub(crate) fn run_host_loop(
    mut host: SecureJsHost,
    command_rx: async_channel::Receiver<HostCommand<DenoHostCommand>>,
    event_tx: Sender<DenoHostEvent>,
    _lifecycle: HostLifecycleCell,
) {
    while handle_command_result(command_rx.recv_blocking(), |command| {
        match command {
            DenoHostCommand::Activate { reply } => {
                let result = host.activate(&event_tx);
                let _ = reply.send_blocking(result);
            }
            DenoHostCommand::CallOp {
                name,
                args_json,
                reply,
            } => {
                let result = host.call_op(&name, &args_json);
                let _ = reply.send_blocking(result);
            }
            DenoHostCommand::ImportModule { specifier, reply } => {
                let result = host.import_module(&specifier);
                let _ = reply.send_blocking(result);
            }
        }
        Ok(())
    }) {}
}
