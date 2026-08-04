//! Shared helpers for Wuling API calls from ama10-ui.

use std::collections::HashMap;
use std::sync::{Arc, Mutex, OnceLock};

use ama10::auth::{StoredCreds, WulingClient};
use ama10_i18n::tr;
use anyhow::{Context as _, Result, bail};
use async_lock::Mutex as AsyncMutex;
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
    if access_token_expired(&stored, now) {
        return refresh_access_token(client, now, cx).await;
    }
    Ok(stored.access_token)
}

async fn refresh_access_token(client: &WulingClient, now: i64, cx: &AsyncApp) -> Result<String> {
    let reconnect = || tr!("Reconnect Wuling DevOps").to_string();
    let lock = refresh_lock_for(client.server().as_str());
    let _guard = lock.lock().await;

    // Another caller may have finished refreshing while we waited for the lock.
    let stored = client.load_credentials(cx).await?.with_context(reconnect)?;
    if !access_token_expired(&stored, now) {
        return Ok(stored.access_token);
    }

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

fn access_token_expired(stored: &StoredCreds, now: i64) -> bool {
    stored.expires_at_unix > 0 && now >= stored.expires_at_unix
}

fn refresh_lock_for(server: &str) -> Arc<AsyncMutex<()>> {
    static LOCKS: OnceLock<Mutex<HashMap<String, Arc<AsyncMutex<()>>>>> = OnceLock::new();
    let locks = LOCKS.get_or_init(|| Mutex::new(HashMap::new()));
    let mut map = locks
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    map.entry(server.to_string())
        .or_insert_with(|| Arc::new(AsyncMutex::new(())))
        .clone()
}

pub fn require_slug(org_slug: Option<&str>) -> Result<String> {
    let slug = org_slug.map(str::trim).filter(|s| !s.is_empty());
    match slug {
        Some(slug) => Ok(slug.to_string()),
        None => bail!("{}", tr!("Select an organization first.")),
    }
}
