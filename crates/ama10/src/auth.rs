//! OAuth Device Authorization Grant client for Wuling DevOps.
//!
//! What this module does:
//!
//!   1. Fetches the IdP discovery doc (`/.well-known/wuling-clients`) so the
//!      official desktop client_id is whatever the server says it is —
//!      Esperanta works against both the SaaS instance and self-hosted
//!      Wuling deployments without a config flag.
//!   2. Walks the RFC 8628 device flow: ask for a device_code+user_code,
//!      show the user_code to the user via the UI, poll /token until the
//!      user approves (or we hit slow_down / expired / denied).
//!   3. Persists the resulting (access_token, refresh_token, scopes,
//!      expires_at) into the platform `CredentialsProvider` keyed by server
//!      URL — so multiple Wuling instances and the Zed account live side
//!      by side without colliding.
//!
//! What this module deliberately does NOT do:
//!
//!   - Authorization-code-with-PKCE. Esperanta is a desktop app on a host
//!     that may or may not be able to open a loopback HTTP server (think
//!     enterprise lockdowns); device flow needs neither a browser nor a
//!     local port and is the simplest fit. Web clients use the auth-code
//!     flow via the React SPA instead.
//!   - HTTP retries / circuit-breaking. We rely on the caller to surface
//!     transient failures as toasts; getting fancy here would obscure real
//!     server issues during early integration.
//!
//! The HTTP client is *stock* `reqwest 0.13` — explicitly NOT the
//! workspace `zed-reqwest` shim. See ama10/Cargo.toml for why.

use std::sync::Arc;
use std::time::Duration;

use anyhow::{Context as _, Result};
use credentials_provider::CredentialsProvider;
use gpui::AsyncApp;
use serde::{Deserialize, Serialize};

use crate::server_url::ServerUrl;

/// Discovery document returned by `/.well-known/wuling-clients`. Only the
/// fields Esperanta actually uses are deserialised; unknown fields are
/// ignored so server-side schema growth doesn't break older clients.
#[derive(Debug, Clone, Deserialize)]
pub struct WellKnown {
    pub issuer: String,
    pub desktop_official_client_id: String,
    pub authorization_endpoint: String,
    pub token_endpoint: String,
    pub device_authorization_endpoint: String,
    pub revocation_endpoint: String,
    pub frontend_device_verification_uri: String,
    pub scopes_supported: Vec<String>,
}

/// One step's result from the device-flow polling loop.
pub enum PollResult {
    /// User hasn't decided yet; caller should keep polling.
    Pending,
    /// Server says we're polling too fast; bump the interval before retrying.
    SlowDown,
    /// User denied the request.
    Denied,
    /// The device_code expired before the user acted.
    Expired,
    /// Issued: we have tokens.
    Issued(Tokens),
}

/// The (access_token, refresh_token, scopes, expires_at) bundle we hand back
/// to the caller and persist via CredentialsProvider.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Tokens {
    pub access_token: String,
    pub refresh_token: Option<String>,
    pub expires_in: u64,
    pub scope: String,
}

impl Tokens {
    pub fn scopes(&self) -> Vec<&str> {
        self.scope.split_whitespace().collect()
    }
}

/// The DevAuth response from `/api/v1/oauth/device_authorization`.
#[derive(Debug, Clone, Deserialize)]
pub struct DeviceCodeResp {
    pub device_code: String,
    pub user_code: String,
    pub verification_uri: String,
    pub verification_uri_complete: String,
    pub expires_in: u64,
    pub interval: u64,
}

/// `me` is `/api/v1/auth/me`. Subset that the account panel renders.
#[derive(Debug, Clone, Deserialize)]
pub struct Me {
    pub id: String,
    pub username: String,
    pub display_name: String,
    #[serde(default)]
    pub email: String,
    #[serde(default)]
    pub github_login: String,
}

/// The persisted credentials envelope. We serialise this into the password
/// slot of `CredentialsProvider::write_credentials` so a future Esperanta
/// release can add fields without changing the keychain layout.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StoredCreds {
    pub access_token: String,
    pub refresh_token: Option<String>,
    pub expires_at_unix: i64,
    pub scope: String,
    pub username: String,
}

/// A Wuling DevOps OAuth client bound to a single server URL.
///
/// Lifecycle is: construct once with `new`, then call `sign_in_device_flow`
/// to grab tokens, `refresh` to rotate them, and `current_user` / `revoke`
/// as needed. `load_stored` rehydrates from `CredentialsProvider`.
#[derive(Clone)]
pub struct WulingClient {
    server: ServerUrl,
    http: reqwest::Client,
    creds: Arc<dyn CredentialsProvider>,
    // `reqwest 0.13` schedules its request timeout via `tokio::time::sleep`,
    // which panics when its future is polled outside a Tokio runtime. GPUI's
    // executor is not a Tokio runtime, so we run each HTTP call through
    // `handle.spawn(...)` to hand polling to the workspace `gpui_tokio`
    // runtime. See `device_flow_request` / the per-method spawn calls below.
    tokio_handle: tokio::runtime::Handle,
}

