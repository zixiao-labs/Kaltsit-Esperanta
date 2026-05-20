//! Sign-in flow modal for the Wuling DevOps account.
//!
//! When the user invokes `ama10::SignIn`, we spawn an async task that:
//!
//!   1. Loads the current `WulingConfig` (server URL).
//!   2. Hits `/.well-known/wuling-clients` for the discovery doc.
//!   3. Calls `device_authorization` and shows the user_code + browser URL
//!      via a `log::info!` (the proper modal is a TODO — gpui modals need a
//!      workspace handle; we keep the surface minimal until the wiring in
//!      `crates/zed/src/zed/ama10.rs` provides one).
//!   4. Polls `/token`. On success, persists tokens via the platform
//!      `CredentialsProvider` and logs the resolved username.
//!
//! The deliberately log-driven UX here is the bootstrap surface. The richer
//! GPUI modal will be layered on top in a follow-up — what matters is that
//! the protocol round-trip works and the tokens land in the keychain so the
//! AskPass interceptor can use them on the very next `git push`.

use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use ama10::auth::{PollResult, Tokens, WulingClient};
use anyhow::{Context as _, Result};
use credentials_provider::CredentialsProvider;
use gpui::{App, AsyncApp};

use crate::settings::WulingConfig;

/// Drive the full sign-in flow end-to-end. Returns the resolved username
/// when tokens are minted, or an error otherwise.
///
/// `creds_provider_factory` returns the platform credentials store. We pass
/// it in rather than capturing globally so tests can swap an in-memory
/// implementation; production wiring in zed::ama10 hands us the real one.
pub async fn run_device_flow(cx: &AsyncApp, creds: Arc<dyn CredentialsProvider>) -> Result<String> {
    let config = WulingConfig::load();
    let client = WulingClient::new(config.server.clone(), creds);

    let well_known = client.discover().await.context("discover well-known")?;
    log::info!(
        "ama10: starting device flow against {} (client_id={})",
        client.server(),
        well_known.desktop_official_client_id
    );

    let scopes = [
        "user:read",
        "repo:read",
        "issue:read",
        "mr:read",
        "git:read",
        "git:write",
    ];
    let dev = client.device_flow_begin(&well_known, &scopes).await?;
    log::info!(
        "ama10: visit {} and enter code {}",
        dev.verification_uri,
        dev.user_code
    );
    cx.background_executor()
        .timer(Duration::from_millis(1))
        .await;

    let mut interval_secs = dev.interval.max(1);
    let deadline = SystemTime::now() + Duration::from_secs(dev.expires_in);

    loop {
        if SystemTime::now() > deadline {
            anyhow::bail!("device_code expired before approval");
        }
        cx.background_executor()
            .timer(Duration::from_secs(interval_secs))
            .await;
        match client
            .device_flow_poll(&well_known, &dev.device_code)
            .await?
        {
            PollResult::Pending => continue,
            PollResult::SlowDown => {
                interval_secs = interval_secs.saturating_add(5).min(30);
                continue;
            }
            PollResult::Denied => anyhow::bail!("user denied the sign-in request"),
            PollResult::Expired => anyhow::bail!("device_code expired before approval"),
            PollResult::Issued(tokens) => {
                let username = finalise_sign_in(&client, cx, &tokens).await?;
                log::info!("ama10: signed in as {} on {}", username, client.server());
                return Ok(username);
            }
        }
    }
}

async fn finalise_sign_in(client: &WulingClient, cx: &AsyncApp, tokens: &Tokens) -> Result<String> {
    let me = client.current_user(&tokens.access_token).await?;
    let expires_at = UNIX_EPOCH
        .elapsed()
        .map(|d| d.as_secs() as i64 + tokens.expires_in as i64)
        .unwrap_or(0);
    client
        .save_credentials(cx, &me.username, tokens, expires_at)
        .await?;
    Ok(me.username)
}

/// Entry point the GPUI action wiring calls. Spawns the flow in the
/// background; logs success / failure. Callers that want to display a UI
/// modal can keep the future and await it themselves; this convenience just
/// fires-and-forgets so the command palette action returns immediately.
pub fn spawn_sign_in(cx: &mut App, creds: Arc<dyn CredentialsProvider>) {
    cx.spawn(async move |cx| match run_device_flow(cx, creds).await {
        Ok(username) => log::info!("ama10: sign-in succeeded ({username})"),
        Err(err) => log::error!("ama10: sign-in failed: {err:#}"),
    })
    .detach();
}

/// Sign the user out: revoke the access token, then delete the cached
/// credentials. Best-effort — a network failure during revoke doesn't keep
/// the credentials from being wiped locally.
pub async fn run_sign_out(cx: &AsyncApp, creds: Arc<dyn CredentialsProvider>) -> Result<()> {
    let config = WulingConfig::load();
    let client = WulingClient::new(config.server.clone(), creds);
    if let Some(stored) = client.load_credentials(cx).await? {
        if let Ok(well_known) = client.discover().await {
            if let Err(err) = client.revoke(&well_known, &stored.access_token).await {
                log::warn!("ama10: revoke failed (clearing local creds anyway): {err}");
            }
        }
    }
    client.clear_credentials(cx).await
}

pub fn spawn_sign_out(cx: &mut App, creds: Arc<dyn CredentialsProvider>) {
    cx.spawn(async move |cx| match run_sign_out(cx, creds).await {
        Ok(()) => {
            cx.update(|cx| crate::account_state::set_account(cx, None));
            log::info!("ama10: signed out");
        }
        // Local credentials may still be present — leave the UI showing
        // signed-in so the user knows to retry rather than being told it
        // worked when it didn't.
        Err(err) => log::error!("ama10: sign-out failed: {err:#}"),
    })
    .detach();
}
