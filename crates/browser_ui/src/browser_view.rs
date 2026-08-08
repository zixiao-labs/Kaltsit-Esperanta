use std::sync::Arc;
use std::time::{Duration, Instant};

use crate::design_mode::DesignModeState;
use async_host_runtime::HostLifecycle;
use extension_cef::{AsyncCefHost, BrowserId, CefHostEvent, CefSettings, SharedPaintFrame};
use gpui::{
    App, Context, Entity, EventEmitter, FocusHandle, Focusable, KeyDownEvent, KeyUpEvent,
    MouseDownEvent, MouseMoveEvent, MouseUpEvent, ScrollWheelEvent, SharedString, Task, Window,
    actions, prelude::*,
};
use project::Project;
use ui::{Icon, IconName, Label, LabelSize, prelude::*};
#[cfg(target_os = "macos")]
use util::ResultExt as _;
use workspace::{
    Workspace,
    item::{Item, ItemEvent},
};

actions!(
    browser,
    [
        /// Open the embedded browser in an editor tab.
        Open,
        /// Toggle Design Mode for the active browser tab.
        ToggleDesignMode,
    ]
);

/// Open the embedded browser at a specific URL.
#[derive(Clone, PartialEq, serde::Deserialize, schemars::JsonSchema, gpui::Action)]
#[action(namespace = browser)]
pub struct OpenUrl {
    pub url: String,
}

pub struct BrowserView {
    focus_handle: FocusHandle,
    #[allow(dead_code)]
    project: Entity<Project>,
    host: Arc<AsyncCefHost>,
    browser_id: Option<BrowserId>,
    address: SharedString,
    title: SharedString,
    status: SharedString,
    latest_frame: Option<SharedPaintFrame>,
    #[cfg(target_os = "macos")]
    surface_frame: Option<core_video::pixel_buffer::CVPixelBuffer>,
    design_mode: DesignModeState,
    agent_context: SharedString,
    _event_task: Task<()>,
    _bootstrap_task: Task<()>,
}

impl BrowserView {
    pub fn new(
        project: Entity<Project>,
        initial_url: Option<String>,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let address = SharedString::from(initial_url.unwrap_or_else(|| "about:blank".to_string()));
        // Stub host until a real libcef is available; never dlopen on the UI thread.
        let host = Arc::new(AsyncCefHost::spawn_stub(CefSettings::default()));
        let event_rx = host.event_receiver();

        let event_task = cx.spawn(async move |this, cx| {
            while let Ok(event) = event_rx.recv().await {
                if this
                    .update(cx, |this, cx| {
                        this.handle_host_event(event, cx);
                    })
                    .is_err()
                {
                    break;
                }
            }
        });

        let host_for_boot = host.clone();
        let boot_url = address.to_string();
        let bootstrap_task = cx.spawn(async move |this, cx| {
            let deadline = Instant::now() + Duration::from_secs(10);
            loop {
                match host_for_boot.lifecycle().get() {
                    HostLifecycle::Ready => break,
                    HostLifecycle::Failed { message } => {
                        this.update(cx, |this, cx| {
                            this.status =
                                SharedString::from(format!("Browser unavailable: {message}"));
                            cx.notify();
                        })
                        .ok();
                        return;
                    }
                    HostLifecycle::Loading => {
                        if Instant::now() > deadline {
                            this.update(cx, |this, cx| {
                                this.status =
                                    SharedString::from("Timed out waiting for browser host");
                                cx.notify();
                            })
                            .ok();
                            return;
                        }
                        cx.background_executor()
                            .timer(Duration::from_millis(16))
                            .await;
                    }
                }
            }

            match host_for_boot.create_browser(boot_url.clone(), None).await {
                Ok(id) => {
                    this.update(cx, |this, cx| {
                        this.browser_id = Some(id);
                        this.status = SharedString::from("Ready");
                        cx.notify();
                    })
                    .ok();
                }
                Err(error) => {
                    this.update(cx, |this, cx| {
                        this.status =
                            SharedString::from(format!("Failed to create browser: {error:#}"));
                        cx.notify();
                    })
                    .ok();
                }
            }
        });

        Self {
            focus_handle: cx.focus_handle(),
            project,
            host,
            browser_id: None,
            address,
            title: SharedString::from("Browser"),
            status: SharedString::from("Loading browser host…"),
            latest_frame: None,
            #[cfg(target_os = "macos")]
            surface_frame: None,
            design_mode: DesignModeState::default(),
            agent_context: SharedString::default(),
            _event_task: event_task,
            _bootstrap_task: bootstrap_task,
        }
    }

