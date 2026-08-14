//! CEF off-screen rendering runtime (macOS).
//!
//! CEF client callbacks are `unsafe extern "C"` entry points that only manipulate
//! raw CEF pointers; keep the lint allowed so the FFI surface stays readable.
#![allow(unsafe_op_in_unsafe_fn)]

use std::path::Path;

use anyhow::{Result, anyhow};
use async_channel::Sender;

use crate::ffi::CefFunctionTable;
use crate::host::{BrowserId, CefHostEvent, CefSettings, KeyEventPayload, MouseButtonKind};

#[cfg(target_os = "macos")]
use std::collections::HashMap;
#[cfg(target_os = "macos")]
use std::ffi::CString;
#[cfg(target_os = "macos")]
use std::mem::offset_of;
#[cfg(target_os = "macos")]
use std::path::PathBuf;
#[cfg(target_os = "macos")]
use std::sync::atomic::{AtomicI32, AtomicU32, AtomicUsize, Ordering};

#[cfg(target_os = "macos")]
use anyhow::{Context as _, bail};

#[cfg(target_os = "macos")]
use crate::cef_sys::{
    cef_app_t, cef_base_ref_counted_t, cef_browser_settings_t, cef_browser_t, cef_client_t,
    cef_color_t, cef_event_flags_t_EVENTFLAG_LEFT_MOUSE_BUTTON,
    cef_event_flags_t_EVENTFLAG_MIDDLE_MOUSE_BUTTON,
    cef_event_flags_t_EVENTFLAG_RIGHT_MOUSE_BUTTON, cef_frame_t, cef_life_span_handler_t,
    cef_load_handler_t, cef_log_severity_t, cef_main_args_t, cef_mouse_button_type_t_MBT_LEFT,
    cef_mouse_button_type_t_MBT_MIDDLE, cef_mouse_button_type_t_MBT_RIGHT, cef_mouse_event_t,
    cef_paint_element_type_t, cef_paint_element_type_t_PET_VIEW, cef_rect_t, cef_render_handler_t,
    cef_screen_info_t, cef_settings_t, cef_state_t_STATE_ENABLED, cef_string_t, cef_window_info_t,
};
#[cfg(target_os = "macos")]
use crate::host::PaintBuffer;
#[cfg(target_os = "macos")]
use crate::managed::managed_cef_root;
#[cfg(target_os = "macos")]
use crate::osr;

#[cfg(not(target_os = "macos"))]
pub struct CefRuntime;

#[cfg(not(target_os = "macos"))]
impl CefRuntime {
    pub fn initialize(
        _settings: &CefSettings,
        _table: &CefFunctionTable,
        _library_path: &Path,
    ) -> Result<Self> {
        Err(anyhow!("OSR only on macOS"))
    }

    pub fn create_browser(
        &mut self,
        _url: &str,
        _browser_id: BrowserId,
        _view_width: i32,
        _view_height: i32,
        _device_scale_factor: f32,
        _table: &CefFunctionTable,
        _events: &Sender<CefHostEvent>,
    ) -> Result<()> {
        Err(anyhow!("OSR only on macOS"))
    }

    pub fn navigate(
        &self,
        _browser_id: BrowserId,
        _url: &str,
        _table: &CefFunctionTable,
    ) -> Result<()> {
        Err(anyhow!("OSR only on macOS"))
    }

    pub fn close_browser(&mut self, _browser_id: BrowserId) -> Result<()> {
        Err(anyhow!("OSR only on macOS"))
    }

    pub fn resize_browser(
        &self,
        _browser_id: BrowserId,
        _view_width: i32,
        _view_height: i32,
        _device_scale_factor: f32,
    ) -> Result<()> {
        Err(anyhow!("OSR only on macOS"))
    }

    pub fn send_mouse_click(
        &self,
        _browser_id: BrowserId,
        _x: f32,
        _y: f32,
        _button: MouseButtonKind,
        _mouse_up: bool,
        _click_count: u32,
        _modifiers: u32,
    ) -> Result<()> {
        Err(anyhow!("OSR only on macOS"))
    }

    pub fn send_mouse_move(
        &self,
        _browser_id: BrowserId,
        _x: f32,
        _y: f32,
        _mouse_leave: bool,
        _modifiers: u32,
    ) -> Result<()> {
        Err(anyhow!("OSR only on macOS"))
    }

    pub fn send_mouse_wheel(
        &self,
        _browser_id: BrowserId,
        _x: f32,
        _y: f32,
        _delta_x: f32,
        _delta_y: f32,
        _modifiers: u32,
    ) -> Result<()> {
        Err(anyhow!("OSR only on macOS"))
    }

    pub fn send_key_event(&self, _browser_id: BrowserId, _event: &KeyEventPayload) -> Result<()> {
        Err(anyhow!("OSR only on macOS"))
    }

    pub fn set_focus(&self, _browser_id: BrowserId, _focused: bool) -> Result<()> {
        Err(anyhow!("OSR only on macOS"))
    }

    pub fn do_message_loop_work(&self, _table: &CefFunctionTable) {}

    pub fn shutdown(&mut self, _table: &CefFunctionTable) {}
}

#[cfg(target_os = "macos")]
struct MainArgsStorage {
    _args: Vec<CString>,
    argv: Vec<*mut std::ffi::c_char>,
}

#[cfg(target_os = "macos")]
impl MainArgsStorage {
    fn new() -> Self {
        let args: Vec<CString> = std::env::args()
            .map(|argument| {
                CString::new(argument).unwrap_or_else(|_| CString::new("").expect("empty"))
            })
            .collect();
        let argv: Vec<*mut std::ffi::c_char> = args
            .iter()
            .map(|arg| arg.as_ptr() as *mut std::ffi::c_char)
            .collect();
        Self { _args: args, argv }
    }

