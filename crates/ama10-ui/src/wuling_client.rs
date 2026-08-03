//! Shared helpers for Wuling API calls from ama10-ui.

use std::sync::Arc;

use ama10::auth::WulingClient;
use anyhow::{Context as _, Result, bail};
use credentials_provider::CredentialsProvider;
use gpui::{App, AsyncApp};
use settings::Settings as _;

use crate::settings::ConnectorSettings;

pub fn wuling_client(
    credentials: Arc<dyn CredentialsProvider>,
    cx: &App,
) -> Result<WulingClient> {
    let server = ConnectorSettings::get_global(cx).wuling_server.clone();
    let tokio_handle = gpui_tokio::Tokio::handle(cx);
    WulingClient::new(server, credentials, tokio_handle)
}

pub async fn load_access_token(client: &WulingClient, cx: &AsyncApp) -> Result<String> {
    let stored = client
        .load_credentials(cx)
        .await?
        .context("Connect Wuling DevOps first")?;
    Ok(stored.access_token)
}

pub fn require_slug(org_slug: Option<&str>) -> Result<String> {
    let slug = org_slug.map(str::trim).filter(|s| !s.is_empty());
    match slug {
        Some(slug) => Ok(slug.to_string()),
        None => bail!("Select an organization first"),
    }
}