    pub fn host(&self) -> &Arc<AsyncCefHost> {
        &self.host
    }

    pub fn browser_id(&self) -> Option<BrowserId> {
        self.browser_id
    }

    pub fn design_mode_enabled(&self) -> bool {
        self.design_mode.enabled
    }

    pub fn agent_context_markdown(&self) -> SharedString {
        self.agent_context.clone()
    }

    pub fn set_design_mode(&mut self, enabled: bool, cx: &mut Context<Self>) {
        self.design_mode.enabled = enabled;
        if !enabled {
            self.design_mode.clear();
            self.agent_context = SharedString::default();
        }
        cx.notify();
    }

    pub fn toggle_design_mode(&mut self, cx: &mut Context<Self>) {
        self.set_design_mode(!self.design_mode.enabled, cx);
    }

    fn refresh_agent_context(&mut self) {
        self.agent_context = SharedString::from(self.design_mode.agent_context_markdown());
    }

    pub fn navigate_to(&mut self, url: String, cx: &mut Context<Self>) {
        self.address = SharedString::from(url.clone());
        self.status = SharedString::from("Navigating…");
        cx.notify();

        let host = self.host.clone();
        let browser_id = self.browser_id;
        cx.spawn(async move |this, cx| {
            let Some(id) = browser_id else {
                return;
            };
            if let Err(error) = host.navigate(id, url).await {
                this.update(cx, |this, cx| {
                    this.status = SharedString::from(format!("Navigate failed: {error:#}"));
                    cx.notify();
                })
                .ok();
            }
        })
        .detach();
    }

    fn handle_host_event(&mut self, event: CefHostEvent, cx: &mut Context<Self>) {
        match event {
            CefHostEvent::AddressChanged { url, .. } => {
                self.address = SharedString::from(url);
            }
            CefHostEvent::TitleChanged { title, .. } => {
                self.title = SharedString::from(title);
                cx.emit(ItemEvent::UpdateTab);
            }
            CefHostEvent::LoadStart { url, .. } => {
                self.status = SharedString::from(format!("Loading {url}…"));
            }
            CefHostEvent::LoadEnd { http_status, .. } => {
                self.status = SharedString::from(format!("Loaded ({http_status})"));
            }
            CefHostEvent::LoadError { message, .. } => {
                self.status = SharedString::from(format!("Load error: {message}"));
            }
            CefHostEvent::Frame(frame) => {
                #[cfg(target_os = "macos")]
                {
                    self.surface_frame = frame.to_cv_pixel_buffer().log_err();
                }
                self.latest_frame = Some(frame);
            }
            CefHostEvent::Paint(_)
            | CefHostEvent::BrowserCreated(_)
            | CefHostEvent::BrowserClosed(_) => {}
        }
        cx.notify();
    }