    fn as_cef_args(&mut self) -> cef_main_args_t {
        cef_main_args_t {
            argc: self.argv.len() as std::ffi::c_int,
            argv: self.argv.as_mut_ptr(),
        }
    }
}

#[cfg(target_os = "macos")]
pub struct CefRuntime {
    initialized: bool,
    /// Retains argv pointers for the process lifetime required by CEF.
    #[allow(dead_code)]
    main_args: MainArgsStorage,
    browsers: HashMap<BrowserId, *mut cef_browser_t>,
    clients: HashMap<BrowserId, *mut OsrClient>,
    cef_id_to_browser_id: HashMap<std::ffi::c_int, BrowserId>,
}

#[cfg(target_os = "macos")]
unsafe impl Send for CefRuntime {}

#[cfg(target_os = "macos")]
impl CefRuntime {
    pub fn initialize(
        settings: &CefSettings,
        table: &CefFunctionTable,
        library_path: &Path,
    ) -> Result<Self> {
        let framework_dir = framework_dir_from_library_path(library_path)?;
        let main_bundle_path = find_main_bundle_path();
        let helper_path = find_helper_path(main_bundle_path.as_deref()).ok_or_else(|| {
            anyhow!(
                "CEF helper not found; package zeta-cef-helper under the app's Contents/Frameworks directory or set ZETA_CEF_HELPER"
            )
        })?;
        let cache_root = cache_root_path(settings);
        let cache_path = cache_root.join("cache");
        let resources_dir = framework_dir.join("Resources");

        std::fs::create_dir_all(&cache_root)
            .with_context(|| format!("creating CEF cache root {}", cache_root.display()))?;
        std::fs::create_dir_all(&cache_path)
            .with_context(|| format!("creating CEF cache path {}", cache_path.display()))?;

        // Child CEF helpers resolve the framework via this env var.
        // SAFETY: called once during host init before subprocess spawn.
        unsafe {
            std::env::set_var("ZETA_CEF_FRAMEWORK", &framework_dir);
        }

        let mut main_args = MainArgsStorage::new();
        let cef_main_args = main_args.as_cef_args();

        let subprocess_exit = unsafe {
            (table.cef_execute_process)(&cef_main_args, std::ptr::null_mut(), std::ptr::null_mut())
        };
        if subprocess_exit >= 0 {
            bail!("cef_execute_process returned {subprocess_exit} in browser host thread");
        }

        let mut cef_settings = cef_settings_t::default();
        cef_settings.size = std::mem::size_of::<cef_settings_t>();
        cef_settings.no_sandbox = 1;
        cef_settings.windowless_rendering_enabled = 1;
        cef_settings.external_message_pump = 1;
        cef_settings.disable_signal_handlers = 1;
        cef_settings.log_severity = settings.log_severity as cef_log_severity_t;
        cef_settings.remote_debugging_port = settings.remote_debugging_port as std::ffi::c_int;

        set_cef_string(
            table,
            &mut cef_settings.framework_dir_path,
            &framework_dir.to_string_lossy(),
        )?;
        set_cef_string(
            table,
            &mut cef_settings.resources_dir_path,
            &resources_dir.to_string_lossy(),
        )?;
        set_cef_string(
            table,
            &mut cef_settings.root_cache_path,
            &cache_root.to_string_lossy(),
        )?;
        set_cef_string(
            table,
            &mut cef_settings.cache_path,
            &cache_path.to_string_lossy(),
        )?;
        if let Some(main_bundle_path) = &main_bundle_path {
            set_cef_string(
                table,
                &mut cef_settings.main_bundle_path,
                &main_bundle_path.to_string_lossy(),
            )?;
        }
        if !settings.user_agent.is_empty() {
            set_cef_string(table, &mut cef_settings.user_agent, &settings.user_agent)?;
        }
        set_cef_string(
            table,
            &mut cef_settings.browser_subprocess_path,
            &helper_path.to_string_lossy(),
        )?;

        let init_ok = unsafe {
            (table.cef_initialize)(
                &cef_main_args,
                &cef_settings,
                std::ptr::null_mut::<cef_app_t>(),
                std::ptr::null_mut(),
            )
        };
        let exit_code = unsafe { (table.cef_get_exit_code)() };

        clear_cef_string(table, &mut cef_settings.framework_dir_path);
        clear_cef_string(table, &mut cef_settings.resources_dir_path);
        clear_cef_string(table, &mut cef_settings.root_cache_path);
        clear_cef_string(table, &mut cef_settings.cache_path);
        clear_cef_string(table, &mut cef_settings.main_bundle_path);
        clear_cef_string(table, &mut cef_settings.user_agent);
        clear_cef_string(table, &mut cef_settings.browser_subprocess_path);

        if init_ok == 0 {
            bail!("cef_initialize failed with exit code {exit_code}");
        }

        Ok(Self {
            initialized: true,
            main_args,
            browsers: HashMap::new(),
            clients: HashMap::new(),
            cef_id_to_browser_id: HashMap::new(),
        })
    }