impl WulingClient {
    pub fn new(
        server: ServerUrl,
        creds: Arc<dyn CredentialsProvider>,
        tokio_handle: tokio::runtime::Handle,
    ) -> Self {
        let http = reqwest::Client::builder()
            .user_agent(user_agent())
            .timeout(Duration::from_secs(30))
            .build()
            .expect("reqwest builder with stock defaults cannot fail");
        Self {
            server,
            http,
            creds,
            tokio_handle,
        }
    }

    pub fn server(&self) -> &ServerUrl {
        &self.server
    }

    /// GET `/.well-known/wuling-clients`. Cheap; fine to call on every
    /// sign-in attempt — the server caches via Cache-Control: max-age=300.
    pub async fn discover(&self) -> Result<WellKnown> {
        let url = self.server.join("/.well-known/wuling-clients");
        let http = self.http.clone();
        self.tokio_handle
            .spawn(async move {
                let resp = http.get(url).send().await?.error_for_status()?;
                anyhow::Ok(resp.json::<WellKnown>().await?)
            })
            .await?
    }

    /// POST `/api/v1/oauth/device_authorization` for a public client.
    /// Returns the device_code + user_code tuple to show the user.
    pub async fn device_flow_begin(
        &self,
        well_known: &WellKnown,
        scopes: &[&str],
    ) -> Result<DeviceCodeResp> {
        let scope_value = scopes.join(" ");
        let body = url::form_urlencoded::Serializer::new(String::new())
            .append_pair("client_id", &well_known.desktop_official_client_id)
            .append_pair("scope", &scope_value)
            .finish();
        let endpoint = well_known.device_authorization_endpoint.clone();
        let http = self.http.clone();
        self.tokio_handle
            .spawn(async move {
                let resp = http
                    .post(&endpoint)
                    .header(
                        reqwest::header::CONTENT_TYPE,
                        "application/x-www-form-urlencoded",
                    )
                    .body(body)
                    .send()
                    .await?;
                let status = resp.status();
                let bytes = resp.bytes().await?;
                if !status.is_success() {
                    anyhow::bail!(
                        "device_authorization failed ({}): {}",
                        status,
                        String::from_utf8_lossy(&bytes)
                    );
                }
                serde_json::from_slice::<DeviceCodeResp>(&bytes)
                    .context("decode device_authorization response")
            })
            .await?
    }

    /// One iteration of the polling loop. Callers should sleep at least
    /// `interval` seconds between calls; `SlowDown` instructs them to widen
    /// the gap.
    pub async fn device_flow_poll(
        &self,
        well_known: &WellKnown,
        device_code: &str,
    ) -> Result<PollResult> {
        let body = url::form_urlencoded::Serializer::new(String::new())
            .append_pair("grant_type", "urn:ietf:params:oauth:grant-type:device_code")
            .append_pair("device_code", device_code)
            .append_pair("client_id", &well_known.desktop_official_client_id)
            .finish();
        let endpoint = well_known.token_endpoint.clone();
        let http = self.http.clone();
        self.tokio_handle
            .spawn(async move {
                let resp = http
                    .post(&endpoint)
                    .header(
                        reqwest::header::CONTENT_TYPE,
                        "application/x-www-form-urlencoded",
                    )
                    .body(body)
                    .send()
                    .await?;
                let status = resp.status();
                let bytes = resp.bytes().await?;
                if status.is_success() {
                    let tokens: Tokens =
                        serde_json::from_slice(&bytes).context("decode token response")?;
                    return anyhow::Ok(PollResult::Issued(tokens));
                }
                let err: OAuthErr =
                    serde_json::from_slice(&bytes).context("decode OAuth error envelope")?;
                Ok(match err.error.as_str() {
                    "authorization_pending" => PollResult::Pending,
                    "slow_down" => PollResult::SlowDown,
                    "access_denied" => PollResult::Denied,
                    "expired_token" => PollResult::Expired,
                    other => {
                        anyhow::bail!(
                            "device flow failed: {} ({})",
                            other,
                            err.error_description.unwrap_or_default()
                        );
                    }
                })
            })
            .await?
    }

