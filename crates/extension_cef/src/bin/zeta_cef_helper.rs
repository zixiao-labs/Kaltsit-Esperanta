//! CEF subprocess helper for macOS.

use std::env;
use std::ffi::CString;
use std::path::PathBuf;
use std::process;

use libloading::{Library, Symbol};

#[repr(C)]
struct CefMainArgs {
    argc: std::ffi::c_int,
    argv: *mut *mut std::ffi::c_char,
}

fn main() {
    if let Err(error) = run() {
        eprintln!("zeta-cef-helper failed: {error:#}");
        process::exit(1);
    }
}

fn run() -> Result<(), Box<dyn std::error::Error>> {
    #[cfg(not(target_os = "macos"))]
    {
        return Err("zeta-cef-helper is only supported on macOS".into());
    }

    #[cfg(target_os = "macos")]
    {
        let framework_binary = resolve_framework_binary()?;
        let library = unsafe { Library::new(&framework_binary)? };

        type CefExecuteProcessFn = unsafe extern "C" fn(
            args: *const CefMainArgs,
            application: *mut std::ffi::c_void,
            windows_sandbox_info: *mut std::ffi::c_void,
        ) -> std::ffi::c_int;

        let cef_execute_process: Symbol<CefExecuteProcessFn> =
            unsafe { library.get(b"cef_execute_process\0")? };

        let args: Vec<CString> = env::args()
            .map(|argument| {
                CString::new(argument).unwrap_or_else(|_| CString::new("").expect("empty"))
            })
            .collect();
        let mut argv: Vec<*mut std::ffi::c_char> = args
            .iter()
            .map(|arg| arg.as_ptr() as *mut std::ffi::c_char)
            .collect();

        let main_args = CefMainArgs {
            argc: argv.len() as std::ffi::c_int,
            argv: argv.as_mut_ptr(),
        };

        let exit_code =
            unsafe { cef_execute_process(&main_args, std::ptr::null_mut(), std::ptr::null_mut()) };

        if exit_code == -1 {
            process::exit(0);
        }
        process::exit(exit_code);
    }
}

#[cfg(target_os = "macos")]
fn resolve_framework_binary() -> Result<PathBuf, Box<dyn std::error::Error>> {
    if let Ok(path) = env::var("ZETA_CEF_FRAMEWORK") {
        let framework = PathBuf::from(path);
        let binary = framework.join("Chromium Embedded Framework");
        if binary.is_file() {
            return Ok(binary);
        }
        return Err(format!(
            "ZETA_CEF_FRAMEWORK does not contain Chromium Embedded Framework binary: {}",
            framework.display()
        )
        .into());
    }

    if let Ok(exe) = env::current_exe() {
        if let Some(parent) = exe.parent() {
            let sibling =
                parent.join("Chromium Embedded Framework.framework/Chromium Embedded Framework");
            if sibling.is_file() {
                return Ok(sibling);
            }
        }
    }

    Err("could not resolve Chromium Embedded Framework binary; set ZETA_CEF_FRAMEWORK".into())
}
