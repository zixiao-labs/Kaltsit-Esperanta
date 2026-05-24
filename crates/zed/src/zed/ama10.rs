//! Esperanta-local glue between the editor and the Ama10 / Wuling DevOps stack.
//!
//! `ama10` ships the Wuling DevOps OpenAPI client and `ama10-ui` grows the
//! user-facing surface for it. This module is the single entry point the
//! editor calls during startup so the rest of `crates/zed` doesn't have to
//! sprout fork-local module references everywhere.
//!
//! What lives here:
//!
//!   - A `cx.observe_new(Workspace …)` that hooks the `ama10_ui::SignIn` /
//!     `ama10_ui::SignOut` action handlers up to each workspace as it is
//!     created. The handler grabs the platform `CredentialsProvider` from the
//!     global `Client` and hands it to `ama10_ui::open_sign_in_modal` /
//!     `ama10_ui::spawn_sign_out`.
//!
//! The action types themselves live in `ama10-ui` so other crates (notably
//! `title_bar`) can dispatch them without taking a dependency on the editor
//! binary.

use ama10_ui::{SetServerUrl, SignIn, SignOut, WulingAccountState};
use client::Client;
use gpui::App;
use workspace::Workspace;

pub fn init(cx: &mut App) {
    ama10_ui::init(cx);
    let creds = Client::global(cx).credentials_provider();
    WulingAccountState::register(cx, creds);

    cx.observe_new(|workspace: &mut Workspace, _, _| {
        workspace
            .register_action(|workspace, _: &SignIn, window, cx| {
                let creds = Client::global(cx).credentials_provider();
                ama10_ui::open_sign_in_modal(workspace, creds, window, cx);
            })
            .register_action(|_, _: &SignOut, _, cx| {
                let creds = Client::global(cx).credentials_provider();
                ama10_ui::spawn_sign_out(cx, creds);
            })
            .register_action(|workspace, _: &SetServerUrl, window, cx| {
                ama10_ui::open_server_url_modal(workspace, window, cx);
            });
        ama10_ui::register_workspace_actions(workspace);
    })
    .detach();
}
