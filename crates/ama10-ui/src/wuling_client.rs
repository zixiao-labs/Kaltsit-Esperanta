//! Shared helpers for Wuling API calls from ama10-ui.

use std::sync::Arc;

use ama10::auth::{StoredCreds, WulingClient};
use ama10_i18n::tr;
use anyhow::{Context as _, Result, bail};
use credentials_provider::CredentialsProvider;
use gpui::{App, AsyncApp};
use settings::Settings as _;

use crate::settings::ConnectorSettings;

pub fn wuling_client(credentials: Arc<dyn CredentialsProvider>, cx: &App) -> Result<WulingClient> {
    let server = ConnectorSettings::get_global(cx).wuling_server.clone();
    let tokio_handle = gpui_tokio::Tokio::handle(cx);
    WulingClient::new(server, credentials, tokio_handle)
}

pub async fn load_access_token(client: &WulingClient, cx: &AsyncApp) -> Result<String> {
    let stored = client
        .load_credentials(cx)
        .await?
        .with_context(|| tr!("Connect Wuling DevOps first").to_string())?;
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .context("system clock is before the Unix epoch")?
        .as_secs()
        .try_into()
        .context("current Unix timestamp does not fit in i64")?;
    if stored.expires_at_unix > 0 && now >= stored.expires_at_unix {
        return refresh_access_token(client, &stored, now, cx).await;
    }
    Ok(stored.access_token)
}

async fn refresh_access_token(
    client: &WulingClient,
    stored: &StoredCreds,
    now: i64,
    cx: &AsyncApp,
) -> Result<String> {
    let reconnect = || tr!("Reconnect Wuling DevOps").to_string();
    let Some(refresh_token) = stored.refresh_token.as_deref() else {
        bail!("{}", reconnect());
    };
    let well_known = client.discover().await.with_context(reconnect)?;
    let tokens = client
        .refresh(&well_known, refresh_token)
        .await
        .with_context(reconnect)?;
    let access_token = tokens.access_token.clone();
    let expires_at = now
        .checked_add(tokens.expires_in)
        .context("Wuling token expiration timestamp overflowed")?;
    client
        .save_credentials(cx, &stored.username, &tokens, expires_at)
        .await
        .with_context(reconnect)?;
    Ok(access_token)
}

pub fn require_slug(org_slug: Option<&str>) -> Result<String> {
    let slug = org_slug.map(str::trim).filter(|s| !s.is_empty());
    match slug {
        Some(slug) => Ok(slug.to_string()),
        None => bail!("{}", tr!("Select an organization first.")),
    }
}