    fn render_viewport(&self, cx: &mut Context<Self>) -> AnyElement {
        let content = self.render_viewport_content();

        // Capture relative to the viewport; Design Mode consumes input itself.
        div()
            .id("browser-viewport")
            .size_full()
            .track_focus(&self.focus_handle)
            .on_mouse_down(
                gpui::MouseButton::Left,
                cx.listener(|this, event: &MouseDownEvent, _window, cx| {
                    if this.design_mode.enabled {
                        this.handle_design_mouse_down(event, cx);
                        return;
                    }
                    let Some(id) = this.browser_id else {
                        return;
                    };
                    crate::input_bridge::forward_mouse_down(&this.host, id, event);
                }),
            )
            .on_mouse_up(
                gpui::MouseButton::Left,
                cx.listener(|this, event: &MouseUpEvent, _window, cx| {
                    if this.design_mode.enabled {
                        if event.modifiers.shift {
                            this.design_mode.finish_region();
                            this.refresh_agent_context();
                            cx.notify();
                        }
                        return;
                    }
                    let Some(id) = this.browser_id else {
                        return;
                    };
                    crate::input_bridge::forward_mouse_up(&this.host, id, event);
                }),
            )
            .on_mouse_move(cx.listener(|this, event: &MouseMoveEvent, _window, cx| {
                if this.design_mode.enabled {
                    if event.modifiers.shift {
                        this.design_mode.update_region(event.position);
                        cx.notify();
                    }
                    return;
                }
                let Some(id) = this.browser_id else {
                    return;
                };
                crate::input_bridge::forward_mouse_move(&this.host, id, event);
            }))
            .on_scroll_wheel(cx.listener(|this, event: &ScrollWheelEvent, _window, _cx| {
                if this.design_mode.enabled {
                    return;
                }
                let Some(id) = this.browser_id else {
                    return;
                };
                crate::input_bridge::forward_scroll(&this.host, id, event);
            }))
            .on_key_down(cx.listener(|this, event: &KeyDownEvent, _window, _cx| {
                if this.design_mode.enabled {
                    return;
                }
                let Some(id) = this.browser_id else {
                    return;
                };
                crate::input_bridge::forward_key_down(&this.host, id, event);
            }))
            .on_key_up(cx.listener(|this, event: &KeyUpEvent, _window, _cx| {
                if this.design_mode.enabled {
                    return;
                }
                let Some(id) = this.browser_id else {
                    return;
                };
                crate::input_bridge::forward_key_up(&this.host, id, event);
            }))
            .child(content)
            .children(self.render_design_overlays())
            .into_any_element()
    }

    fn handle_design_mouse_down(&mut self, event: &MouseDownEvent, cx: &mut Context<Self>) {
        if event.modifiers.shift {
            self.design_mode.begin_region(event.position);
            cx.notify();
            return;
        }

        let x: f32 = event.position.x.into();
        let y: f32 = event.position.y.into();
        let node = extension_cef::DesignNodeInfo {
            xpath: format!("//*[@data-x='{x}'][@data-y='{y}']"),
            tag: "div".into(),
            attributes: vec![("data-design-mode".into(), "true".into())],
            computed_style_summary: "/* stub DOM probe — wire CDP Overlay next */".into(),
            x,
            y,
        };
        let additive = event.modifiers.alt || event.modifiers.secondary();
        self.design_mode.select_node(node, additive);
        self.refresh_agent_context();
        cx.notify();
    }

    fn render_design_overlays(&self) -> Vec<AnyElement> {
        if !self.design_mode.enabled {
            return Vec::new();
        }
        let mut overlays = Vec::new();
        if let Some(bounds) = self.design_mode.region_bounds_px() {
            overlays.push(
                div()
                    .absolute()
                    .left(bounds.origin.x)
                    .top(bounds.origin.y)
                    .w(bounds.size.width)
                    .h(bounds.size.height)
                    .border_2()
                    .border_color(gpui::blue())
                    .bg(gpui::rgba(0x3b82f633))
                    .into_any_element(),
            );
        }
        for selection in &self.design_mode.selected {
            overlays.push(
                div()
                    .absolute()
                    .left(gpui::px(selection.node.x))
                    .top(gpui::px(selection.node.y))
                    .w(gpui::px(24.))
                    .h(gpui::px(24.))
                    .border_2()
                    .border_color(gpui::green())
                    .bg(gpui::rgba(0x22c55e33))
                    .into_any_element(),
            );
        }
        overlays
    }

