//! Embedded browser editor tab (frontend enhancement surface).

mod browser_view;

pub use browser_view::{BrowserView, Open, OpenUrl, open_or_reuse_browser};

use gpui::App;

pub fn init(cx: &mut App) {
    cx.observe_new(|workspace: &mut workspace::Workspace, _, _| {
        workspace.register_action(|workspace, _: &Open, window, cx| {
            open_or_reuse_browser(workspace, None, window, cx);
        });
        workspace.register_action(|workspace, action: &OpenUrl, window, cx| {
            open_or_reuse_browser(workspace, Some(action.url.clone()), window, cx);
        });
    })
    .detach();
}
