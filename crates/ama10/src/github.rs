use std::sync::Arc;
use std::time::Duration;

use anyhow::{Context as _, Result};
use credentials_provider::CredentialsProvider;
use gpui::AsyncApp;
use serde::{Deserialize, Serialize};

use crate::connector::{ConnectorAccount, ConnectorId};

const CREDENTIALS_KEY: &str = "ama10-connector://github";
const DEVICE_CODE_ENDPOINT: &str = "https://github.com/login/device/code";
const ACCESS_TOKEN_ENDPOINT: &str = "https://github.com/login/oauth/access_token";
const CURRENT_USER_ENDPOINT: &str = "https://api.github.com/user";

#[derive(Debug, Clone, Deserialize)]
pub struct GithubDeviceCode {
    pub device_code: String,
    pub user_code: String,
    pub verification_uri: String,
    pub expires_in: u64,
    pub interval: u64,
}

pub enum GithubPollResult {
    Pending,
    SlowDown,
    Denied,
    Expired,
    Issued(String),
}

#[derive(Debug, Deserialize)]
struct GithubTokenResponse {
    #[serde(default)]
    access_token: Option<String>,
    #[serde(default)]
    error: Option<String>,
    #[serde(default)]
    error_description: Option<String>,
}

#[derive(Debug, Deserialize)]
struct GithubUser {
    login: String,
    #[serde(default)]
    name: Option<String>,
    avatar_url: String,
    html_url: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct StoredGithubCredentials {
    access_token: String,
    account: ConnectorAccount,
}

#[derive(Clone)]
pub struct GithubClient {
    client_id: String,
    http: reqwest::Client,
    credentials: Arc<dyn CredentialsProvider>,
    tokio_handle: tokio::runtime::Handle,
}

impl GithubClient {
    pub fn new(
        client_id: String,
        credentials: Arc<dyn CredentialsProvider>,
        tokio_handle: tokio::runtime::Handle,
    ) -> Result<Self> {
        let client_id = client_id.trim().to_string();
        validate_client_id(&client_id)?;
        let http = reqwest::Client::builder()
            .user_agent(crate::wuling_api::user_agent("ama10-github"))
            .timeout(Duration::from_secs(30))
            .build()
            .context("build GitHub HTTP client")?;
        Ok(Self {
            client_id,
            http,
            credentials,
            tokio_handle,
        })
    }

    pub async fn device_flow_begin(&self) -> Result<GithubDeviceCode> {
        let client_id = self.client_id.clone();
        let http = self.http.clone();
        self.tokio_handle
            .spawn(async move {
                let response = http
                    .post(DEVICE_CODE_ENDPOINT)
                    .header(reqwest::header::ACCEPT, "application/json")
                    .form(&[("client_id", client_id), ("scope", "read:user".to_string())])
                    .send()
                    .await?
                    .error_for_status()?;
                response
                    .json::<GithubDeviceCode>()
                    .await
                    .context("decode GitHub device-code response")
            })
            .await?
    }

    pub async fn device_flow_poll(&self, device_code: &str) -> Result<GithubPollResult> {
        let client_id = self.client_id.clone();
        let device_code = device_code.to_string();
        let http = self.http.clone();
        self.tokio_handle
            .spawn(async move {
                let response = http
                    .post(ACCESS_TOKEN_ENDPOINT)
                    .header(reqwest::header::ACCEPT, "application/json")
                    .form(&[
                        ("client_id", client_id),
                        ("device_code", device_code),
                        (
                            "grant_type",
                            "urn:ietf:params:oauth:grant-type:device_code".to_string(),
                        ),
                    ])
                    .send()
                    .await?
                    .error_for_status()?;
                let response = response
                    .json::<GithubTokenResponse>()
                    .await
                    .context("decode GitHub token response")?;
                parse_poll_response(response)
            })
            .await?
    }

