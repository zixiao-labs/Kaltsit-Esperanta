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

pub mod account_state;
pub mod askpass_delegate;
pub mod server_url_modal;
pub mod settings;
pub mod sign_in;
pub mod sign_in_modal;

use gpui::{App, actions};

pub use account_state::{WulingAccount, WulingAccountChanged, WulingAccountState};
pub use askpass_delegate::{LookupResult, lookup_for_host};
pub use server_url_modal::open_server_url_modal;
pub use settings::WulingConfig;
pub use sign_in::{spawn_sign_in, spawn_sign_out};
pub use sign_in_modal::open_sign_in_modal;

/// Re-exported for downstream crates (e.g. `settings_ui`) that want to display
/// the SaaS Wuling DevOps URL without taking a direct dependency on `ama10`.
pub const DEFAULT_SERVER_URL: &str = ama10::server_url::DEFAULT_SERVER_URL;

actions!(
    ama10,
    [
        /// Begin the OAuth 2.1 Device Authorization Grant flow against the
        /// configured Wuling DevOps server, prompting the user to visit the
        /// verification URL and enter the displayed user_code.
        SignIn,
        /// Revoke the stored Wuling DevOps access token and clear the
        /// platform keychain entry. Best-effort: a network failure during
        /// revoke still clears the local credentials.
        SignOut,
        /// Open a text-input modal that edits the persisted Wuling DevOps
        /// server URL in `wuling.json`.
        SetServerUrl,
    ]
);

/// One-shot initialisation called from `crates/zed::ama10::init`. Today this
/// just primes the on-disk config so the very first sign-in attempt doesn't
/// race a file-create; future versions will also register command-palette
/// actions here once we have a public action-naming convention worked out
/// with the editor.
pub fn init(_cx: &mut App) {
    // Touch the config so any malformed file is logged (inside `load`) on
    // startup rather than on first use.
    WulingConfig::load();
}
