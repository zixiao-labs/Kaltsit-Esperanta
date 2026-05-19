//! `ama10-ui` — Wuling DevOps user-facing surface inside Kal'tsit·Esperanta.
//!
//! Stage-2 scope: the OAuth Device Authorization Grant flow ships behind a
//! command-palette action, with token storage going through the platform
//! `CredentialsProvider`. The richer account panel + issues / MR side panes
//! come in a later sprint; this crate's `init` registers the actions so
//! both can grow at their own pace without churning the `crates/zed` glue.
//!
//! The single entry point the editor's `crates/zed::ama10` calls during
//! startup is `init(cx)`. Everything else this crate offers is exposed via
//! free functions so the glue can wire one feature at a time.
//!
//! Public surface:
//!
//!   - [`init`] — register settings + log a one-line banner.
//!   - [`sign_in::spawn_sign_in`] / [`sign_in::spawn_sign_out`] — fire the
//!     OAuth Device flow / sign-out.
//!   - [`askpass_delegate::lookup_for_host`] — used by the git AskPass
//!     interceptor to inject Wuling tokens into HTTPS push/pull.
//!   - [`settings::WulingConfig`] — load/save the persistent config
//!     (server URL).

pub mod askpass_delegate;
pub mod settings;
pub mod sign_in;

use gpui::App;

pub use askpass_delegate::{lookup_for_host, LookupResult};
pub use settings::WulingConfig;
pub use sign_in::{spawn_sign_in, spawn_sign_out};

/// One-shot initialisation called from `crates/zed::ama10::init`. Today this
/// just primes the on-disk config so the very first sign-in attempt doesn't
/// race a file-create; future versions will also register command-palette
/// actions here once we have a public action-naming convention worked out
/// with the editor.
pub fn init(_cx: &mut App) {
    // Touch the config to ensure the directory exists and any malformed
    // file is loudly logged on startup rather than on first use.
    let _ = WulingConfig::load();
}
