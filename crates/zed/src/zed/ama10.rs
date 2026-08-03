//! Esperanta-local glue between the editor and connector accounts.
//!
//! This keeps fork-local connector wiring out of the rest of `crates/zed`.

use ama10_ui::{
    ConnectGithub, ConnectWuling, ConnectorAccountState, ConnectorId, DisconnectGithub,
    DisconnectWuling,
};
use client::Client;
use gpui::App;
use workspace::Workspace;

pub fn init(cx: &mut App) {
    ama10_ui::init(cx);
    let creds = Client::global(cx).credentials_provider();
    ConnectorAccountState::register(cx, creds);

    cx.observe_new(|workspace: &mut Workspace, _, _| {
        workspace
            .register_action(|workspace, _: &ConnectWuling, window, cx| {
                let creds = Client::global(cx).credentials_provider();
                ama10_ui::open_sign_in_modal(workspace, ConnectorId::Wuling, creds, window, cx);
            })
            .register_action(|workspace, _: &DisconnectWuling, window, cx| {
                let creds = Client::global(cx).credentials_provider();
                ama10_ui::spawn_disconnect(workspace, ConnectorId::Wuling, window, cx, creds);
            })
            .register_action(|workspace, _: &ConnectGithub, window, cx| {
                let creds = Client::global(cx).credentials_provider();
                ama10_ui::open_sign_in_modal(workspace, ConnectorId::Github, creds, window, cx);
            })
            .register_action(|workspace, _: &DisconnectGithub, window, cx| {
                let creds = Client::global(cx).credentials_provider();
                ama10_ui::spawn_disconnect(workspace, ConnectorId::Github, window, cx, creds);
            })
            .register_action(|workspace, _: &ama10_ui::OpenRunnerConfig, window, cx| {
                ama10_ui::open_or_reuse_runner_config(workspace, window, cx);
            })
            .register_action(|workspace, _: &ama10_ui::OpenWulingWorkflow, window, cx| {
                ama10_ui::open_or_reuse_workflow(workspace, None, window, cx);
            });
    })
    .detach();
}