    fn render_viewport_content(&self) -> AnyElement {
        #[cfg(target_os = "macos")]
        if let Some(frame) = &self.surface_frame {
            return gpui::surface(frame.clone()).size_full().into_any_element();
        }

        if let Some(frame) = &self.latest_frame {
            return self.render_frame_placeholder(frame);
        }

        self.render_status_placeholder()
    }

    fn render_frame_placeholder(&self, frame: &SharedPaintFrame) -> AnyElement {
        div()
            .size_full()
            .flex()
            .items_center()
            .justify_center()
            .child(
                Label::new(format!(
                    "Frame {}×{} ({} bytes) — surface unavailable on this platform",
                    frame.width,
                    frame.height,
                    frame.bgra.len()
                ))
                .size(LabelSize::Small),
            )
            .into_any_element()
    }

    fn render_status_placeholder(&self) -> AnyElement {
        div()
            .size_full()
            .flex()
            .items_center()
            .justify_center()
            .child(Label::new(self.status.clone()).size(LabelSize::Small))
            .into_any_element()
    }
}

pub fn open_or_reuse_browser(
    workspace: &mut Workspace,
    url: Option<String>,
    window: &mut Window,
    cx: &mut Context<Workspace>,
) {
    if let Some(existing) = workspace.item_of_type::<BrowserView>(cx) {
        if let Some(url) = url {
            existing.update(cx, |view, cx| view.navigate_to(url, cx));
        }
        workspace.activate_item(&existing, true, true, window, cx);
        return;
    }

    let project = workspace.project().clone();
    let view = cx.new(|cx| BrowserView::new(project, url, window, cx));
    workspace.add_item_to_active_pane(Box::new(view), None, true, window, cx);
}

impl EventEmitter<ItemEvent> for BrowserView {}

impl Focusable for BrowserView {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl Item for BrowserView {
    type Event = ItemEvent;

    fn tab_icon(&self, _window: &Window, _cx: &App) -> Option<Icon> {
        Some(Icon::new(IconName::ToolWeb))
    }

    fn tab_content_text(&self, _detail: usize, _cx: &App) -> SharedString {
        if self.title.is_empty() {
            SharedString::from("Browser")
        } else {
            self.title.clone()
        }
    }

    fn tab_tooltip_text(&self, _cx: &App) -> Option<SharedString> {
        Some(self.address.clone())
    }

    fn to_item_events(event: &Self::Event, f: &mut dyn FnMut(ItemEvent)) {
        f(*event)
    }

    fn show_toolbar(&self) -> bool {
        false
    }
}

impl Render for BrowserView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let design_label = if self.design_mode.enabled {
            "Design Mode: On"
        } else {
            "Design Mode: Off"
        };

        v_flex()
            .size_full()
            .key_context("Browser")
            .track_focus(&self.focus_handle(cx))
            .child(
                h_flex()
                    .w_full()
                    .gap_2()
                    .px_2()
                    .py_1()
                    .border_b_1()
                    .border_color(cx.theme().colors().border)
                    .child(Label::new(self.address.clone()).size(LabelSize::Small))
                    .child(div().flex_1())
                    .child(
                        Button::new("toggle-design-mode", design_label).on_click(cx.listener(
                            |this, _, _window, cx| {
                                this.toggle_design_mode(cx);
                            },
                        )),
                    ),
            )
            .child(
                h_flex()
                    .w_full()
                    .px_2()
                    .py_0p5()
                    .child(Label::new(self.status.clone()).size(LabelSize::XSmall)),
            )
            .when(
                self.design_mode.enabled && !self.agent_context.is_empty(),
                |this| {
                    this.child(
                        div()
                            .w_full()
                            .max_h(gpui::px(120.))
                            .overflow_hidden()
                            .px_2()
                            .py_1()
                            .border_b_1()
                            .border_color(cx.theme().colors().border)
                            .child(Label::new(self.agent_context.clone()).size(LabelSize::XSmall)),
                    )
                },
            )
            .child(
                div()
                    .flex_1()
                    .w_full()
                    .relative()
                    .child(self.render_viewport(cx)),
            )
    }
}
