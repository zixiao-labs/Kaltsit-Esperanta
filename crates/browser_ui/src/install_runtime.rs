use std::sync::Arc;

use extension_cef::{
    install_managed_cef, probe_managed_libcef_path, refresh_managed_cef as refresh_managed_cef_lib,
};
use gpui::{AppContext as _, AsyncApp, Context, Window, actions};
use http_client::HttpClient;
use workspace::notifications::{DetachAndPromptErr, NotificationId};
use workspace::{Toast, Workspace};

actions!(
    browser,
    [
        /// Downloads the managed Chromium Embedded Framework runtime.
        InstallBrowserRuntime,
    ]
);

pub fn install_browser_runtime(window: &mut Window, cx: &mut Context<Workspace>) {
    cx.spawn_in(window, async move |workspace, cx| {
        if let Some(path) = probe_managed_libcef_path() {
            workspace.update(cx, |workspace, cx| {
                workspace.show_toast(
                    Toast::new(
                        NotificationId::unique::<AlreadyInstalled>(),
                        format!("Browser runtime already installed at {}", path.display()),
                    ),
                    cx,
                );
            })?;
            return Ok(());
        }

        let http = workspace.read_with(cx, |workspace, _cx| workspace.client().http_client())?;
        let http: Arc<dyn HttpClient> = http;

        workspace.update(cx, |workspace, cx| {
            workspace.show_toast(
                Toast::new(
                    NotificationId::unique::<Installing>(),
                    "Downloading browser runtime…",
                ),
                cx,
            );
        })?;

        let path = cx
            .background_spawn(async move { install_managed_cef(http).await })
            .await?;

        workspace.update(cx, |workspace, cx| {
            workspace.show_toast(
                Toast::new(
                    NotificationId::unique::<Installed>(),
                    format!(
                        "Installed browser runtime to {}. Open a Browser tab to use it.",
                        path.display()
                    ),
                ),
                cx,
            );
        })?;
        Ok(())
    })
    .detach_and_prompt_err(
        "Cannot install the browser runtime",
        window,
        cx,
        |error, _, _| Some(format!("{error:#}")),
    );
}

/// Background refresh used by component-only auto-update (never full-app).
pub fn refresh_managed_cef(http: Arc<dyn HttpClient>, cx: &AsyncApp) -> gpui::Task<()> {
    cx.background_spawn(async move {
        match refresh_managed_cef_lib(http).await {
            Ok(Some(path)) => log::info!("CEF runtime ready at {}", path.display()),
            Ok(None) => log::debug!("CEF runtime not installed; skipping component refresh"),
            Err(error) => log::warn!("CEF runtime refresh skipped: {error:#}"),
        }
    })
}

struct AlreadyInstalled;
struct Installing;
struct Installed;
