//! GPUI-facing async CEF session (load + command broker).

use std::path::{Path, PathBuf};

use anyhow::{Result, anyhow};
use async_channel::bounded;
use async_host_runtime::{HostLifecycleCell, HostSession};

use crate::default_libcef_candidates;
use crate::ffi;
use crate::host::{
    BrowserId, CefBrowserSettings, CefHost, CefHostCommand, CefHostEvent, CefSettings,
    run_host_loop,
};

/// Asynchronously loaded CEF host. Safe to construct on the GPUI thread.
pub struct AsyncCefHost {
    session: HostSession<CefHostCommand, CefHostEvent>,
}

impl AsyncCefHost {
    /// Spawn a stub host that never touches libcef (always succeeds).
    pub fn spawn_stub(mut settings: CefSettings) -> Self {
        settings.force_stub = true;
        Self::spawn_inner(settings, None)
    }

    /// Probe default paths and load libcef on a background thread.
    pub fn spawn(settings: CefSettings) -> Self {
        Self::spawn_inner(settings, None)
    }

    /// Load a specific library path on a background thread.
    pub fn spawn_with_library_path(settings: CefSettings, library_path: impl Into<String>) -> Self {
        Self::spawn_inner(settings, Some(library_path.into()))
    }

    fn spawn_inner(settings: CefSettings, library_path: Option<String>) -> Self {
        let session = HostSession::spawn_thread(
            "cef-host",
            move || load_host(settings, library_path),
            run_host_loop,
        );
        Self { session }
    }

    pub fn lifecycle(&self) -> &HostLifecycleCell {
        self.session.lifecycle()
    }

    pub fn try_recv_event(&self) -> Result<CefHostEvent, async_channel::TryRecvError> {
        self.session.try_recv_event()
    }

    pub fn event_receiver(&self) -> async_channel::Receiver<CefHostEvent> {
        self.session.clone_event_receiver()
    }

    pub async fn create_browser(
        &self,
        url: impl Into<String>,
        settings: Option<CefBrowserSettings>,
    ) -> Result<BrowserId> {
        let (reply_tx, reply_rx) = bounded(1);
        self.session
            .send(CefHostCommand::CreateBrowser {
                url: url.into(),
                settings: settings.unwrap_or_default(),
                reply: reply_tx,
            })
            .await?;
        reply_rx
            .recv()
            .await
            .map_err(|_| anyhow!("CEF host closed while creating browser"))?
    }

    pub async fn close_browser(&self, id: BrowserId) -> Result<()> {
        let (reply_tx, reply_rx) = bounded(1);
        self.session
            .send(CefHostCommand::CloseBrowser {
                id,
                reply: reply_tx,
            })
            .await?;
        reply_rx
            .recv()
            .await
            .map_err(|_| anyhow!("CEF host closed while closing browser"))?
    }

    pub async fn navigate(&self, id: BrowserId, url: impl Into<String>) -> Result<()> {
        let (reply_tx, reply_rx) = bounded(1);
        self.session
            .send(CefHostCommand::Navigate {
                id,
                url: url.into(),
                reply: reply_tx,
            })
            .await?;
        reply_rx
            .recv()
            .await
            .map_err(|_| anyhow!("CEF host closed while navigating"))?
    }

    pub async fn execute_javascript(&self, id: BrowserId, code: impl Into<String>) -> Result<()> {
        let (reply_tx, reply_rx) = bounded(1);
        self.session
            .send(CefHostCommand::ExecuteJavaScript {
                id,
                code: code.into(),
                reply: reply_tx,
            })
            .await?;
        reply_rx
            .recv()
            .await
            .map_err(|_| anyhow!("CEF host closed while executing JavaScript"))?
    }

    pub async fn do_message_loop_work(&self) -> Result<()> {
        self.session.send(CefHostCommand::DoMessageLoopWork).await
    }
}

fn load_host(settings: CefSettings, library_path: Option<String>) -> Result<CefHost> {
    if settings.force_stub {
        log::info!("CEF host starting in stub mode");
        return Ok(CefHost::new_stub(settings));
    }

    let path = match library_path {
        Some(path) => PathBuf::from(path),
        None => probe_libcef_path()
            .map(PathBuf::from)
            .ok_or_else(|| anyhow!("libcef not found on default search paths"))?,
    };

    log::info!("Loading libcef from {}", path.display());
    let table = ffi::try_load(&path)?;
    CefHost::new_from_table(settings, table)
}

/// Return the first existing candidate path that successfully resolves CEF symbols.
pub fn probe_libcef_path() -> Option<String> {
    for candidate in default_libcef_candidates() {
        let path = Path::new(candidate);
        // Skip bare sonames that are not absolute and do not exist as relative
        // files — still try libloading which searches the loader path.
        match ffi::try_load(path) {
            Ok(_) => return Some((*candidate).to_owned()),
            Err(error) => {
                log::debug!("CEF probe skipped {}: {error:#}", path.display());
            }
        }
    }
    None
}
