//! In-thread CEF host state (runs exclusively on the async host thread).

use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};

use anyhow::{Result, anyhow};
use async_channel::Sender;
use async_host_runtime::{HostCommand, HostLifecycleCell, handle_command_result};

use crate::ffi::CefFunctionTable;
use crate::osr::{self, SharedPaintFrame};

/// Opaque browser identifier stable across the async boundary.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct BrowserId(pub u64);

/// CEF log severity (matches CEF's `cef_log_severity_t` values).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
#[repr(i32)]
pub enum CefLogSeverity {
    #[default]
    Default = 0,
    Verbose = 1,
    Info = 2,
    Warning = 3,
    Error = 4,
    Fatal = 5,
    Disable = 99,
}

/// Subset of CEF settings needed before full OSR wiring.
#[derive(Clone, Debug)]
pub struct CefSettings {
    pub log_severity: CefLogSeverity,
    pub remote_debugging_port: u16,
    pub cache_path: String,
    pub user_agent: String,
    /// When true, never call into real libcef even if loaded (tests / CI).
    pub force_stub: bool,
}

impl Default for CefSettings {
    fn default() -> Self {
        Self {
            log_severity: CefLogSeverity::Default,
            remote_debugging_port: 0,
            cache_path: String::new(),
            user_agent: String::new(),
            force_stub: cfg!(feature = "force-stub"),
        }
    }
}

#[derive(Clone, Debug)]
pub struct CefBrowserSettings {
    pub windowless_rendering_enabled: bool,
    pub javascript_access_clipboard: bool,
}

impl Default for CefBrowserSettings {
    fn default() -> Self {
        Self {
            windowless_rendering_enabled: true,
            javascript_access_clipboard: false,
        }
    }
}

/// BGRA (or platform paint) buffer emitted by OSR (filled in later PRs).
#[derive(Clone, Debug)]
pub struct PaintBuffer {
    pub browser_id: BrowserId,
    pub width: u32,
    pub height: u32,
    pub bytes: Vec<u8>,
}

#[derive(Debug)]
pub enum CefHostCommand {
    CreateBrowser {
        url: String,
        settings: CefBrowserSettings,
        reply: Sender<Result<BrowserId>>,
    },
    CloseBrowser {
        id: BrowserId,
        reply: Sender<Result<()>>,
    },
    Navigate {
        id: BrowserId,
        url: String,
        reply: Sender<Result<()>>,
    },
    ExecuteJavaScript {
        id: BrowserId,
        code: String,
        reply: Sender<Result<()>>,
    },
    /// Pump the CEF message loop once (external pump mode).
    DoMessageLoopWork,
}

#[derive(Debug)]
pub enum CefHostEvent {
    BrowserCreated(BrowserId),
    BrowserClosed(BrowserId),
    LoadStart {
        id: BrowserId,
        url: String,
    },
    LoadEnd {
        id: BrowserId,
        http_status: i32,
    },
    LoadError {
        id: BrowserId,
        message: String,
    },
    /// Raw BGRA paint from CEF.
    Paint(PaintBuffer),
    /// Sendable frame for the UI thread (`to_cv_pixel_buffer` on macOS).
    Frame(SharedPaintFrame),
    TitleChanged {
        id: BrowserId,
        title: String,
    },
    AddressChanged {
        id: BrowserId,
        url: String,
    },
}

struct BrowserSlot {
    url: String,
}

/// Concrete host that owns either a real function table or a stub.
pub struct CefHost {
    settings: CefSettings,
    table: Option<CefFunctionTable>,
    browsers: HashMap<BrowserId, BrowserSlot>,
    next_id: AtomicU64,
    stub: bool,
}

impl CefHost {
    pub fn new_stub(settings: CefSettings) -> Self {
        Self {
            settings,
            table: None,
            browsers: HashMap::new(),
            next_id: AtomicU64::new(1),
            stub: true,
        }
    }

    pub fn new_from_table(settings: CefSettings, table: CefFunctionTable) -> Result<Self> {
        // Full `cef_initialize` requires CEF main-args / app structure that we
        // wire with OSR in the next stack layer. For this layer we only retain
        // the loaded table to prove dynamic linking succeeded.
        let _ = &table.cef_initialize;
        Ok(Self {
            settings,
            table: Some(table),
            browsers: HashMap::new(),
            next_id: AtomicU64::new(1),
            stub: false,
        })
    }

    pub fn is_stub(&self) -> bool {
        self.stub
    }

    fn alloc_id(&self) -> BrowserId {
        BrowserId(self.next_id.fetch_add(1, Ordering::SeqCst))
    }

