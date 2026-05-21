//! Session-scoped, observable record of "am I signed into Wuling DevOps right
//! now?". Backed by an in-memory `Entity<WulingAccountState>` that lives in
//! `App` globals so any view (e.g. the title bar) can subscribe and rerender
//! when sign-in state changes without polling the keychain.
//!
//! On `init`, we kick off a best-effort async load from the
//! `CredentialsProvider` so a previously-signed-in user starts the session
//! with the chip lit up. The sign-in modal updates the state directly on
//! success; sign-out clears it. The persistent keychain entry is the
//! authoritative source — the in-memory state is just a cached view for the
//! UI.

use std::sync::Arc;

use ama10::auth::WulingClient;
use ama10::server_url::ServerUrl;
use anyhow::Result;
use credentials_provider::CredentialsProvider;
use gpui::{App, AppContext as _, Entity, EventEmitter, Global};

use crate::settings::WulingConfig;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WulingAccount {
    pub username: String,
    pub server: ServerUrl,
}

pub struct WulingAccountState {
    account: Option<WulingAccount>,
}

pub struct WulingAccountChanged;
impl EventEmitter<WulingAccountChanged> for WulingAccountState {}

struct GlobalWulingAccountState(Entity<WulingAccountState>);
impl Global for GlobalWulingAccountState {}

impl WulingAccountState {
    /// Register the global singleton and start the best-effort async load
    /// from the platform credentials store. Idempotent; safe to call multiple
    /// times (a second call short-circuits).
    pub fn register(cx: &mut App, creds: Arc<dyn CredentialsProvider>) {
        if cx.has_global::<GlobalWulingAccountState>() {
            return;
        }
        let entity = cx.new(|_| WulingAccountState { account: None });
        cx.set_global(GlobalWulingAccountState(entity.clone()));
        cx.spawn(async move |cx| {
            if let Ok(Some(account)) = load_from_keychain(&creds, cx).await {
                entity.update(cx, |this, cx| {
                    // A concurrent sign-in / sign-out during the async
                    // keychain load takes precedence — only backfill if the
                    // slot is still empty.
                    if this.account.is_none() {
                        this.set(Some(account), cx);
                    }
                });
            }
        })
        .detach();
    }

    pub fn try_global(cx: &App) -> Option<Entity<Self>> {
        cx.try_global::<GlobalWulingAccountState>()
            .map(|g| g.0.clone())
    }

    pub fn account(&self) -> Option<&WulingAccount> {
        self.account.as_ref()
    }

    pub fn set(&mut self, account: Option<WulingAccount>, cx: &mut gpui::Context<Self>) {
        if self.account == account {
            return;
        }
        self.account = account;
        cx.emit(WulingAccountChanged);
        cx.notify();
    }
}

/// Update the global state from anywhere (e.g. the sign-in modal or sign-out
/// helper). No-op if the state has never been registered (which only happens
/// in tests / non-editor contexts).
pub fn set_account(cx: &mut App, account: Option<WulingAccount>) {
    let Some(state) = WulingAccountState::try_global(cx) else {
        return;
    };
    state.update(cx, |this, cx| this.set(account, cx));
}

async fn load_from_keychain(
    creds: &Arc<dyn CredentialsProvider>,
    cx: &mut gpui::AsyncApp,
) -> Result<Option<WulingAccount>> {
    let config = WulingConfig::load();
    let url = config.server.as_str().to_string();
    let Some((_, bytes)) = creds.read_credentials(&url, cx).await? else {
        return Ok(None);
    };
    let stored: ama10::auth::StoredCreds = serde_json::from_slice(&bytes)?;
    let now = std::time::UNIX_EPOCH
        .elapsed()
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0);
    // If the access_token is past its sell-by but a refresh_token is on hand,
    // try to mint a fresh pair so the chip stays lit through a long idle
    // (access_token defaults to 1h, refresh to 14d). On any refresh error we
    // surface as signed-out — the askpass interceptor will retry on the next
    // git op and the user can re-sign-in via the panel/modal.
    if stored.expires_at_unix > 0 && now >= stored.expires_at_unix {
        let Some(refresh_token) = stored.refresh_token.as_deref() else {
            return Ok(None);
        };
        let refreshed_username =
            match refresh_in_background(&config.server, &stored, refresh_token, creds, cx).await {
                Ok(name) => name,
                Err(err) => {
                    log::info!("ama10: startup token refresh failed: {err:#}");
                    return Ok(None);
                }
            };
        return Ok(Some(WulingAccount {
            username: refreshed_username,
            server: config.server,
        }));
    }
    Ok(Some(WulingAccount {
        username: stored.username,
        server: config.server,
    }))
}

async fn refresh_in_background(
    server: &ServerUrl,
    stored: &ama10::auth::StoredCreds,
    refresh_token: &str,
    creds: &Arc<dyn CredentialsProvider>,
    cx: &mut gpui::AsyncApp,
) -> Result<String> {
    let tokio_handle = cx.update(|cx| gpui_tokio::Tokio::handle(cx));
    let client = WulingClient::new(server.clone(), creds.clone(), tokio_handle);
    let well_known = client.discover().await?;
    let tokens = client.refresh(&well_known, refresh_token).await?;
    let now = std::time::UNIX_EPOCH
        .elapsed()
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0);
    let expires_at = now + tokens.expires_in as i64;
    client
        .save_credentials(cx, &stored.username, &tokens, expires_at)
        .await?;
    Ok(stored.username.clone())
}