    pub async fn current_account(&self, access_token: &str) -> Result<ConnectorAccount> {
        let access_token = access_token.to_string();
        let http = self.http.clone();
        self.tokio_handle
            .spawn(async move {
                let user = http
                    .get(CURRENT_USER_ENDPOINT)
                    .header(reqwest::header::ACCEPT, "application/vnd.github+json")
                    .header("X-GitHub-Api-Version", "2022-11-28")
                    .bearer_auth(access_token)
                    .send()
                    .await?
                    .error_for_status()?
                    .json::<GithubUser>()
                    .await
                    .context("decode GitHub user response")?;
                Ok(ConnectorAccount {
                    connector: ConnectorId::Github,
                    display_name: user.name.unwrap_or_else(|| user.login.clone()),
                    username: user.login,
                    avatar_url: Some(user.avatar_url),
                    profile_url: Some(user.html_url),
                })
            })
            .await?
    }

    pub async fn save_credentials(
        &self,
        access_token: String,
        account: ConnectorAccount,
        cx: &AsyncApp,
    ) -> Result<()> {
        let payload = serde_json::to_vec(&StoredGithubCredentials {
            access_token,
            account: account.clone(),
        })?;
        self.credentials
            .write_credentials(CREDENTIALS_KEY, &account.username, &payload, cx)
            .await
    }

    pub async fn load_account(&self, cx: &AsyncApp) -> Result<Option<ConnectorAccount>> {
        Self::load_stored_account(&self.credentials, cx).await
    }

    pub async fn load_stored_account(
        credentials: &Arc<dyn CredentialsProvider>,
        cx: &AsyncApp,
    ) -> Result<Option<ConnectorAccount>> {
        let Some((_, payload)) = credentials.read_credentials(CREDENTIALS_KEY, cx).await? else {
            return Ok(None);
        };
        let stored = serde_json::from_slice::<StoredGithubCredentials>(&payload)
            .context("decode stored GitHub connector credentials")?;
        Ok(Some(stored.account))
    }

    pub async fn clear_credentials(&self, cx: &AsyncApp) -> Result<()> {
        Self::clear_stored_credentials(&self.credentials, cx).await
    }

    pub async fn clear_stored_credentials(
        credentials: &Arc<dyn CredentialsProvider>,
        cx: &AsyncApp,
    ) -> Result<()> {
        credentials.delete_credentials(CREDENTIALS_KEY, cx).await
    }
}

fn validate_client_id(client_id: &str) -> Result<()> {
    if client_id.trim().is_empty() {
        anyhow::bail!("GitHub OAuth App Client ID is not configured");
    }
    Ok(())
}

fn parse_poll_response(response: GithubTokenResponse) -> Result<GithubPollResult> {
    if let Some(access_token) = response.access_token {
        return Ok(GithubPollResult::Issued(access_token));
    }
    let error = response
        .error
        .context("GitHub token response contained neither an access token nor an error")?;
    Ok(match error.as_str() {
        "authorization_pending" => GithubPollResult::Pending,
        "slow_down" => GithubPollResult::SlowDown,
        "access_denied" => GithubPollResult::Denied,
        "expired_token" => GithubPollResult::Expired,
        _ => anyhow::bail!(
            "GitHub device flow failed: {} ({})",
            error,
            response.error_description.unwrap_or_default()
        ),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_device_flow_states() {
        let pending = parse_poll_response(GithubTokenResponse {
            access_token: None,
            error: Some("authorization_pending".to_string()),
            error_description: None,
        })
        .expect("pending response should be valid");
        assert!(matches!(pending, GithubPollResult::Pending));

        let issued = parse_poll_response(GithubTokenResponse {
            access_token: Some("token".to_string()),
            error: None,
            error_description: None,
        })
        .expect("token response should be valid");
        assert!(matches!(issued, GithubPollResult::Issued(token) if token == "token"));
    }

    #[test]
    fn rejects_missing_client_id_before_network_use() {
        assert!(validate_client_id("  ").is_err());
    }
}