    pub fn create_browser(
        &mut self,
        url: String,
        _settings: CefBrowserSettings,
        events: &Sender<CefHostEvent>,
    ) -> Result<BrowserId> {
        let id = self.alloc_id();
        self.browsers.insert(id, BrowserSlot { url: url.clone() });
        let _ = events.send_blocking(CefHostEvent::BrowserCreated(id));
        let _ = events.send_blocking(CefHostEvent::AddressChanged {
            id,
            url: url.clone(),
        });
        let _ = events.send_blocking(CefHostEvent::LoadStart { id, url });
        let _ = events.send_blocking(CefHostEvent::LoadEnd {
            id,
            http_status: 200,
        });
        // Stub OSR: emit a solid frame so UI can exercise the surface path.
        if self.stub {
            self.emit_stub_frame(id, events);
        }
        Ok(id)
    }

    pub fn close_browser(&mut self, id: BrowserId, events: &Sender<CefHostEvent>) -> Result<()> {
        if self.browsers.remove(&id).is_none() {
            return Err(anyhow!("unknown browser {id:?}"));
        }
        let _ = events.send_blocking(CefHostEvent::BrowserClosed(id));
        Ok(())
    }

    pub fn navigate(
        &mut self,
        id: BrowserId,
        url: String,
        events: &Sender<CefHostEvent>,
    ) -> Result<()> {
        let slot = self
            .browsers
            .get_mut(&id)
            .ok_or_else(|| anyhow!("unknown browser {id:?}"))?;
        slot.url = url.clone();
        let _ = events.send_blocking(CefHostEvent::AddressChanged {
            id,
            url: url.clone(),
        });
        let _ = events.send_blocking(CefHostEvent::LoadStart { id, url });
        let _ = events.send_blocking(CefHostEvent::LoadEnd {
            id,
            http_status: 200,
        });
        if self.stub {
            self.emit_stub_frame(id, events);
        }
        Ok(())
    }

    pub fn execute_javascript(&self, id: BrowserId, _code: &str) -> Result<()> {
        if !self.browsers.contains_key(&id) {
            return Err(anyhow!("unknown browser {id:?}"));
        }
        Ok(())
    }

    pub fn do_message_loop_work(&self) {
        if let Some(table) = &self.table {
            unsafe {
                (table.cef_do_message_loop_work)();
            }
        }
    }

    /// Push a CEF OSR paint as sendable shared frames for the UI thread.
    pub fn push_paint(&self, paint: PaintBuffer, events: &Sender<CefHostEvent>) -> Result<()> {
        let frame = osr::paint_buffer_to_shared_frame(&paint)?;
        let _ = events.send_blocking(CefHostEvent::Paint(paint));
        let _ = events.send_blocking(CefHostEvent::Frame(frame));
        Ok(())
    }

    fn emit_stub_frame(&self, id: BrowserId, events: &Sender<CefHostEvent>) {
        let paint = osr::solid_color_paint(id, 16, 16, [40, 40, 40, 255]);
        if let Err(error) = self.push_paint(paint, events) {
            log::warn!("failed to emit stub OSR frame: {error:#}");
        }
    }
}

impl Drop for CefHost {
    fn drop(&mut self) {
        if let Some(table) = self.table.take() {
            // Only shut down if we had previously initialized; today we retain
            // the table without calling cef_initialize, so skip cef_shutdown.
            let _ = table;
            let _ = &self.settings;
        }
    }
}

pub(crate) fn run_host_loop(
    mut host: CefHost,
    command_rx: async_channel::Receiver<HostCommand<CefHostCommand>>,
    event_tx: Sender<CefHostEvent>,
    _lifecycle: HostLifecycleCell,
) {
    while handle_command_result(command_rx.recv_blocking(), |command| {
        dispatch(&mut host, command, &event_tx)
    }) {}
}

fn dispatch(
    host: &mut CefHost,
    command: CefHostCommand,
    events: &Sender<CefHostEvent>,
) -> Result<()> {
    match command {
        CefHostCommand::CreateBrowser {
            url,
            settings,
            reply,
        } => {
            let result = host.create_browser(url, settings, events);
            let _ = reply.send_blocking(result);
        }
        CefHostCommand::CloseBrowser { id, reply } => {
            let result = host.close_browser(id, events);
            let _ = reply.send_blocking(result);
        }
        CefHostCommand::Navigate { id, url, reply } => {
            let result = host.navigate(id, url, events);
            let _ = reply.send_blocking(result);
        }
        CefHostCommand::ExecuteJavaScript { id, code, reply } => {
            let result = host.execute_javascript(id, &code);
            let _ = reply.send_blocking(result);
        }
        CefHostCommand::DoMessageLoopWork => {
            host.do_message_loop_work();
        }
    }
    Ok(())
}