    pub fn create_browser(
        &mut self,
        url: &str,
        browser_id: BrowserId,
        view_width: i32,
        view_height: i32,
        device_scale_factor: f32,
        table: &CefFunctionTable,
        events: &Sender<CefHostEvent>,
    ) -> Result<()> {
        let client = OsrClient::new(
            browser_id,
            view_width,
            view_height,
            device_scale_factor,
            events.clone(),
            self,
        );
        let client_ptr = Box::into_raw(client);

        let mut window_info = cef_window_info_t::default();
        window_info.bounds = cef_rect_t {
            x: 0,
            y: 0,
            width: view_width,
            height: view_height,
        };
        window_info.windowless_rendering_enabled = 1;

        let mut browser_settings = cef_browser_settings_t::default();
        browser_settings.size = std::mem::size_of::<cef_browser_settings_t>();
        browser_settings.windowless_frame_rate = 30;
        browser_settings.background_color = opaque_white() as cef_color_t;
        browser_settings.javascript_access_clipboard = cef_state_t_STATE_ENABLED;

        let mut url_string = cef_string_t::default();
        set_cef_string(table, &mut url_string, url)?;

        let create_ok = unsafe {
            (table.cef_browser_host_create_browser)(
                &window_info,
                &mut (*client_ptr).client as *mut cef_client_t,
                &url_string,
                &browser_settings,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
            )
        };

        clear_cef_string(table, &mut url_string);

        if create_ok == 0 {
            unsafe {
                OsrClient::release_box(client_ptr);
            }
            bail!("cef_browser_host_create_browser failed for browser {browser_id:?}");
        }

        self.clients.insert(browser_id, client_ptr);
        Ok(())
    }

    pub fn navigate(
        &self,
        browser_id: BrowserId,
        url: &str,
        table: &CefFunctionTable,
    ) -> Result<()> {
        let browser = self
            .browsers
            .get(&browser_id)
            .copied()
            .ok_or_else(|| anyhow!("no CEF browser for {browser_id:?}"))?;
        unsafe {
            let get_main_frame = (*browser).get_main_frame.ok_or_else(|| {
                anyhow!("cef_browser_t.get_main_frame missing for {browser_id:?}")
            })?;
            let frame = get_main_frame(browser);
            if frame.is_null() {
                bail!("get_main_frame returned null for {browser_id:?}");
            }
            let load_url = (*frame)
                .load_url
                .ok_or_else(|| anyhow!("cef_frame_t.load_url missing for {browser_id:?}"))?;
            let mut url_string = cef_string_t::default();
            set_cef_string(table, &mut url_string, url)?;
            load_url(frame, &url_string);
            clear_cef_string(table, &mut url_string);
        }
        Ok(())
    }

    pub fn close_browser(&mut self, browser_id: BrowserId) -> Result<()> {
        let host = self.browser_host(browser_id)?;
        unsafe {
            if let Some(close_browser) = (*host).close_browser {
                close_browser(host, 0);
            }
        }
        Ok(())
    }

    pub fn resize_browser(
        &self,
        browser_id: BrowserId,
        view_width: i32,
        view_height: i32,
        device_scale_factor: f32,
    ) -> Result<()> {
        if view_width <= 0 || view_height <= 0 {
            return Ok(());
        }
        let client_ptr = self
            .clients
            .get(&browser_id)
            .copied()
            .ok_or_else(|| anyhow!("no CEF client for {browser_id:?}"))?;
        let scale = if device_scale_factor.is_finite() && device_scale_factor > 0.0 {
            device_scale_factor
        } else {
            1.0
        };
        unsafe {
            let previous_width = (*client_ptr).view_width.swap(view_width, Ordering::Relaxed);
            let previous_height = (*client_ptr)
                .view_height
                .swap(view_height, Ordering::Relaxed);
            let previous_scale = f32::from_bits(
                (*client_ptr)
                    .device_scale_factor_bits
                    .swap(scale.to_bits(), Ordering::Relaxed),
            );
            if previous_width == view_width
                && previous_height == view_height
                && (previous_scale - scale).abs() < f32::EPSILON
            {
                return Ok(());
            }
        }
        let host = self.browser_host(browser_id)?;
        unsafe {
            if let Some(notify_screen_info_changed) = (*host).notify_screen_info_changed {
                notify_screen_info_changed(host);
            }
            if let Some(was_resized) = (*host).was_resized {
                was_resized(host);
            }
        }
        Ok(())
    }

    pub fn send_mouse_click(
        &self,
        browser_id: BrowserId,
        x: f32,
        y: f32,
        button: MouseButtonKind,
        mouse_up: bool,
        click_count: u32,
        modifiers: u32,
    ) -> Result<()> {
        let host = self.browser_host(browser_id)?;
        let button_type = match button {
            MouseButtonKind::Left => cef_mouse_button_type_t_MBT_LEFT,
            MouseButtonKind::Middle => cef_mouse_button_type_t_MBT_MIDDLE,
            MouseButtonKind::Right => cef_mouse_button_type_t_MBT_RIGHT,
        };
        let mut event = cef_mouse_event_t {
            x: x.round() as std::ffi::c_int,
            y: y.round() as std::ffi::c_int,
            modifiers: modifiers | mouse_button_event_flag(button),
        };
        unsafe {
            let send = (*host)
                .send_mouse_click_event
                .ok_or_else(|| anyhow!("send_mouse_click_event missing"))?;
            send(
                host,
                &event,
                button_type,
                mouse_up as std::ffi::c_int,
                click_count.max(1) as std::ffi::c_int,
            );
            let _ = &mut event;
        }
        Ok(())
    }

    pub fn send_mouse_move(
        &self,
        browser_id: BrowserId,
        x: f32,
        y: f32,
        mouse_leave: bool,
        modifiers: u32,
    ) -> Result<()> {
        let host = self.browser_host(browser_id)?;
        let event = cef_mouse_event_t {
            x: x.round() as std::ffi::c_int,
            y: y.round() as std::ffi::c_int,
            modifiers,
        };
        unsafe {
            let send = (*host)
                .send_mouse_move_event
                .ok_or_else(|| anyhow!("send_mouse_move_event missing"))?;
            send(host, &event, mouse_leave as std::ffi::c_int);
        }
        Ok(())
    }

