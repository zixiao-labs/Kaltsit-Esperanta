//! Esperanta-local glue between the editor and the Ama10 / Wuling DevOps stack.
//!
//! `ama10` ships the Wuling DevOps OpenAPI client and `ama10-ui` grows the
//! user-facing surface for it. This module is the single entry point the
//! editor calls during startup so the rest of `crates/zed` doesn't have to
//! sprout fork-local module references everywhere.
//!
//! What lives here:
//!
//!   - Declaration of the `ama10::SignIn` / `ama10::SignOut` actions so the
//!     command palette can surface them.
//!   - A `cx.observe_new(Workspace …)` that hooks the action handlers up to
//!     each workspace as it is created. The handler grabs the platform
//!     `CredentialsProvider` from the global `Client` and hands it to
//!     `ama10_ui::spawn_sign_in` / `spawn_sign_out`.
//!
//! What deliberately does *not* live here:
//!
//!   - AskPass interception. `ama10_ui::lookup_for_host` exists and is
//!     ready, but Zed's git layer constructs `AskPassDelegate` instances
//!     inline (see `crates/git/src/repository.rs:888-944,2318-2469,3460`)
//!     and there's no extension point to register a fallback delegate
//!     without modifying upstream. The intercept will land alongside the
//!     account panel work — see C3 follow-up in the plan.

use client::Client;
use gpui::{App, actions};
use workspace::Workspace;

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
    ]
);

pub fn init(cx: &mut App) {
    ama10_ui::init(cx);

    cx.observe_new(|workspace: &mut Workspace, _, _| {
        workspace
            .register_action(|_, _: &SignIn, _, cx| {
                let creds = Client::global(cx).credentials_provider();
                ama10_ui::spawn_sign_in(cx, creds);
            })
            .register_action(|_, _: &SignOut, _, cx| {
                let creds = Client::global(cx).credentials_provider();
                ama10_ui::spawn_sign_out(cx, creds);
            });
    })
    .detach();
}
