//! Chromium Embedded Framework (CEF) host with asynchronous, fail-soft loading.
//!
//! `libcef` is loaded via `libloading` on a dedicated background thread so GPUI
//! never blocks on `dlopen` / CEF initialize. When the library is missing or
//! symbols cannot be resolved, the host stays in [`HostLifecycle::Failed`] and
//! callers can show a clear unavailable state.

mod cdp;
mod ffi;
mod host;
mod osr;
mod session;

pub use async_host_runtime::{HostLifecycle, HostLifecycleCell};
pub use cdp::{CdpSession, ConsoleMessage, DesignNodeInfo, NetworkEntry};
pub use host::{
    BrowserId, CefBrowserSettings, CefHost, CefHostCommand, CefHostEvent, CefLogSeverity,
    CefSettings, KeyEventPayload, MouseButtonKind, PaintBuffer,
};
pub use osr::{SharedPaintFrame, paint_buffer_to_shared_frame, solid_color_paint};
pub use session::{AsyncCefHost, probe_libcef_path};

use anyhow::Result;

/// Candidate filenames / sonames for the CEF shared library.
pub fn default_libcef_candidates() -> &'static [&'static str] {
    #[cfg(target_os = "macos")]
    {
        &[
            "libcef.dylib",
            "Chromium Embedded Framework",
            "/Library/Frameworks/Chromium Embedded Framework.framework/Chromium Embedded Framework",
            "/usr/local/lib/libcef.dylib",
            "/opt/homebrew/lib/libcef.dylib",
        ]
    }
    #[cfg(target_os = "linux")]
    {
        &[
            "libcef.so",
            "/usr/lib/libcef.so",
            "/usr/local/lib/libcef.so",
            "/opt/cef/libcef.so",
        ]
    }
    #[cfg(target_os = "windows")]
    {
        &["libcef.dll", "cef.dll"]
    }
    #[cfg(not(any(target_os = "macos", target_os = "linux", target_os = "windows")))]
    {
        &[]
    }
}

/// Resolve a library path from an explicit override or platform defaults.
pub fn resolve_libcef_path(explicit: Option<&str>) -> Result<String> {
    if let Some(path) = explicit {
        return Ok(path.to_owned());
    }
    probe_libcef_path().ok_or_else(|| {
        anyhow::anyhow!(
            "libcef not found; install CEF and ensure it is on the loader path \
             (looked for: {})",
            default_libcef_candidates().join(", ")
        )
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use async_host_runtime::HostLifecycle;
    use std::time::Duration;

    #[test]
    fn settings_default_enables_windowless() {
        let settings = CefBrowserSettings::default();
        assert!(settings.windowless_rendering_enabled);
    }

    #[test]
    fn async_host_fails_soft_without_libcef() {
        // Force the stub path so CI machines without CEF still exercise the
        // async session machinery.
        let host = AsyncCefHost::spawn_stub(CefSettings::default());
        let deadline = std::time::Instant::now() + Duration::from_secs(2);
        loop {
            match host.lifecycle().get() {
                HostLifecycle::Ready => break,
                HostLifecycle::Failed { message } => {
                    panic!("stub host should become Ready, got Failed: {message}");
                }
                HostLifecycle::Loading => {
                    assert!(std::time::Instant::now() < deadline);
                    std::thread::sleep(Duration::from_millis(5));
                }
            }
        }

        let browser = smol::block_on(host.create_browser("https://example.com", None))
            .expect("stub create_browser");
        assert_eq!(browser.0, 1);

        drop(host);
    }

    #[test]
    fn stub_create_browser_emits_osr_frame() {
        let host = AsyncCefHost::spawn_stub(CefSettings::default());
        let deadline = std::time::Instant::now() + Duration::from_secs(2);
        while !matches!(host.lifecycle().get(), HostLifecycle::Ready) {
            assert!(std::time::Instant::now() < deadline);
            std::thread::sleep(Duration::from_millis(5));
        }

        smol::block_on(host.create_browser("https://example.com", None)).expect("create");

        let deadline = std::time::Instant::now() + Duration::from_secs(2);
        let mut saw_frame = false;
        while std::time::Instant::now() < deadline {
            while let Ok(event) = host.try_recv_event() {
                if matches!(event, CefHostEvent::Frame(_)) {
                    saw_frame = true;
                    break;
                }
            }
            if saw_frame {
                break;
            }
            std::thread::sleep(Duration::from_millis(5));
        }
        assert!(saw_frame, "expected OSR Frame event from stub browser");
    }

    #[test]
    fn stub_navigate_emits_osr_frame() {
        let host = AsyncCefHost::spawn_stub(CefSettings::default());
        let deadline = std::time::Instant::now() + Duration::from_secs(2);
        while !matches!(host.lifecycle().get(), HostLifecycle::Ready) {
            assert!(std::time::Instant::now() < deadline);
            std::thread::sleep(Duration::from_millis(5));
        }

        let browser_id =
            smol::block_on(host.create_browser("https://example.com", None)).expect("create");
        while host.try_recv_event().is_ok() {}

        smol::block_on(host.navigate(browser_id, "https://example.org")).expect("navigate");

        let deadline = std::time::Instant::now() + Duration::from_secs(2);
        let mut saw_frame = false;
        while std::time::Instant::now() < deadline {
            while let Ok(event) = host.try_recv_event() {
                if matches!(event, CefHostEvent::Frame(_)) {
                    saw_frame = true;
                    break;
                }
            }
            if saw_frame {
                break;
            }
            std::thread::sleep(Duration::from_millis(5));
        }
        assert!(saw_frame, "expected OSR Frame event after stub navigate");
    }

    #[test]
    fn missing_library_load_marks_failed() {
        let host = AsyncCefHost::spawn_with_library_path(
            CefSettings::default(),
            "/nonexistent/path/libcef-does-not-exist",
        );
        let deadline = std::time::Instant::now() + Duration::from_secs(2);
        loop {
            match host.lifecycle().get() {
                HostLifecycle::Failed { message } => {
                    assert!(
                        message.contains("libcef") || message.contains("load"),
                        "unexpected message: {message}"
                    );
                    break;
                }
                HostLifecycle::Ready => panic!("missing library must not become Ready"),
                HostLifecycle::Loading => {
                    assert!(std::time::Instant::now() < deadline);
                    std::thread::sleep(Duration::from_millis(5));
                }
            }
        }
    }
}