    pub fn send_mouse_wheel(
        &self,
        browser_id: BrowserId,
        x: f32,
        y: f32,
        delta_x: f32,
        delta_y: f32,
        modifiers: u32,
    ) -> Result<()> {
        let host = self.browser_host(browser_id)?;
        let event = cef_mouse_event_t {
            x: x.round() as std::ffi::c_int,
            y: y.round() as std::ffi::c_int,
            modifiers,
        };
        unsafe {
            let send = (*host)
                .send_mouse_wheel_event
                .ok_or_else(|| anyhow!("send_mouse_wheel_event missing"))?;
            send(
                host,
                &event,
                delta_x.round() as std::ffi::c_int,
                delta_y.round() as std::ffi::c_int,
            );
        }
        Ok(())
    }

    pub fn send_key_event(&self, browser_id: BrowserId, event: &KeyEventPayload) -> Result<()> {
        // macOS: Chromium IME/TSM paths call HIToolbox APIs that assert
        // `dispatch_assert_queue(main)`. Our CEF UI thread is the background
        // `cef-host` thread (external message pump), so SendKeyEvent traps.
        // Mouse/wheel remain safe; keyboard needs a main-thread CEF pump later.
        let _ = (browser_id, event);
        Ok(())
    }

    pub fn set_focus(&self, browser_id: BrowserId, focused: bool) -> Result<()> {
        let host = self.browser_host(browser_id)?;
        unsafe {
            if let Some(set_focus) = (*host).set_focus {
                set_focus(host, focused as std::ffi::c_int);
            }
        }
        Ok(())
    }

    fn browser_host(
        &self,
        browser_id: BrowserId,
    ) -> Result<*mut crate::cef_sys::_cef_browser_host_t> {
        let browser = self
            .browsers
            .get(&browser_id)
            .copied()
            .ok_or_else(|| anyhow!("no CEF browser for {browser_id:?}"))?;
        unsafe {
            let get_host = (*browser)
                .get_host
                .ok_or_else(|| anyhow!("cef_browser_t.get_host missing"))?;
            let host = get_host(browser);
            if host.is_null() {
                bail!("get_host returned null for {browser_id:?}");
            }
            Ok(host)
        }
    }

    pub fn do_message_loop_work(&self, table: &CefFunctionTable) {
        if self.initialized {
            unsafe {
                (table.cef_do_message_loop_work)();
            }
        }
    }

    pub fn shutdown(&mut self, table: &CefFunctionTable) {
        if self.initialized {
            for client_ptr in self.clients.drain().map(|(_, pointer)| pointer) {
                unsafe {
                    OsrClient::release_box(client_ptr);
                }
            }
            self.browsers.clear();
            self.cef_id_to_browser_id.clear();
            unsafe {
                (table.cef_shutdown)();
            }
            self.initialized = false;
        }
    }

    fn register_browser(
        &mut self,
        browser_id: BrowserId,
        cef_browser: *mut cef_browser_t,
        cef_identifier: std::ffi::c_int,
    ) {
        self.browsers.insert(browser_id, cef_browser);
        self.cef_id_to_browser_id.insert(cef_identifier, browser_id);
    }

    fn unregister_browser(&mut self, cef_identifier: std::ffi::c_int) -> Option<BrowserId> {
        if let Some(browser_id) = self.cef_id_to_browser_id.remove(&cef_identifier) {
            self.browsers.remove(&browser_id);
            if let Some(client_ptr) = self.clients.remove(&browser_id) {
                unsafe {
                    OsrClient::release(client_ptr);
                }
            }
            Some(browser_id)
        } else {
            None
        }
    }

    fn browser_id_for_cef_browser(&self, browser: *mut cef_browser_t) -> Option<BrowserId> {
        unsafe {
            let get_identifier = (*browser).get_identifier?;
            let cef_identifier = get_identifier(browser);
            self.cef_id_to_browser_id.get(&cef_identifier).copied()
        }
    }
}

#[cfg(target_os = "macos")]
impl Drop for CefRuntime {
    fn drop(&mut self) {
        let _ = self.initialized;
    }
}

#[cfg(target_os = "macos")]
fn framework_dir_from_library_path(library_path: &Path) -> Result<PathBuf> {
    let framework_dir = library_path
        .parent()
        .ok_or_else(|| anyhow!("libcef path has no parent: {}", library_path.display()))?;
    if !framework_dir.is_dir() {
        bail!(
            "CEF framework directory does not exist: {}",
            framework_dir.display()
        );
    }
    Ok(framework_dir.to_path_buf())
}

#[cfg(target_os = "macos")]
fn cache_root_path(settings: &CefSettings) -> PathBuf {
    if settings.cache_path.is_empty() {
        managed_cef_root().join("user_data")
    } else {
        PathBuf::from(&settings.cache_path)
    }
}

#[cfg(target_os = "macos")]
fn find_main_bundle_path() -> Option<PathBuf> {
    std::env::current_exe().ok().and_then(|executable| {
        executable
            .ancestors()
            .find(|path| path.extension().is_some_and(|extension| extension == "app"))
            .map(Path::to_path_buf)
    })
}

