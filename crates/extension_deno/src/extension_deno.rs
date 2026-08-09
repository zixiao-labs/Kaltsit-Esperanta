//! Deno / JS extension runtime with zero ambient I/O.
//!
//! Default build uses a secure stub host that still enforces the security
//! contracts (no remote imports, capability-gated ops, async load). Enable the
//! `deno-core` feature to embed `deno_core::JsRuntime`.

mod loader;
mod runtime;
mod session;

pub use async_host_runtime::{HostLifecycle, HostLifecycleCell};
pub use loader::{ModuleSpec, resolve_local_module, validate_import_specifier};
pub use runtime::{DenoHostCommand, DenoHostEvent, SecureJsHost};
pub use session::{AsyncDenoExtension, DenoExtensionSettings};

use anyhow::Result;
use std::path::Path;

/// Detect whether an extension directory should be treated as Deno/JS.
pub fn is_deno_extension_dir(path: &Path) -> bool {
    path.join("extension.js").is_file()
        || path.join("extension.ts").is_file()
        || path.join("extension.mjs").is_file()
}

/// Load settings for embedded JS; `enabled=false` refuse to start (ultra/disabled).
pub fn ensure_embedded_js_allowed(settings: &DenoExtensionSettings) -> Result<()> {
    if !settings.enabled {
        anyhow::bail!("embedded JS extensions are disabled by settings");
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn rejects_remote_imports() {
        assert!(validate_import_specifier("https://evil.example/x.js").is_err());
        assert!(validate_import_specifier("http://evil.example/x.js").is_err());
        assert!(validate_import_specifier("./local.js").is_ok());
        assert!(validate_import_specifier("file:///tmp/x.js").is_err());
    }

    #[test]
    fn async_host_loads_stub_extension() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join("extension.js"),
            "export function activate() {}",
        )
        .unwrap();
        let host =
            AsyncDenoExtension::spawn(dir.path().to_path_buf(), DenoExtensionSettings::default());
        let deadline = std::time::Instant::now() + Duration::from_secs(2);
        while !host.lifecycle().get().is_ready() {
            assert!(
                !host.lifecycle().get().is_failed(),
                "unexpected failure: {:?}",
                host.lifecycle().failure_message()
            );
            assert!(std::time::Instant::now() < deadline);
            std::thread::sleep(Duration::from_millis(5));
        }
        smol::block_on(host.call_activate()).expect("activate");
    }

    #[test]
    fn disabled_settings_fail_soft() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("extension.js"), "").unwrap();
        let mut settings = DenoExtensionSettings::default();
        settings.enabled = false;
        let host = AsyncDenoExtension::spawn(dir.path().to_path_buf(), settings);
        let deadline = std::time::Instant::now() + Duration::from_secs(2);
        loop {
            match host.lifecycle().get() {
                HostLifecycle::Failed { message } => {
                    assert!(message.contains("disabled"));
                    break;
                }
                HostLifecycle::Ready => panic!("disabled JS must not become Ready"),
                HostLifecycle::Loading => {
                    assert!(std::time::Instant::now() < deadline);
                    std::thread::sleep(Duration::from_millis(5));
                }
            }
        }
    }
}
