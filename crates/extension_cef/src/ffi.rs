//! Dynamic CEF symbol table loaded via `libloading`.

use std::path::Path;

use anyhow::{Context as _, Result, anyhow};
use libloading::{Library, Symbol};

#[cfg(target_os = "macos")]
use crate::cef_sys::{
    cef_app_t, cef_browser_settings_t, cef_client_t, cef_dictionary_value_t, cef_main_args_t,
    cef_settings_t, cef_string_utf16_t, cef_window_info_t,
};

/// Raw CEF entry points resolved from `libcef`.
pub struct CefFunctionTable {
    _library: Library,
    pub cef_initialize: CefInitializeFn,
    pub cef_shutdown: CefShutdownFn,
    pub cef_do_message_loop_work: CefDoMessageLoopWorkFn,
    pub cef_run_message_loop: CefRunMessageLoopFn,
    pub cef_quit_message_loop: CefQuitMessageLoopFn,
    pub cef_execute_process: CefExecuteProcessFn,
    pub cef_browser_host_create_browser: CefBrowserHostCreateBrowserFn,
    pub cef_string_utf8_to_utf16: CefStringUtf8ToUtf16Fn,
    pub cef_string_utf16_clear: CefStringUtf16ClearFn,
    pub cef_get_exit_code: CefGetExitCodeFn,
}

#[cfg(target_os = "macos")]
pub type CefInitializeFn = unsafe extern "C" fn(
    args: *const cef_main_args_t,
    settings: *const cef_settings_t,
    application: *mut cef_app_t,
    windows_sandbox_info: *mut std::ffi::c_void,
) -> std::ffi::c_int;

#[cfg(not(target_os = "macos"))]
pub type CefInitializeFn = unsafe extern "C" fn(
    args: *const std::ffi::c_void,
    settings: *const std::ffi::c_void,
    application: *mut std::ffi::c_void,
    windows_sandbox_info: *mut std::ffi::c_void,
) -> std::ffi::c_int;

pub type CefShutdownFn = unsafe extern "C" fn();
pub type CefDoMessageLoopWorkFn = unsafe extern "C" fn();
pub type CefRunMessageLoopFn = unsafe extern "C" fn();
pub type CefQuitMessageLoopFn = unsafe extern "C" fn();

#[cfg(target_os = "macos")]
pub type CefExecuteProcessFn = unsafe extern "C" fn(
    args: *const cef_main_args_t,
    application: *mut cef_app_t,
    windows_sandbox_info: *mut std::ffi::c_void,
) -> std::ffi::c_int;

#[cfg(not(target_os = "macos"))]
pub type CefExecuteProcessFn = unsafe extern "C" fn(
    args: *const std::ffi::c_void,
    application: *mut std::ffi::c_void,
    windows_sandbox_info: *mut std::ffi::c_void,
) -> std::ffi::c_int;

#[cfg(target_os = "macos")]
pub type CefBrowserHostCreateBrowserFn = unsafe extern "C" fn(
    window_info: *const cef_window_info_t,
    client: *mut cef_client_t,
    url: *const crate::cef_sys::cef_string_t,
    settings: *const cef_browser_settings_t,
    extra_info: *mut cef_dictionary_value_t,
    request_context: *mut crate::cef_sys::cef_request_context_t,
) -> std::ffi::c_int;

#[cfg(not(target_os = "macos"))]
pub type CefBrowserHostCreateBrowserFn = unsafe extern "C" fn(
    window_info: *const std::ffi::c_void,
    client: *mut std::ffi::c_void,
    url: *const std::ffi::c_void,
    settings: *const std::ffi::c_void,
    extra_info: *mut std::ffi::c_void,
    request_context: *mut std::ffi::c_void,
) -> std::ffi::c_int;

#[cfg(target_os = "macos")]
pub type CefStringUtf8ToUtf16Fn = unsafe extern "C" fn(
    source: *const std::ffi::c_char,
    source_length: usize,
    output: *mut cef_string_utf16_t,
) -> std::ffi::c_int;

#[cfg(not(target_os = "macos"))]
pub type CefStringUtf8ToUtf16Fn = unsafe extern "C" fn(
    source: *const std::ffi::c_char,
    source_length: usize,
    output: *mut std::ffi::c_void,
) -> std::ffi::c_int;

#[cfg(target_os = "macos")]
pub type CefStringUtf16ClearFn = unsafe extern "C" fn(string: *mut cef_string_utf16_t);

#[cfg(not(target_os = "macos"))]
pub type CefStringUtf16ClearFn = unsafe extern "C" fn(string: *mut std::ffi::c_void);

pub type CefGetExitCodeFn = unsafe extern "C" fn() -> std::ffi::c_int;

impl CefFunctionTable {
    /// Load `libcef` from `path` and resolve required symbols.
    ///
    /// # Safety
    ///
    /// The shared library must export CEF symbols matching these signatures.
    pub unsafe fn load(path: &Path) -> Result<Self> {
        unsafe {
            let library = Library::new(path)
                .with_context(|| format!("failed to dlopen {}", path.display()))?;
            let cef_initialize = Self::symbol(&library, b"cef_initialize\0")?;
            let cef_shutdown = Self::symbol(&library, b"cef_shutdown\0")?;
            let cef_do_message_loop_work = Self::symbol(&library, b"cef_do_message_loop_work\0")?;
            let cef_run_message_loop = Self::symbol(&library, b"cef_run_message_loop\0")?;
            let cef_quit_message_loop = Self::symbol(&library, b"cef_quit_message_loop\0")?;
            let cef_execute_process = Self::symbol(&library, b"cef_execute_process\0")?;
            let cef_browser_host_create_browser =
                Self::symbol(&library, b"cef_browser_host_create_browser\0")?;
            let cef_string_utf8_to_utf16 = Self::symbol(&library, b"cef_string_utf8_to_utf16\0")?;
            let cef_string_utf16_clear = Self::symbol(&library, b"cef_string_utf16_clear\0")?;
            let cef_get_exit_code = Self::symbol(&library, b"cef_get_exit_code\0")?;

            Ok(Self {
                _library: library,
                cef_initialize,
                cef_shutdown,
                cef_do_message_loop_work,
                cef_run_message_loop,
                cef_quit_message_loop,
                cef_execute_process,
                cef_browser_host_create_browser,
                cef_string_utf8_to_utf16,
                cef_string_utf16_clear,
                cef_get_exit_code,
            })
        }
    }

    unsafe fn symbol<T>(library: &Library, name: &[u8]) -> Result<T>
    where
        T: Copy,
    {
        let symbol: Symbol<T> = unsafe {
            library
                .get(name)
                .with_context(|| format!("missing CEF symbol {}", String::from_utf8_lossy(name)))?
        };
        Ok(*symbol)
    }
}

/// Probe whether `path` looks like a loadable CEF library (open + resolve).
///
/// Absolute paths must exist. Bare sonames are passed to the dynamic loader
/// so the platform search path can resolve them.
pub fn try_load(path: &Path) -> Result<CefFunctionTable> {
    if path.is_absolute() && !path.exists() {
        return Err(anyhow!("path does not exist: {}", path.display()));
    }
    unsafe { CefFunctionTable::load(path) }
}