#[cfg(target_os = "macos")]
fn find_helper_path(main_bundle_path: Option<&Path>) -> Option<PathBuf> {
    const HELPER_RELATIVE_PATH: &str =
        "Contents/Frameworks/zeta-cef-helper.app/Contents/MacOS/zeta-cef-helper";

    if let Ok(path) = std::env::var("ZETA_CEF_HELPER") {
        let helper = PathBuf::from(path);
        if helper.is_file() {
            return Some(helper);
        }
        log::warn!(
            "ZETA_CEF_HELPER is set but not a file: {}",
            helper.display()
        );
    }

    if let Some(main_bundle_path) = main_bundle_path {
        let bundled_helper = main_bundle_path.join(HELPER_RELATIVE_PATH);
        if bundled_helper.is_file() {
            return Some(bundled_helper);
        }
    }

    std::env::current_exe()
        .ok()
        .and_then(|executable| executable.parent().map(Path::to_path_buf))
        .map(|parent| parent.join("zeta-cef-helper"))
        .filter(|helper| helper.is_file())
}

#[cfg(target_os = "macos")]
fn opaque_white() -> u32 {
    0xFF_FF_FF_FF
}

#[cfg(target_os = "macos")]
fn set_cef_string(table: &CefFunctionTable, dest: &mut cef_string_t, value: &str) -> Result<()> {
    clear_cef_string(table, dest);
    let result = unsafe {
        (table.cef_string_utf8_to_utf16)(
            value.as_ptr() as *const std::ffi::c_char,
            value.len(),
            dest,
        )
    };
    if result == 0 {
        return Err(anyhow!("cef_string_utf8_to_utf16 failed"));
    }
    Ok(())
}

#[cfg(target_os = "macos")]
fn clear_cef_string(table: &CefFunctionTable, string: &mut cef_string_t) {
    unsafe {
        (table.cef_string_utf16_clear)(string);
    }
}

#[cfg(target_os = "macos")]
fn cef_string_to_rust(string: &cef_string_t) -> String {
    if string.str_.is_null() || string.length == 0 {
        return String::new();
    }
    let units = unsafe { std::slice::from_raw_parts(string.str_, string.length) };
    String::from_utf16_lossy(units)
}

#[cfg(target_os = "macos")]
fn emit_paint(
    browser_id: BrowserId,
    width: i32,
    height: i32,
    buffer: *const std::ffi::c_void,
    events: &Sender<CefHostEvent>,
) {
    if width <= 0 || height <= 0 {
        return;
    }
    let byte_count = match (width as u64)
        .checked_mul(height as u64)
        .and_then(|pixels| pixels.checked_mul(4))
    {
        Some(count) if count <= usize::MAX as u64 => count as usize,
        _ => return,
    };

    let mut bytes = vec![0_u8; byte_count];
    unsafe {
        std::ptr::copy_nonoverlapping(buffer as *const u8, bytes.as_mut_ptr(), byte_count);
    }

    let paint = PaintBuffer {
        browser_id,
        width: width as u32,
        height: height as u32,
        bytes,
    };
    if let Ok(frame) = osr::paint_buffer_to_shared_frame(&paint) {
        let _ = events.send_blocking(CefHostEvent::Paint(paint));
        let _ = events.send_blocking(CefHostEvent::Frame(frame));
    }
}

#[cfg(target_os = "macos")]
#[repr(C)]
struct OsrClient {
    render: cef_render_handler_t,
    life_span: cef_life_span_handler_t,
    load: cef_load_handler_t,
    client: cef_client_t,
    ref_count: AtomicUsize,
    browser_id: BrowserId,
    view_width: AtomicI32,
    view_height: AtomicI32,
    device_scale_factor_bits: AtomicU32,
    events: Sender<CefHostEvent>,
    runtime: *mut CefRuntime,
}

#[cfg(target_os = "macos")]
fn mouse_button_event_flag(button: MouseButtonKind) -> u32 {
    match button {
        MouseButtonKind::Left => cef_event_flags_t_EVENTFLAG_LEFT_MOUSE_BUTTON,
        MouseButtonKind::Middle => cef_event_flags_t_EVENTFLAG_MIDDLE_MOUSE_BUTTON,
        MouseButtonKind::Right => cef_event_flags_t_EVENTFLAG_RIGHT_MOUSE_BUTTON,
    }
}

#[cfg(target_os = "macos")]
impl OsrClient {
    fn new(
        browser_id: BrowserId,
        view_width: i32,
        view_height: i32,
        device_scale_factor: f32,
        events: Sender<CefHostEvent>,
        runtime: &mut CefRuntime,
    ) -> Box<Self> {
        let scale = if device_scale_factor.is_finite() && device_scale_factor > 0.0 {
            device_scale_factor
        } else {
            1.0
        };
        let mut client = Box::new(Self {
            render: cef_render_handler_t::default(),
            life_span: cef_life_span_handler_t::default(),
            load: cef_load_handler_t::default(),
            client: cef_client_t::default(),
            ref_count: AtomicUsize::new(1),
            browser_id,
            view_width: AtomicI32::new(view_width.max(1)),
            view_height: AtomicI32::new(view_height.max(1)),
            device_scale_factor_bits: AtomicU32::new(scale.to_bits()),
            events,
            runtime: runtime as *mut CefRuntime,
        });
        client.init_vtables();
        client
    }

