//! Git-credentials interceptor that returns Wuling OAuth tokens.
//!
//! Zed's git layer asks for credentials by spawning an external askpass
//! process: see `crates/git/src/repository.rs:888-944,2318-2469,3460` and
//! the `AskPassDelegate` in `crates/askpass/src/askpass.rs`. The delegate
//! receives a prompt string (e.g. "Password for 'https://alice@host':")
//! and must answer with the secret to feed back to git.
//!
//! We can't *register* a delegate from outside the askpass module — there's
//! no extension point. Instead, this module exposes `lookup_for_host`, a
//! synchronous query that wraps `CredentialsProvider::read_credentials`,
//! so the zed-glue layer (`crates/zed/src/zed/ama10.rs`) can construct
//! `AskPassDelegate::new` with a closure that consults us first and falls
//! through to the default prompt otherwise.
//!
//! What's intercepted:
//!
//!   - The git URL has the host of the currently-configured Wuling server.
//!   - We have non-expired stored credentials for that server.
//!
//! Anything else falls back to the existing askpass UI.

use std::sync::Arc;

use ama10::auth::{StoredCreds, WulingClient};
use anyhow::Result;
use credentials_provider::CredentialsProvider;
use gpui::AsyncApp;

use crate::settings::WulingConfig;

/// Margin before `expires_at_unix` that we treat the access_token as already
/// stale so a refresh round-trip happens before the token is actually rejected
/// upstream. 60s comfortably covers a clock skew + the worst-case discover +
/// /token RTT.
const REFRESH_LEEWAY_SECS: i64 = 60;

/// Result of trying to satisfy an askpass prompt with stored Wuling creds.
pub enum LookupResult {
    /// We have a token and it applies to this URL — return it as the password.
    UseToken(String),
    /// Either the URL doesn't match the configured Wuling host, or no token
    /// is stored. The caller should let the default askpass UI run.
    FallThrough,
}

/// Inspect `prompt_url` (typically what git would put in a Basic auth
/// header) and return a token if the URL matches the configured Wuling
/// server and we have non-expired credentials.
///
/// Called by the AskPass closure in zed::ama10. Keep it cheap — the closure
/// fires on every git operation.
pub async fn lookup_for_host(
    prompt_url: &str,
    creds: Arc<dyn CredentialsProvider>,
    cx: &AsyncApp,
) -> Result<LookupResult> {
    let config = WulingConfig::load();
    let server_host = config.server.host();

    let parsed = match url::Url::parse(prompt_url) {
        Ok(u) => u,
        Err(_) => return Ok(LookupResult::FallThrough),
    };
    let Some(host) = parsed.host_str() else {
        return Ok(LookupResult::FallThrough);
    };
    if !host.eq_ignore_ascii_case(server_host) {
        return Ok(LookupResult::FallThrough);
    }

    let server_key = config.server.as_str().to_string();
    let Some((_, bytes)) = creds.read_credentials(&server_key, cx).await? else {
        return Ok(LookupResult::FallThrough);
    };
    let stored: StoredCreds = match serde_json::from_slice(&bytes) {
        Ok(v) => v,
        Err(err) => {
            log::warn!("ama10: keychain entry for {server_key} is malformed: {err}");
            return Ok(LookupResult::FallThrough);
        }
    };
    let now = std::time::UNIX_EPOCH
        .elapsed()
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0);
    let is_expiring =
        stored.expires_at_unix > 0 && now + REFRESH_LEEWAY_SECS >= stored.expires_at_unix;
    if !is_expiring {
        return Ok(LookupResult::UseToken(stored.access_token));
    }

    // Token is expired (or close to it). Try to mint a fresh pair via the
    // refresh_token. On success, persist the new pair and return the new
    // access_token; on failure, clear the dead credentials and fall through
    // so the user is prompted to sign in again.
    let Some(refresh_token) = stored.refresh_token.as_deref() else {
        log::info!(
            "ama10: stored token expired at {} (now {}) and no refresh_token saved",
            stored.expires_at_unix,
            now
        );
        return Ok(LookupResult::FallThrough);
    };
    log::info!(
        "ama10: stored token expired at {} (now {}); attempting refresh",
        stored.expires_at_unix,
        now
    );
    match refresh_stored(&config, &stored, refresh_token, creds, cx).await {
        Ok(new_token) => Ok(LookupResult::UseToken(new_token)),
        Err(err) => {
            log::warn!("ama10: refresh failed: {err:#}");
            Ok(LookupResult::FallThrough)
        }
    }
}

async fn refresh_stored(
    config: &WulingConfig,
    stored: &StoredCreds,
    refresh_token: &str,
    creds: Arc<dyn CredentialsProvider>,
    cx: &AsyncApp,
) -> Result<String> {
    let tokio_handle = cx.update(|cx| gpui_tokio::Tokio::handle(cx));
    let client = WulingClient::new(config.server.clone(), creds, tokio_handle);
    let well_known = client.discover().await?;
    let tokens = client.refresh(&well_known, refresh_token).await?;
    let now = std::time::UNIX_EPOCH
        .elapsed()
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0);
    let expires_at = now + tokens.expires_in as i64;
    let access = tokens.access_token.clone();
    client
        .save_credentials(cx, &stored.username, &tokens, expires_at)
        .await?;
    Ok(access)
}
