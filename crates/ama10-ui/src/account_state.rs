use std::collections::HashMap;
use std::sync::Arc;

use ama10::auth::WulingClient;
use ama10::connector::{ConnectorAccount, ConnectorId};
use ama10::github::GithubClient;
use ama10::server_url::ServerUrl;
use anyhow::{Context as _, Result};
use credentials_provider::CredentialsProvider;
use gpui::{App, AppContext as _, Entity, EventEmitter, Global, Subscription};
use settings::{Settings as _, SettingsStore};

use crate::settings::{ConnectorSettings, WulingConfig};

pub struct ConnectorAccountState {
    accounts: HashMap<ConnectorId, ConnectorAccount>,
    wuling_server: ServerUrl,
}

pub struct ConnectorAccountsChanged;
impl EventEmitter<ConnectorAccountsChanged> for ConnectorAccountState {}

struct GlobalConnectorAccountState {
    entity: Entity<ConnectorAccountState>,
    _settings_subscription: Subscription,
}
impl Global for GlobalConnectorAccountState {}

impl ConnectorAccountState {
    pub fn register(cx: &mut App, credentials: Arc<dyn CredentialsProvider>) {
        if cx.has_global::<GlobalConnectorAccountState>() {
            return;
        }
        let wuling_server = ConnectorSettings::get_global(cx).wuling_server.clone();
        let entity = cx.new(|_| Self {
            accounts: HashMap::new(),
            wuling_server,
        });
        let settings_subscription = cx.observe_global::<SettingsStore>({
            let entity = entity.clone();
            let credentials = credentials.clone();
            move |cx| {
                let server = ConnectorSettings::get_global(cx).wuling_server.clone();
                let server_changed = entity.update(cx, |this, cx| {
                    if this.wuling_server == server {
                        return false;
                    }
                    this.wuling_server = server;
                    this.set(ConnectorId::Wuling, None, cx);
                    true
                });
                if server_changed {
                    let entity = entity.clone();
                    let credentials = credentials.clone();
                    cx.spawn(async move |cx| {
                        match load_wuling_account(&credentials, cx).await {
                            Ok(account) => {
                                entity.update(cx, |this, cx| {
                                    this.set(ConnectorId::Wuling, account, cx)
                                });
                            }
                            Err(error) => {
                                log::warn!(
                                    "ama10: failed to restore Wuling account after a server change: {error:#}"
                                );
                            }
                        }
                    })
                    .detach();
                }
            }
        });
        cx.set_global(GlobalConnectorAccountState {
            entity: entity.clone(),
            _settings_subscription: settings_subscription,
        });
        cx.spawn(async move |cx| {
            match load_wuling_account(&credentials, cx).await {
                Ok(Some(account)) => {
                    entity.update(cx, |this, cx| {
                        this.set(account.connector, Some(account), cx)
                    });
                }
                Ok(None) => {}
                Err(error) => log::warn!("ama10: failed to restore Wuling account: {error:#}"),
            }
            match load_github_account(&credentials, cx).await {
                Ok(Some(account)) => {
                    entity.update(cx, |this, cx| {
                        this.set(account.connector, Some(account), cx)
                    });
                }
                Ok(None) => {}
                Err(error) => log::warn!("ama10: failed to restore GitHub account: {error:#}"),
            }
        })
        .detach();
    }

    pub fn try_global(cx: &App) -> Option<Entity<Self>> {
        cx.try_global::<GlobalConnectorAccountState>()
            .map(|state| state.entity.clone())
    }

    pub fn account(&self, connector: ConnectorId) -> Option<&ConnectorAccount> {
        self.accounts.get(&connector)
    }

    pub fn accounts(&self) -> impl Iterator<Item = &ConnectorAccount> {
        self.accounts.values()
    }

    pub fn set(
        &mut self,
        connector: ConnectorId,
        account: Option<ConnectorAccount>,
        cx: &mut gpui::Context<Self>,
    ) {
        let changed = match account {
            Some(account) if self.accounts.get(&connector) == Some(&account) => false,
            Some(account) => {
                self.accounts.insert(connector, account);
                true
            }
            None => self.accounts.remove(&connector).is_some(),
        };
        if changed {
            cx.emit(ConnectorAccountsChanged);
            cx.notify();
        }
    }
}

pub fn set_account(cx: &mut App, connector: ConnectorId, account: Option<ConnectorAccount>) {
    let Some(state) = ConnectorAccountState::try_global(cx) else {
        return;
    };
    state.update(cx, |this, cx| this.set(connector, account, cx));
}

async fn load_wuling_account(
    credentials: &Arc<dyn CredentialsProvider>,
    cx: &mut gpui::AsyncApp,
) -> Result<Option<ConnectorAccount>> {
    let config = cx.update(|cx| WulingConfig::load(cx));
    let key = config.server.as_str().to_string();
    let Some((_, bytes)) = credentials.read_credentials(&key, cx).await? else {
        return Ok(None);
    };
    let stored: ama10::auth::StoredCreds = serde_json::from_slice(&bytes)?;
    let tokio_handle = cx.update(|cx| gpui_tokio::Tokio::handle(cx));
    let client = WulingClient::new(config.server.clone(), credentials.clone(), tokio_handle)?;
    let now = i64::try_from(
        std::time::UNIX_EPOCH
            .elapsed()
            .context("system clock is before the Unix epoch")?
            .as_secs(),
    )
    .context("current Unix timestamp does not fit in i64")?;
    let access_token = if stored.expires_at_unix > 0 && now >= stored.expires_at_unix {
        let Some(refresh_token) = stored.refresh_token.as_deref() else {
            return Ok(None);
        };
        refresh_wuling_token(&client, &stored, refresh_token, now, cx).await?
    } else {
        stored.access_token
    };
    let user = client.current_user(&access_token).await?;
    Ok(Some(ConnectorAccount {
        connector: ConnectorId::Wuling,
        display_name: user.display_name,
        username: user.username,
        avatar_url: (!user.avatar_url.is_empty()).then_some(user.avatar_url),
        profile_url: Some(config.server.as_str().to_string()),
    }))
}

async fn refresh_wuling_token(
    client: &WulingClient,
    stored: &ama10::auth::StoredCreds,
    refresh_token: &str,
    now: i64,
    cx: &mut gpui::AsyncApp,
) -> Result<String> {
    let well_known = client.discover().await?;
    let tokens = client.refresh(&well_known, refresh_token).await?;
    let access_token = tokens.access_token.clone();
    let expires_at = now
        .checked_add(tokens.expires_in)
        .context("Wuling token expiration timestamp overflowed")?;
    client
        .save_credentials(cx, &stored.username, &tokens, expires_at)
        .await?;
    Ok(access_token)
}

async fn load_github_account(
    credentials: &Arc<dyn CredentialsProvider>,
    cx: &mut gpui::AsyncApp,
) -> Result<Option<ConnectorAccount>> {
    GithubClient::load_stored_account(credentials, cx).await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn state_returns_accounts_by_connector() {
        let account = ConnectorAccount {
            connector: ConnectorId::Github,
            username: "amiya".to_string(),
            display_name: "Amiya".to_string(),
            avatar_url: None,
            profile_url: None,
        };
        let mut state = ConnectorAccountState {
            accounts: HashMap::from([(ConnectorId::Github, account.clone())]),
            wuling_server: ServerUrl::default_saas(),
        };

        assert_eq!(state.account(ConnectorId::Github), Some(&account));
        assert!(state.account(ConnectorId::Wuling).is_none());
        state.accounts.clear();
        assert_eq!(state.accounts().count(), 0);
    }
}