    fn init_vtables(&mut self) {
        self.render.base.size = std::mem::size_of::<cef_render_handler_t>();
        self.render.base.add_ref = Some(render_add_ref);
        self.render.base.release = Some(render_release);
        self.render.base.has_one_ref = Some(render_has_one_ref);
        self.render.base.has_at_least_one_ref = Some(render_has_at_least_one_ref);
        self.render.get_view_rect = Some(render_get_view_rect);
        self.render.get_screen_info = Some(render_get_screen_info);
        self.render.on_paint = Some(render_on_paint);

        self.life_span.base.size = std::mem::size_of::<cef_life_span_handler_t>();
        self.life_span.base.add_ref = Some(life_span_add_ref);
        self.life_span.base.release = Some(life_span_release);
        self.life_span.base.has_one_ref = Some(life_span_has_one_ref);
        self.life_span.base.has_at_least_one_ref = Some(life_span_has_at_least_one_ref);
        self.life_span.on_before_popup = Some(life_span_on_before_popup);
        self.life_span.on_after_created = Some(life_span_on_after_created);
        self.life_span.on_before_close = Some(life_span_on_before_close);

        self.load.base.size = std::mem::size_of::<cef_load_handler_t>();
        self.load.base.add_ref = Some(load_add_ref);
        self.load.base.release = Some(load_release);
        self.load.base.has_one_ref = Some(load_has_one_ref);
        self.load.base.has_at_least_one_ref = Some(load_has_at_least_one_ref);
        self.load.on_load_start = Some(load_on_load_start);
        self.load.on_load_end = Some(load_on_load_end);
        self.load.on_load_error = Some(load_on_load_error);

        self.client.base.size = std::mem::size_of::<cef_client_t>();
        self.client.base.add_ref = Some(client_add_ref);
        self.client.base.release = Some(client_release);
        self.client.base.has_one_ref = Some(client_has_one_ref);
        self.client.base.has_at_least_one_ref = Some(client_has_at_least_one_ref);
        self.client.get_life_span_handler = Some(client_get_life_span_handler);
        self.client.get_load_handler = Some(client_get_load_handler);
        self.client.get_render_handler = Some(client_get_render_handler);
    }

    unsafe fn from_render(ptr: *mut cef_render_handler_t) -> *mut Self {
        (ptr as *mut u8).sub(offset_of!(Self, render)) as *mut Self
    }

    unsafe fn from_life_span(ptr: *mut cef_life_span_handler_t) -> *mut Self {
        (ptr as *mut u8).sub(offset_of!(Self, life_span)) as *mut Self
    }

    unsafe fn from_load(ptr: *mut cef_load_handler_t) -> *mut Self {
        (ptr as *mut u8).sub(offset_of!(Self, load)) as *mut Self
    }

    unsafe fn from_client(ptr: *mut cef_client_t) -> *mut Self {
        (ptr as *mut u8).sub(offset_of!(Self, client)) as *mut Self
    }

    unsafe fn add_ref(instance: *mut Self) {
        (*instance).ref_count.fetch_add(1, Ordering::Relaxed);
    }

    unsafe fn release(instance: *mut Self) -> std::ffi::c_int {
        let previous = (*instance).ref_count.fetch_sub(1, Ordering::AcqRel);
        if previous <= 1 {
            drop(Box::from_raw(instance));
            1
        } else {
            0
        }
    }

    unsafe fn release_box(instance: *mut Self) {
        loop {
            if (*instance).ref_count.load(Ordering::Acquire) == 0 {
                break;
            }
            if Self::release(instance) != 0 {
                break;
            }
        }
    }
}

#[cfg(target_os = "macos")]
unsafe extern "C" fn render_add_ref(base: *mut cef_base_ref_counted_t) {
    OsrClient::add_ref(OsrClient::from_render(base as *mut cef_render_handler_t));
}

#[cfg(target_os = "macos")]
unsafe extern "C" fn render_release(base: *mut cef_base_ref_counted_t) -> std::ffi::c_int {
    OsrClient::release(OsrClient::from_render(base as *mut cef_render_handler_t))
}

#[cfg(target_os = "macos")]
unsafe extern "C" fn render_has_one_ref(base: *mut cef_base_ref_counted_t) -> std::ffi::c_int {
    let client = OsrClient::from_render(base as *mut cef_render_handler_t);
    ((*client).ref_count.load(Ordering::Acquire) == 1) as std::ffi::c_int
}

#[cfg(target_os = "macos")]
unsafe extern "C" fn render_has_at_least_one_ref(
    base: *mut cef_base_ref_counted_t,
) -> std::ffi::c_int {
    let client = OsrClient::from_render(base as *mut cef_render_handler_t);
    ((*client).ref_count.load(Ordering::Acquire) >= 1) as std::ffi::c_int
}

#[cfg(target_os = "macos")]
unsafe extern "C" fn render_get_view_rect(
    self_: *mut cef_render_handler_t,
    _browser: *mut cef_browser_t,
    rect: *mut cef_rect_t,
) {
    let client = OsrClient::from_render(self_);
    if rect.is_null() {
        return;
    }
    (*rect).x = 0;
    (*rect).y = 0;
    (*rect).width = (*client).view_width.load(Ordering::Relaxed);
    (*rect).height = (*client).view_height.load(Ordering::Relaxed);
}

#[cfg(target_os = "macos")]
unsafe extern "C" fn render_get_screen_info(
    self_: *mut cef_render_handler_t,
    _browser: *mut cef_browser_t,
    screen_info: *mut cef_screen_info_t,
) -> std::ffi::c_int {
    let client = OsrClient::from_render(self_);
    if screen_info.is_null() {
        return 0;
    }
    let width = (*client).view_width.load(Ordering::Relaxed);
    let height = (*client).view_height.load(Ordering::Relaxed);
    let scale = f32::from_bits((*client).device_scale_factor_bits.load(Ordering::Relaxed));
    (*screen_info).device_scale_factor = scale;
    (*screen_info).depth = 24;
    (*screen_info).depth_per_component = 8;
    (*screen_info).is_monochrome = 0;
    (*screen_info).rect.x = 0;
    (*screen_info).rect.y = 0;
    (*screen_info).rect.width = width;
    (*screen_info).rect.height = height;
    (*screen_info).available_rect = (*screen_info).rect;
    1
}

