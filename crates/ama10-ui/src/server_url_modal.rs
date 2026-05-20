//! Workspace modal for editing the persisted Wuling DevOps server URL.
//!
//! Standalone from `sign_in_modal` because the two are independently useful:
//! the user might want to change which Wuling instance Esperanta talks to
//! without immediately signing in (or while signed in to a different one).
//! Mutating `WulingConfig` writes through to `wuling.json`; the sign-in flow
//! always re-reads the file so the next sign-in attempt picks up the change.

use ama10::server_url::ServerUrl;
use editor::Editor;
use gpui::{
    AppContext as _, Context, DismissEvent, Entity, EventEmitter, FocusHandle, Focusable,
    MouseDownEvent, ParentElement as _, Render, SharedString, Styled as _, Window, div,
};
use ui::{
    ActiveTheme as _, Button, ButtonCommon as _, ButtonStyle, Clickable as _, Color,
    FixedWidth as _, Headline, HeadlineSize, InteractiveElement as _, IntoElement, Label,
    LabelCommon as _, LabelSize, StyledExt as _, h_flex, v_flex,
};
use workspace::{ModalView, Workspace};

use crate::settings::WulingConfig;

pub struct WulingServerUrlModal {
    editor: Entity<Editor>,
    error: Option<SharedString>,
    focus_handle: FocusHandle,
}

impl ModalView for WulingServerUrlModal {}
impl EventEmitter<DismissEvent> for WulingServerUrlModal {}

impl Focusable for WulingServerUrlModal {
    fn focus_handle(&self, cx: &gpui::App) -> FocusHandle {
        self.editor.focus_handle(cx)
    }
}

pub fn open_server_url_modal(
    workspace: &mut Workspace,
    window: &mut Window,
    cx: &mut Context<Workspace>,
) {
    workspace.toggle_modal(window, cx, |window, cx| {
        WulingServerUrlModal::new(window, cx)
    });
}

impl WulingServerUrlModal {
    fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let current = WulingConfig::load().server;
        let editor = cx.new(|cx| {
            let mut editor = Editor::single_line(window, cx);
            editor.set_placeholder_text(ama10::server_url::DEFAULT_SERVER_URL, window, cx);
            editor.set_text(current.as_str(), window, cx);
            editor
        });
        let focus_handle = editor.focus_handle(cx);
        Self {
            editor,
            error: None,
            focus_handle,
        }
    }

    fn cancel(&mut self, _: &menu::Cancel, _: &mut Window, cx: &mut Context<Self>) {
        cx.emit(DismissEvent);
    }

    fn save(&mut self, _: &menu::Confirm, _: &mut Window, cx: &mut Context<Self>) {
        let text = self.editor.read(cx).text(cx);
        let parsed = match ServerUrl::parse(&text) {
            Ok(server) => server,
            Err(err) => {
                self.error = Some(format!("{err}").into());
                cx.notify();
                return;
            }
        };
        let mut config = WulingConfig::load();
        config.server = parsed;
        match config.save() {
            Ok(()) => cx.emit(DismissEvent),
            Err(err) => {
                self.error = Some(format!("Save failed: {err:#}").into());
                cx.notify();
            }
        }
    }
}

impl Render for WulingServerUrlModal {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let focus_handle = self.focus_handle.clone();
        v_flex()
            .key_context("WulingServerUrl")
            .track_focus(&focus_handle)
            .on_action(cx.listener(Self::cancel))
            .on_action(cx.listener(Self::save))
            .on_any_mouse_down(cx.listener(|this, _: &MouseDownEvent, window, cx| {
                window.focus(&this.focus_handle, cx);
            }))
            .elevation_3(cx)
            .w(gpui::px(420.0))
            .p_4()
            .gap_3()
            .child(Headline::new("Wuling DevOps server URL").size(HeadlineSize::Small))
            .child(
                Label::new(format!(
                    "Default: {}",
                    ama10::server_url::DEFAULT_SERVER_URL
                ))
                .size(LabelSize::Small)
                .color(Color::Muted),
            )
            .child(
                div()
                    .px_2()
                    .py_1()
                    .rounded_md()
                    .border_1()
                    .border_color(cx.theme().colors().border)
                    .bg(cx.theme().colors().editor_background)
                    .child(self.editor.clone()),
            )
            .children(
                self.error
                    .clone()
                    .map(|msg| Label::new(msg).color(Color::Error)),
            )
            .child(
                h_flex()
                    .gap_2()
                    .child(
                        Button::new("save", "Save")
                            .style(ButtonStyle::Filled)
                            .full_width()
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.save(&menu::Confirm, window, cx);
                            })),
                    )
                    .child(
                        Button::new("cancel", "Cancel")
                            .style(ButtonStyle::Subtle)
                            .on_click(cx.listener(|_, _, _, cx| cx.emit(DismissEvent))),
                    ),
            )
    }
}
