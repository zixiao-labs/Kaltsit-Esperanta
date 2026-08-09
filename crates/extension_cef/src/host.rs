//! In-thread CEF host state (runs exclusively on the async host thread).

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

use anyhow::{Result, anyhow};
use async_channel::Sender;
use async_host_runtime::{HostCommand, HostLifecycleCell, handle_command_result};

use crate::ffi::CefFunctionTable;
use crate::osr::{self, SharedPaintFrame};
use crate::osr_runtime::CefRuntime;

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
    /// View size in DIP (logical) pixels.
    pub view_width: i32,
    pub view_height: i32,
    /// Device pixel ratio for OSR (`GetScreenInfo.device_scale_factor`).
    pub device_scale_factor: f32,
}

impl Default for CefBrowserSettings {
    fn default() -> Self {
        Self {
            windowless_rendering_enabled: true,
            javascript_access_clipboard: false,
            view_width: 960,
            view_height: 540,
            device_scale_factor: 1.0,
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
    ResizeBrowser {
        id: BrowserId,
        view_width: i32,
        view_height: i32,
        device_scale_factor: f32,
    },
    SetFocus {
        id: BrowserId,
        focused: bool,
    },
    SendMouseClick {
        id: BrowserId,
        x: f32,
        y: f32,
        button: MouseButtonKind,
        mouse_up: bool,
        click_count: u32,
        modifiers: u32,
    },
    SendMouseMove {
        id: BrowserId,
        x: f32,
        y: f32,
        mouse_leave: bool,
        modifiers: u32,
    },
    SendMouseWheel {
        id: BrowserId,
        x: f32,
        y: f32,
        delta_x: f32,
        delta_y: f32,
        modifiers: u32,
    },
    SendKeyEvent {
        id: BrowserId,
        event: KeyEventPayload,
    },
    /// Pump the CEF message loop once (external pump mode).
    DoMessageLoopWork,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MouseButtonKind {
    Left,
    Middle,
    Right,
}

#[derive(Clone, Debug)]
pub struct KeyEventPayload {
    pub key_down: bool,
    pub characters: String,
    pub keycode: u32,
    pub modifiers: u32,
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
    runtime: Option<CefRuntime>,
    browsers: HashMap<BrowserId, BrowserSlot>,
    next_id: AtomicU64,
    stub: bool,
    osr_ready: bool,
}

impl CefHost {
    pub fn new_stub(settings: CefSettings) -> Self {
        Self {
            settings,
            table: None,
            runtime: None,
            browsers: HashMap::new(),
            next_id: AtomicU64::new(1),
            stub: true,
            osr_ready: false,
        }
    }

    pub fn new_from_table(
        settings: CefSettings,
        table: CefFunctionTable,
        library_path: PathBuf,
    ) -> Result<Self> {
        let mut host = Self {
            settings: settings.clone(),
            table: Some(table),
            runtime: None,
            browsers: HashMap::new(),
            next_id: AtomicU64::new(1),
            stub: false,
            osr_ready: false,
        };

        match host.try_initialize_runtime(&settings, &library_path) {
            Ok(()) => {
                host.osr_ready = true;
            }
            Err(error) => {
                log::warn!("CEF OSR initialize failed, using placeholder paint: {error:#}");
            }
        }

        Ok(host)
    }

    fn try_initialize_runtime(
        &mut self,
        settings: &CefSettings,
        library_path: &PathBuf,
    ) -> Result<()> {
        let table = self
            .table
            .as_ref()
            .ok_or_else(|| anyhow!("missing CEF function table"))?;
        let runtime = CefRuntime::initialize(settings, table, library_path)?;
        self.runtime = Some(runtime);
        Ok(())
    }

    pub fn is_stub(&self) -> bool {
        self.stub
    }

    pub fn is_osr_ready(&self) -> bool {
        self.osr_ready
    }

    fn alloc_id(&self) -> BrowserId {
        BrowserId(self.next_id.fetch_add(1, Ordering::SeqCst))
    }

    pub fn create_browser(
        &mut self,
        url: String,
        settings: CefBrowserSettings,
        events: &Sender<CefHostEvent>,
    ) -> Result<BrowserId> {
        let id = self.alloc_id();
        self.browsers.insert(id, BrowserSlot { url: url.clone() });
        let _ = events.send_blocking(CefHostEvent::BrowserCreated(id));
        let _ = events.send_blocking(CefHostEvent::AddressChanged {
            id,
            url: url.clone(),
        });

        if self.osr_ready {
            let table = self
                .table
                .as_ref()
                .ok_or_else(|| anyhow!("missing CEF function table"))?;
            let runtime = self
                .runtime
                .as_mut()
                .ok_or_else(|| anyhow!("missing CEF runtime"))?;
            runtime.create_browser(
                &url,
                id,
                settings.view_width.max(1),
                settings.view_height.max(1),
                settings.device_scale_factor,
                table,
                events,
            )?;
            return Ok(id);
        }

        let _ = events.send_blocking(CefHostEvent::LoadStart { id, url });
        self.emit_stub_frame(id, events);
        let _ = events.send_blocking(CefHostEvent::LoadError {
            id,
            message: PLACEHOLDER_OSR_STATUS.to_owned(),
        });
        Ok(id)
    }

    pub fn close_browser(&mut self, id: BrowserId, events: &Sender<CefHostEvent>) -> Result<()> {
        if self.browsers.remove(&id).is_none() {
            return Err(anyhow!("unknown browser {id:?}"));
        }
        if self.osr_ready {
            if let Some(runtime) = self.runtime.as_mut() {
                if let Err(error) = runtime.close_browser(id) {
                    log::warn!("CEF close_browser failed for {id:?}: {error:#}");
                }
            }
            return Ok(());
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

        if self.osr_ready {
            let table = self
                .table
                .as_ref()
                .ok_or_else(|| anyhow!("missing CEF function table"))?;
            let runtime = self
                .runtime
                .as_ref()
                .ok_or_else(|| anyhow!("missing CEF runtime"))?;
            runtime.navigate(id, &url, table)?;
            return Ok(());
        }

        let _ = events.send_blocking(CefHostEvent::LoadStart { id, url });
        self.emit_stub_frame(id, events);
        let _ = events.send_blocking(CefHostEvent::LoadError {
            id,
            message: PLACEHOLDER_OSR_STATUS.to_owned(),
        });
        Ok(())
    }

    pub fn execute_javascript(&self, id: BrowserId, _code: &str) -> Result<()> {
        if !self.browsers.contains_key(&id) {
            return Err(anyhow!("unknown browser {id:?}"));
        }
        Ok(())
    }

    pub fn resize_browser(
        &self,
        id: BrowserId,
        view_width: i32,
        view_height: i32,
        device_scale_factor: f32,
    ) -> Result<()> {
        if !self.browsers.contains_key(&id) {
            return Err(anyhow!("unknown browser {id:?}"));
        }
        if let Some(runtime) = self.runtime.as_ref() {
            runtime.resize_browser(id, view_width, view_height, device_scale_factor)?;
        }
        Ok(())
    }

    pub fn set_focus(&self, id: BrowserId, focused: bool) -> Result<()> {
        if !self.browsers.contains_key(&id) {
            return Err(anyhow!("unknown browser {id:?}"));
        }
        if let Some(runtime) = self.runtime.as_ref() {
            runtime.set_focus(id, focused)?;
        }
        Ok(())
    }

    pub fn send_mouse_click(
        &self,
        id: BrowserId,
        x: f32,
        y: f32,
        button: MouseButtonKind,
        mouse_up: bool,
        click_count: u32,
        modifiers: u32,
    ) -> Result<()> {
        if !self.browsers.contains_key(&id) {
            return Err(anyhow!("unknown browser {id:?}"));
        }
        if let Some(runtime) = self.runtime.as_ref() {
            runtime.send_mouse_click(id, x, y, button, mouse_up, click_count, modifiers)?;
        }
        Ok(())
    }

    pub fn send_mouse_move(
        &self,
        id: BrowserId,
        x: f32,
        y: f32,
        mouse_leave: bool,
        modifiers: u32,
    ) -> Result<()> {
        if !self.browsers.contains_key(&id) {
            return Err(anyhow!("unknown browser {id:?}"));
        }
        if let Some(runtime) = self.runtime.as_ref() {
            runtime.send_mouse_move(id, x, y, mouse_leave, modifiers)?;
        }
        Ok(())
    }

    pub fn send_mouse_wheel(
        &self,
        id: BrowserId,
        x: f32,
        y: f32,
        delta_x: f32,
        delta_y: f32,
        modifiers: u32,
    ) -> Result<()> {
        if !self.browsers.contains_key(&id) {
            return Err(anyhow!("unknown browser {id:?}"));
        }
        if let Some(runtime) = self.runtime.as_ref() {
            runtime.send_mouse_wheel(id, x, y, delta_x, delta_y, modifiers)?;
        }
        Ok(())
    }

    pub fn send_key_event(&self, id: BrowserId, event: &KeyEventPayload) -> Result<()> {
        if !self.browsers.contains_key(&id) {
            return Err(anyhow!("unknown browser {id:?}"));
        }
        if let Some(runtime) = self.runtime.as_ref() {
            runtime.send_key_event(id, event)?;
        }
        Ok(())
    }

    pub fn do_message_loop_work(&self) {
        if let (Some(table), Some(runtime)) = (self.table.as_ref(), self.runtime.as_ref()) {
            runtime.do_message_loop_work(table);
        } else if let Some(table) = &self.table {
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
        // BGRA slate-blue placeholder until CefRenderHandler OnPaint is wired.
        let paint = osr::solid_color_paint(id, 960, 540, [160, 96, 56, 255]);
        if let Err(error) = self.push_paint(paint, events) {
            log::warn!("failed to emit stub OSR frame: {error:#}");
        }
    }
}

/// Sentinel LoadError message when only the CPU placeholder paint is available.
pub const PLACEHOLDER_OSR_STATUS: &str = "PLACEHOLDER_OSR_NOT_WIRED";

impl Drop for CefHost {
    fn drop(&mut self) {
        if let (Some(table), Some(runtime)) = (self.table.as_ref(), self.runtime.as_mut()) {
            runtime.shutdown(table);
        }
        let _ = self.table.take();
        let _ = self.runtime.take();
        let _ = &self.settings;
    }
}

pub(crate) fn run_host_loop(
    mut host: CefHost,
    command_rx: async_channel::Receiver<HostCommand<CefHostCommand>>,
    event_tx: Sender<CefHostEvent>,
    _lifecycle: HostLifecycleCell,
) {
    loop {
        match command_rx.try_recv() {
            Ok(command) => {
                if !handle_command_result(Ok(command), |command| {
                    dispatch(&mut host, command, &event_tx)
                }) {
                    break;
                }
            }
            Err(async_channel::TryRecvError::Closed) => break,
            Err(async_channel::TryRecvError::Empty) => {}
        }
        host.do_message_loop_work();
        std::thread::sleep(Duration::from_millis(5));
    }
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
        CefHostCommand::ResizeBrowser {
            id,
            view_width,
            view_height,
            device_scale_factor,
        } => {
            host.resize_browser(id, view_width, view_height, device_scale_factor)?;
        }
        CefHostCommand::SetFocus { id, focused } => {
            host.set_focus(id, focused)?;
        }
        CefHostCommand::SendMouseClick {
            id,
            x,
            y,
            button,
            mouse_up,
            click_count,
            modifiers,
        } => {
            host.send_mouse_click(id, x, y, button, mouse_up, click_count, modifiers)?;
        }
        CefHostCommand::SendMouseMove {
            id,
            x,
            y,
            mouse_leave,
            modifiers,
        } => {
            host.send_mouse_move(id, x, y, mouse_leave, modifiers)?;
        }
        CefHostCommand::SendMouseWheel {
            id,
            x,
            y,
            delta_x,
            delta_y,
            modifiers,
        } => {
            host.send_mouse_wheel(id, x, y, delta_x, delta_y, modifiers)?;
        }
        CefHostCommand::SendKeyEvent { id, event } => {
            host.send_key_event(id, &event)?;
        }
        CefHostCommand::DoMessageLoopWork => {
            host.do_message_loop_work();
        }
    }
    Ok(())
}
