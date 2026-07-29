use std::sync::Arc;

use ama10::auth::WulingClient;
use ama10::connector::ConnectorId;
use ama10::github::GithubClient;
use anyhow::Result;
use credentials_provider::CredentialsProvider;
use gpui::{AsyncApp, Context, Window};
use settings::Settings as _;
use workspace::{Workspace, notifications::NotifyTaskExt as _};

use crate::settings::ConnectorSettings;

pub async fn disconnect(
    connector: ConnectorId,
    cx: &AsyncApp,
    credentials: Arc<dyn CredentialsProvider>,
) -> Result<()> {
    match connector {
        ConnectorId::Wuling => disconnect_wuling(cx, credentials).await,
        ConnectorId::Github => GithubClient::clear_stored_credentials(&credentials, cx).await,
    }
}

async fn disconnect_wuling(cx: &AsyncApp, credentials: Arc<dyn CredentialsProvider>) -> Result<()> {
    let server = cx.update(|cx| ConnectorSettings::get_global(cx).wuling_server.clone());
    let tokio_handle = cx.update(|cx| gpui_tokio::Tokio::handle(cx));
    let client = WulingClient::new(server, credentials, tokio_handle)?;
    match client.load_credentials(cx).await {
        Ok(Some(stored)) => match client.discover().await {
            Ok(well_known) => {
                if let Err(error) = client.revoke(&well_known, &stored.access_token).await {
                    log::warn!(
                        "ama10: Wuling token revocation failed; clearing it locally: {error}"
                    );
                }
            }
            Err(error) => {
                log::warn!(
                    "ama10: Wuling discovery failed during disconnect; clearing the token locally: {error}"
                );
            }
        },
        Ok(None) => {}
        Err(error) => {
            log::warn!(
                "ama10: failed to load Wuling credentials during disconnect; clearing them locally: {error}"
            );
        }
    }
    client.clear_credentials(cx).await
}

pub fn spawn_disconnect(
    workspace: &mut Workspace,
    connector: ConnectorId,
    window: &mut Window,
    cx: &mut Context<Workspace>,
    credentials: Arc<dyn CredentialsProvider>,
) {
    let workspace = workspace.weak_handle();
    cx.spawn(async move |_workspace, cx| {
        disconnect(connector, cx, credentials).await?;
        cx.update(|cx| crate::account_state::set_account(cx, connector, None));
        log::info!("ama10: disconnected {connector}");
        anyhow::Ok(())
    })
    .detach_and_notify_err(workspace, window, cx);
}