#[cfg(target_os = "macos")]
unsafe extern "C" fn render_on_paint(
    self_: *mut cef_render_handler_t,
    browser: *mut cef_browser_t,
    type_: cef_paint_element_type_t,
    _dirty_rects_count: usize,
    _dirty_rects: *const cef_rect_t,
    buffer: *const std::ffi::c_void,
    width: std::ffi::c_int,
    height: std::ffi::c_int,
) {
    if type_ != cef_paint_element_type_t_PET_VIEW {
        return;
    }
    let client = OsrClient::from_render(self_);
    let browser_id = if (*client).runtime.is_null() {
        (*client).browser_id
    } else {
        (*(*client).runtime)
            .browser_id_for_cef_browser(browser)
            .unwrap_or((*client).browser_id)
    };
    emit_paint(browser_id, width, height, buffer, &(*client).events);
}

#[cfg(target_os = "macos")]
unsafe extern "C" fn life_span_add_ref(base: *mut cef_base_ref_counted_t) {
    OsrClient::add_ref(OsrClient::from_life_span(
        base as *mut cef_life_span_handler_t,
    ));
}

#[cfg(target_os = "macos")]
unsafe extern "C" fn life_span_release(base: *mut cef_base_ref_counted_t) -> std::ffi::c_int {
    OsrClient::release(OsrClient::from_life_span(
        base as *mut cef_life_span_handler_t,
    ))
}

#[cfg(target_os = "macos")]
unsafe extern "C" fn life_span_has_one_ref(base: *mut cef_base_ref_counted_t) -> std::ffi::c_int {
    let client = OsrClient::from_life_span(base as *mut cef_life_span_handler_t);
    ((*client).ref_count.load(Ordering::Acquire) == 1) as std::ffi::c_int
}

#[cfg(target_os = "macos")]
unsafe extern "C" fn life_span_has_at_least_one_ref(
    base: *mut cef_base_ref_counted_t,
) -> std::ffi::c_int {
    let client = OsrClient::from_life_span(base as *mut cef_life_span_handler_t);
    ((*client).ref_count.load(Ordering::Acquire) >= 1) as std::ffi::c_int
}

#[cfg(target_os = "macos")]
unsafe extern "C" fn life_span_on_before_popup(
    _self_: *mut cef_life_span_handler_t,
    _browser: *mut cef_browser_t,
    _frame: *mut cef_frame_t,
    _popup_id: std::ffi::c_int,
    _target_url: *const cef_string_t,
    _target_frame_name: *const cef_string_t,
    _target_disposition: crate::cef_sys::cef_window_open_disposition_t,
    _user_gesture: std::ffi::c_int,
    _popup_features: *const crate::cef_sys::cef_popup_features_t,
    _window_info: *mut cef_window_info_t,
    _client: *mut *mut cef_client_t,
    _settings: *mut cef_browser_settings_t,
    _extra_info: *mut *mut crate::cef_sys::cef_dictionary_value_t,
    _no_javascript_access: *mut std::ffi::c_int,
) -> std::ffi::c_int {
    1
}

#[cfg(target_os = "macos")]
unsafe extern "C" fn life_span_on_after_created(
    self_: *mut cef_life_span_handler_t,
    browser: *mut cef_browser_t,
) {
    let client = OsrClient::from_life_span(self_);
    if browser.is_null() {
        return;
    }
    if let Some(add_ref) = (*browser).base.add_ref {
        add_ref(&mut (*browser).base);
    }
    let cef_identifier = (*browser)
        .get_identifier
        .map(|get_identifier| get_identifier(browser))
        .unwrap_or(0);
    if !(*client).runtime.is_null() {
        (*(*client).runtime).register_browser((*client).browser_id, browser, cef_identifier);
    }
    if let Some(get_host) = (*browser).get_host {
        let host = get_host(browser);
        if !host.is_null() {
            if let Some(notify_screen_info_changed) = (*host).notify_screen_info_changed {
                notify_screen_info_changed(host);
            }
            if let Some(resize) = (*host).was_resized {
                resize(host);
            }
        }
    }
}

#[cfg(target_os = "macos")]
unsafe extern "C" fn life_span_on_before_close(
    self_: *mut cef_life_span_handler_t,
    browser: *mut cef_browser_t,
) {
    let client = OsrClient::from_life_span(self_);
    let cef_identifier = (*browser)
        .get_identifier
        .map(|get_identifier| get_identifier(browser))
        .unwrap_or(0);
    let browser_id = if (*client).runtime.is_null() {
        None
    } else {
        (*(*client).runtime).unregister_browser(cef_identifier)
    };
    if let Some(browser_id) = browser_id {
        let _ = (*client)
            .events
            .send_blocking(CefHostEvent::BrowserClosed(browser_id));
    }
    if let Some(release) = (*browser).base.release {
        release(&mut (*browser).base);
    }
}

#[cfg(target_os = "macos")]
unsafe extern "C" fn load_add_ref(base: *mut cef_base_ref_counted_t) {
    OsrClient::add_ref(OsrClient::from_load(base as *mut cef_load_handler_t));
}

#[cfg(target_os = "macos")]
unsafe extern "C" fn load_release(base: *mut cef_base_ref_counted_t) -> std::ffi::c_int {
    OsrClient::release(OsrClient::from_load(base as *mut cef_load_handler_t))
}