    /// Exchange a refresh_token for a fresh access+refresh pair. RFC 6749
    /// §6 + RFC 6819 §5.2.2.3 rotation semantics — the old refresh becomes
    /// invalid the moment the new pair is returned.
    pub async fn refresh(&self, well_known: &WellKnown, refresh_token: &str) -> Result<Tokens> {
        let body = url::form_urlencoded::Serializer::new(String::new())
            .append_pair("grant_type", "refresh_token")
            .append_pair("refresh_token", refresh_token)
            .append_pair("client_id", &well_known.desktop_official_client_id)
            .finish();
        let endpoint = well_known.token_endpoint.clone();
        let http = self.http.clone();
        self.tokio_handle
            .spawn(async move {
                let resp = http
                    .post(&endpoint)
                    .header(
                        reqwest::header::CONTENT_TYPE,
                        "application/x-www-form-urlencoded",
                    )
                    .body(body)
                    .send()
                    .await?;
                let status = resp.status();
                let bytes = resp.bytes().await?;
                if status.is_success() {
                    return anyhow::Ok(serde_json::from_slice::<Tokens>(&bytes)?);
                }
                let err: OAuthErr = serde_json::from_slice(&bytes)?;
                anyhow::bail!(
                    "refresh failed: {} ({})",
                    err.error,
                    err.error_description.unwrap_or_default()
                );
            })
            .await?
    }

    /// POST `/api/v1/oauth/revoke` (RFC 7009). Fire-and-forget for our
    /// purposes — the server treats unknown tokens as already revoked.
    pub async fn revoke(&self, well_known: &WellKnown, token: &str) -> Result<()> {
        let body = url::form_urlencoded::Serializer::new(String::new())
            .append_pair("token", token)
            .finish();
        let endpoint = well_known.revocation_endpoint.clone();
        let http = self.http.clone();
        self.tokio_handle
            .spawn(async move {
                http.post(&endpoint)
                    .header(
                        reqwest::header::CONTENT_TYPE,
                        "application/x-www-form-urlencoded",
                    )
                    .body(body)
                    .send()
                    .await?;
                anyhow::Ok(())
            })
            .await?
    }

    /// GET `/api/v1/auth/me` with the given Bearer token.
    pub async fn current_user(&self, access_token: &str) -> Result<Me> {
        let url = self.server.join("/api/v1/auth/me");
        let http = self.http.clone();
        let access_token = access_token.to_string();
        self.tokio_handle
            .spawn(async move {
                let resp = http
                    .get(&url)
                    .bearer_auth(access_token)
                    .send()
                    .await?
                    .error_for_status()?;
                anyhow::Ok(resp.json::<Me>().await?)
            })
            .await?
    }

    /// Persist tokens to the OS credentials store keyed by the server URL.
    /// `username` is what the user will see ("alice" et al.); the password
    /// slot stores a JSON-encoded `StoredCreds`.
    pub async fn save_credentials(
        &self,
        cx: &AsyncApp,
        username: &str,
        tokens: &Tokens,
        expires_at_unix: i64,
    ) -> Result<()> {
        let stored = StoredCreds {
            access_token: tokens.access_token.clone(),
            refresh_token: tokens.refresh_token.clone(),
            expires_at_unix,
            scope: tokens.scope.clone(),
            username: username.to_string(),
        };
        let payload = serde_json::to_vec(&stored)?;
        let url = self.server.as_str().to_string();
        self.creds
            .write_credentials(&url, username, &payload, cx)
            .await
    }

    /// Read previously persisted credentials. Returns None when no row
    /// exists. JSON decode errors are surfaced because they imply a
    /// keychain entry from a future Esperanta we don't know how to read —
    /// the caller should treat that as "not signed in" and let the user
    /// re-auth.
    pub async fn load_credentials(&self, cx: &AsyncApp) -> Result<Option<StoredCreds>> {
        let url = self.server.as_str().to_string();
        let read = self.creds.read_credentials(&url, cx).await?;
        let Some((_, bytes)) = read else {
            return Ok(None);
        };
        let stored: StoredCreds = serde_json::from_slice(&bytes)?;
        Ok(Some(stored))
    }

    /// Clear persisted credentials. Used on sign-out and when the refresh
    /// chain expires.
    pub async fn clear_credentials(&self, cx: &AsyncApp) -> Result<()> {
        let url = self.server.as_str().to_string();
        self.creds.delete_credentials(&url, cx).await
    }
}

#[derive(Debug, Deserialize)]
struct OAuthErr {
    error: String,
    #[serde(default)]
    error_description: Option<String>,
}

fn user_agent() -> String {
    // Mirrors the rest of Esperanta — see `paths::APP_NAME` and the
    // documented naming convention. Token-safe ASCII form here is
    // intentional: HTTP/proxy stacks do not all tolerate apostrophes or the
    // U+00B7 middle dot in User-Agent.
    format!(
        "Kaltsit-Esperanta/{} (ama10; +https://github.com/zixiao-labs/Kaltsit-Esperanta)",
        env!("CARGO_PKG_VERSION")
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ua_includes_version() {
        let ua = user_agent();
        assert!(ua.starts_with("Kaltsit-Esperanta/"));
        assert!(ua.contains("ama10"));
    }
}
