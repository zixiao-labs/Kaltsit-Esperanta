//! Minimal Chrome DevTools Protocol client for the embedded browser.
//!
//! Real CEF exposes CDP over its remote-debugging endpoint. Until that socket
//! is wired, this client drives the same agent-facing operations through the
//! async CEF host (navigate / input / paint capture) and keeps console/network
//! buffers in-process for the agent tools.

use std::collections::VecDeque;
use std::sync::Arc;
use std::time::Duration;

use anyhow::{Result, anyhow};
use parking_lot::Mutex;

use crate::{
    AsyncCefHost, BrowserId, CefSettings, KeyEventPayload, MouseButtonKind, SharedPaintFrame,
};

const MAX_LOG_LINES: usize = 2_000;

/// Delay without `smol::Timer` / `async_io::Timer` (both are clippy-disallowed).
async fn delay(duration: Duration) {
    smol::unblock(move || std::thread::sleep(duration)).await;
}

#[derive(Clone, Debug)]
pub struct ConsoleMessage {
    pub level: String,
    pub text: String,
}

#[derive(Clone, Debug)]
pub struct NetworkEntry {
    pub method: String,
    pub url: String,
    pub status: Option<u16>,
}

/// Per-tab CDP-facing session bound to one [`AsyncCefHost`] browser.
pub struct CdpSession {
    host: Arc<AsyncCefHost>,
    browser_id: BrowserId,
    console: Mutex<VecDeque<ConsoleMessage>>,
    network: Mutex<VecDeque<NetworkEntry>>,
    current_url: Mutex<String>,
}

impl CdpSession {
    pub async fn create(initial_url: &str) -> Result<Arc<Self>> {
        // Prefer a managed/system libcef (same policy as BrowserView); fall back
        // to stub so CI and fresh installs stay fail-soft.
        let host = Arc::new(match crate::probe_libcef_path() {
            Some(path) => AsyncCefHost::spawn_with_library_path(CefSettings::default(), path),
            None => AsyncCefHost::spawn_stub(CefSettings::default()),
        });
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
        while !host.lifecycle().get().is_ready() {
            if host.lifecycle().get().is_failed() {
                return Err(anyhow!(
                    "browser host failed: {}",
                    host.lifecycle().failure_message().unwrap_or_default()
                ));
            }
            if std::time::Instant::now() > deadline {
                return Err(anyhow!("timed out waiting for browser host"));
            }
            delay(Duration::from_millis(10)).await;
        }

        let browser_id = host.create_browser(initial_url, None).await?;
        Ok(Arc::new(Self {
            host,
            browser_id,
            console: Mutex::new(VecDeque::new()),
            network: Mutex::new(VecDeque::new()),
            current_url: Mutex::new(initial_url.to_owned()),
        }))
    }

    pub fn browser_id(&self) -> BrowserId {
        self.browser_id
    }

    pub fn current_url(&self) -> String {
        self.current_url.lock().clone()
    }

    pub async fn navigate(&self, url: &str) -> Result<()> {
        self.host.navigate(self.browser_id, url).await?;
        *self.current_url.lock() = url.to_owned();
        self.push_network("GET", url, Some(200));
        Ok(())
    }

    pub fn click(&self, x: f32, y: f32) -> Result<()> {
        self.host.send_mouse_click_blocking(
            self.browser_id,
            x,
            y,
            MouseButtonKind::Left,
            false,
            1,
        )?;
        self.host.send_mouse_click_blocking(
            self.browser_id,
            x,
            y,
            MouseButtonKind::Left,
            true,
            1,
        )?;
        Ok(())
    }

    pub fn type_text(&self, text: &str) -> Result<()> {
        for ch in text.chars() {
            let characters = ch.to_string();
            self.host.send_key_event_blocking(
                self.browser_id,
                KeyEventPayload {
                    key_down: true,
                    characters: characters.clone(),
                    keycode: 0,
                    modifiers: 0,
                },
            )?;
            self.host.send_key_event_blocking(
                self.browser_id,
                KeyEventPayload {
                    key_down: false,
                    characters,
                    keycode: 0,
                    modifiers: 0,
                },
            )?;
        }
        Ok(())
    }

    pub fn scroll(&self, delta_x: f32, delta_y: f32) -> Result<()> {
        self.host
            .send_mouse_wheel_blocking(self.browser_id, 0., 0., delta_x, delta_y)
    }

    pub async fn screenshot(&self) -> Result<SharedPaintFrame> {
        // Drain pending events for a Frame; stub hosts emit one on create/navigate.
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(2);
        loop {
            while let Ok(event) = self.host.try_recv_event() {
                if let crate::CefHostEvent::Frame(frame) = event {
                    return Ok(frame);
                }
            }
            if std::time::Instant::now() > deadline {
                return Err(anyhow!("no OSR frame available for screenshot"));
            }
            delay(Duration::from_millis(10)).await;
        }
    }

    pub fn console_messages(&self) -> Vec<ConsoleMessage> {
        self.console.lock().iter().cloned().collect()
    }

    pub fn network_entries(&self) -> Vec<NetworkEntry> {
        self.network.lock().iter().cloned().collect()
    }

    pub fn push_console(&self, level: impl Into<String>, text: impl Into<String>) {
        let mut console = self.console.lock();
        console.push_back(ConsoleMessage {
            level: level.into(),
            text: text.into(),
        });
        while console.len() > MAX_LOG_LINES {
            console.pop_front();
        }
    }

    fn push_network(&self, method: &str, url: &str, status: Option<u16>) {
        let mut network = self.network.lock();
        network.push_back(NetworkEntry {
            method: method.to_owned(),
            url: url.to_owned(),
            status,
        });
        while network.len() > MAX_LOG_LINES {
            network.pop_front();
        }
    }

    /// CDP-style DOM probe used by Design Mode (stub returns coordinates hit).
    pub fn node_for_location(&self, x: f32, y: f32) -> DesignNodeInfo {
        DesignNodeInfo {
            xpath: format!("//*[@data-stub-x='{x}'][@data-stub-y='{y}']"),
            tag: "div".into(),
            attributes: vec![("data-design-stub".into(), "true".into())],
            computed_style_summary: "display:block".into(),
            x,
            y,
        }
    }
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct DesignNodeInfo {
    pub xpath: String,
    pub tag: String,
    pub attributes: Vec<(String, String)>,
    pub computed_style_summary: String,
    pub x: f32,
    pub y: f32,
}