#[cfg(target_os = "macos")]
unsafe extern "C" fn load_has_one_ref(base: *mut cef_base_ref_counted_t) -> std::ffi::c_int {
    let client = OsrClient::from_load(base as *mut cef_load_handler_t);
    ((*client).ref_count.load(Ordering::Acquire) == 1) as std::ffi::c_int
}

#[cfg(target_os = "macos")]
unsafe extern "C" fn load_has_at_least_one_ref(
    base: *mut cef_base_ref_counted_t,
) -> std::ffi::c_int {
    let client = OsrClient::from_load(base as *mut cef_load_handler_t);
    ((*client).ref_count.load(Ordering::Acquire) >= 1) as std::ffi::c_int
}

#[cfg(target_os = "macos")]
unsafe extern "C" fn load_on_load_start(
    self_: *mut cef_load_handler_t,
    _browser: *mut cef_browser_t,
    frame: *mut cef_frame_t,
    _transition_type: crate::cef_sys::cef_transition_type_t,
) {
    let client = OsrClient::from_load(self_);
    if frame.is_null() {
        return;
    }
    let is_main = (*frame)
        .is_main
        .map(|is_main| is_main(frame) != 0)
        .unwrap_or(false);
    if !is_main {
        return;
    }
    let url = read_frame_url(frame);
    let browser_id = (*client).browser_id;
    let _ = (*client).events.send_blocking(CefHostEvent::LoadStart {
        id: browser_id,
        url,
    });
    let _ = (*client)
        .events
        .send_blocking(CefHostEvent::AddressChanged {
            id: browser_id,
            url: read_frame_url(frame),
        });
}

#[cfg(target_os = "macos")]
unsafe extern "C" fn load_on_load_end(
    self_: *mut cef_load_handler_t,
    _browser: *mut cef_browser_t,
    frame: *mut cef_frame_t,
    http_status_code: std::ffi::c_int,
) {
    let client = OsrClient::from_load(self_);
    if frame.is_null() {
        return;
    }
    let is_main = (*frame)
        .is_main
        .map(|is_main| is_main(frame) != 0)
        .unwrap_or(false);
    if !is_main {
        return;
    }
    let _ = (*client).events.send_blocking(CefHostEvent::LoadEnd {
        id: (*client).browser_id,
        http_status: http_status_code,
    });
}

#[cfg(target_os = "macos")]
unsafe extern "C" fn load_on_load_error(
    self_: *mut cef_load_handler_t,
    _browser: *mut cef_browser_t,
    frame: *mut cef_frame_t,
    _error_code: crate::cef_sys::cef_errorcode_t,
    error_text: *const cef_string_t,
    _failed_url: *const cef_string_t,
) {
    let client = OsrClient::from_load(self_);
    if frame.is_null() {
        return;
    }
    let is_main = (*frame)
        .is_main
        .map(|is_main| is_main(frame) != 0)
        .unwrap_or(false);
    if !is_main {
        return;
    }
    let message = if error_text.is_null() {
        String::from("load error")
    } else {
        cef_string_to_rust(&*error_text)
    };
    let _ = (*client).events.send_blocking(CefHostEvent::LoadError {
        id: (*client).browser_id,
        message,
    });
}

#[cfg(target_os = "macos")]
unsafe fn read_frame_url(frame: *mut cef_frame_t) -> String {
    if let Some(get_url) = (*frame).get_url {
        let userfree = get_url(frame);
        if userfree.is_null() {
            return String::new();
        }
        let url = cef_string_to_rust(&*userfree);
        if let Some(dtor) = (*userfree).dtor {
            dtor((*userfree).str_);
        }
        return url;
    }
    String::new()
}

#[cfg(target_os = "macos")]
unsafe extern "C" fn client_add_ref(base: *mut cef_base_ref_counted_t) {
    OsrClient::add_ref(OsrClient::from_client(base as *mut cef_client_t));
}

#[cfg(target_os = "macos")]
unsafe extern "C" fn client_release(base: *mut cef_base_ref_counted_t) -> std::ffi::c_int {
    OsrClient::release(OsrClient::from_client(base as *mut cef_client_t))
}

#[cfg(target_os = "macos")]
unsafe extern "C" fn client_has_one_ref(base: *mut cef_base_ref_counted_t) -> std::ffi::c_int {
    let client = OsrClient::from_client(base as *mut cef_client_t);
    ((*client).ref_count.load(Ordering::Acquire) == 1) as std::ffi::c_int
}

#[cfg(target_os = "macos")]
unsafe extern "C" fn client_has_at_least_one_ref(
    base: *mut cef_base_ref_counted_t,
) -> std::ffi::c_int {
    let client = OsrClient::from_client(base as *mut cef_client_t);
    ((*client).ref_count.load(Ordering::Acquire) >= 1) as std::ffi::c_int
}

#[cfg(target_os = "macos")]
unsafe extern "C" fn client_get_life_span_handler(
    self_: *mut cef_client_t,
) -> *mut cef_life_span_handler_t {
    let client = OsrClient::from_client(self_);
    OsrClient::add_ref(client);
    &mut (*client).life_span
}

#[cfg(target_os = "macos")]
unsafe extern "C" fn client_get_load_handler(self_: *mut cef_client_t) -> *mut cef_load_handler_t {
    let client = OsrClient::from_client(self_);
    OsrClient::add_ref(client);
    &mut (*client).load
}

#[cfg(target_os = "macos")]
unsafe extern "C" fn client_get_render_handler(
    self_: *mut cef_client_t,
) -> *mut cef_render_handler_t {
    let client = OsrClient::from_client(self_);
    OsrClient::add_ref(client);
    &mut (*client).render
}
