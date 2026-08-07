//! Dynamic CEF symbol table loaded via `libloading`.

use std::path::Path;

use anyhow::{Context as _, Result, anyhow};
use libloading::{Library, Symbol};

/// Raw CEF entry points we need for host lifecycle.
///
/// Signatures intentionally stay close to the C ABI. Full OSR/client vtables
/// are wired in later stack layers; this table only proves the shared library
/// can be opened and core symbols resolved.
pub struct CefFunctionTable {
    _library: Library,
    pub cef_initialize: unsafe extern "C" fn(
        args: *const std::ffi::c_void,
        settings: *const std::ffi::c_void,
        application: *mut std::ffi::c_void,
        windows_sandbox_info: *mut std::ffi::c_void,
    ) -> std::ffi::c_int,
    pub cef_shutdown: unsafe extern "C" fn(),
    pub cef_do_message_loop_work: unsafe extern "C" fn(),
    pub cef_run_message_loop: unsafe extern "C" fn(),
    pub cef_quit_message_loop: unsafe extern "C" fn(),
}

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

            Ok(Self {
                _library: library,
                cef_initialize,
                cef_shutdown,
                cef_do_message_loop_work,
                cef_run_message_loop,
                cef_quit_message_loop,
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
