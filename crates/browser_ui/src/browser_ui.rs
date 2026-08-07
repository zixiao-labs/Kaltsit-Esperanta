//! Embedded browser editor tab (frontend enhancement surface).

mod browser_view;
mod design_mode;
mod input_bridge;

pub use browser_view::{BrowserView, Open, OpenUrl, ToggleDesignMode, open_or_reuse_browser};
pub use design_mode::{DesignModeState, DesignSelection};

use gpui::App;

pub fn init(cx: &mut App) {
    cx.observe_new(|workspace: &mut workspace::Workspace, _, _| {
        workspace.register_action(|workspace, _: &Open, window, cx| {
            open_or_reuse_browser(workspace, None, window, cx);
        });
        workspace.register_action(|workspace, action: &OpenUrl, window, cx| {
            open_or_reuse_browser(workspace, Some(action.url.clone()), window, cx);
        });
        workspace.register_action(|workspace, _: &ToggleDesignMode, window, cx| {
            if let Some(view) = workspace.active_item_as::<BrowserView>(cx) {
                view.update(cx, |view, cx| view.toggle_design_mode(cx));
                workspace.activate_item(&view, true, true, window, cx);
            } else {
                open_or_reuse_browser(workspace, None, window, cx);
                if let Some(view) = workspace.item_of_type::<BrowserView>(cx) {
                    view.update(cx, |view, cx| view.set_design_mode(true, cx));
                }
            }
        });
    })
    .detach();
}
