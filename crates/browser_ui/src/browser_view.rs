use std::sync::Arc;
use std::time::{Duration, Instant};

use crate::design_mode::DesignModeState;
use ama10_i18n::{tr, tr_f};
use async_host_runtime::HostLifecycle;
use extension_cef::{
    AsyncCefHost, BrowserId, CefBrowserSettings, CefHostEvent, CefSettings, PLACEHOLDER_OSR_STATUS,
    SharedPaintFrame,
};
use gpui::{
    App, Bounds, Context, Entity, EventEmitter, FocusHandle, Focusable, KeyDownEvent, KeyUpEvent,
    MouseDownEvent, MouseMoveEvent, MouseUpEvent, ObjectFit, Pixels, Point, ScrollWheelEvent,
    SharedString, Task, Window, canvas, prelude::*,
};
use project::Project;
use ui::{Icon, IconName, Label, LabelSize, prelude::*};
use workspace::{
    Workspace,
    item::{Item, ItemEvent},
};
pub use zed_actions::browser::{Open, OpenUrl, ToggleDesignMode};

fn normalize_navigation_url(input: &str) -> String {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return "about:blank".to_string();
    }
    if trimmed.contains("://") || trimmed.starts_with("about:") {
        return trimmed.to_string();
    }
    format!("https://{trimmed}")
}

pub struct BrowserView {
    focus_handle: FocusHandle,
    #[allow(dead_code)]
    project: Entity<Project>,
    host: Arc<AsyncCefHost>,
    browser_id: Option<BrowserId>,
    address: SharedString,
    address_field: Entity<ui_input::InputField>,
    title: SharedString,
    status: SharedString,
    latest_frame: Option<SharedPaintFrame>,
    #[cfg(target_os = "macos")]
    surface_frame: Option<core_video::pixel_buffer::CVPixelBuffer>,
    viewport_bounds: Bounds<Pixels>,
    device_scale_factor: f32,
    design_mode: DesignModeState,
    agent_context: SharedString,
    _event_task: Task<()>,
    _bootstrap_task: Task<()>,
}

impl BrowserView {
    pub fn new(
        project: Entity<Project>,
        initial_url: Option<String>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let address = SharedString::from(initial_url.unwrap_or_else(|| "about:blank".to_string()));
        let address_field = cx.new(|cx| {
            let field = ui_input::InputField::new(window, cx, tr!("Enter URL").as_ref());
            field.set_text(address.as_ref(), window, cx);
            field
        });
        // Prefer a managed/system libcef; fall back to stub so CI and fresh installs
        // stay fail-soft. Loading always happens on the CEF host thread.
        let host = Arc::new(match extension_cef::probe_libcef_path() {
            Some(path) => AsyncCefHost::spawn_with_library_path(CefSettings::default(), path),
            None => AsyncCefHost::spawn_stub(CefSettings::default()),
        });
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
        let initial_scale = window.scale_factor();
        let bootstrap_task = cx.spawn(async move |this, cx| {
            let deadline = Instant::now() + Duration::from_secs(10);
            loop {
                match host_for_boot.lifecycle().get() {
                    HostLifecycle::Ready => break,
                    HostLifecycle::Failed { message } => {
                        this.update(cx, |this, cx| {
                            this.status = tr_f!("Browser unavailable: {}", message);
                            cx.notify();
                        })
                        .ok();
                        return;
                    }
                    HostLifecycle::Loading => {
                        if Instant::now() > deadline {
                            this.update(cx, |this, cx| {
                                this.status = tr!("Timed out waiting for browser host");
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

            let browser_settings = this
                .read_with(cx, |this, _cx| {
                    let width = f32::from(this.viewport_bounds.size.width).round() as i32;
                    let height = f32::from(this.viewport_bounds.size.height).round() as i32;
                    CefBrowserSettings {
                        view_width: if width > 0 { width } else { 960 },
                        view_height: if height > 0 { height } else { 540 },
                        device_scale_factor: this.device_scale_factor,
                        ..Default::default()
                    }
                })
                .unwrap_or_else(|_| CefBrowserSettings {
                    device_scale_factor: initial_scale,
                    ..Default::default()
                });

            match host_for_boot
                .create_browser(boot_url.clone(), Some(browser_settings))
                .await
            {
                Ok(id) => {
                    this.update(cx, |this, cx| {
                        this.browser_id = Some(id);
                        this.status = tr!("Ready");
                        this.sync_viewport_to_host();
                        cx.notify();
                    })
                    .ok();
                }
                Err(error) => {
                    this.update(cx, |this, cx| {
                        this.status = tr_f!("Failed to create browser: {}", format!("{error:#}"));
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
            address_field,
            title: tr!("Browser"),
            status: tr!("Loading browser host…"),
            latest_frame: None,
            #[cfg(target_os = "macos")]
            surface_frame: None,
            viewport_bounds: Bounds::default(),
            device_scale_factor: initial_scale,
            design_mode: DesignModeState::default(),
            agent_context: SharedString::default(),
            _event_task: event_task,
            _bootstrap_task: bootstrap_task,
        }
    }

    fn sync_viewport_to_host(&self) {
        let Some(id) = self.browser_id else {
            return;
        };
        let width = f32::from(self.viewport_bounds.size.width).round() as i32;
        let height = f32::from(self.viewport_bounds.size.height).round() as i32;
        if width <= 0 || height <= 0 {
            return;
        }
        self.host
            .resize_browser(id, width, height, self.device_scale_factor)
            .ok();
    }

    fn update_viewport_geometry(&mut self, bounds: Bounds<Pixels>, scale_factor: f32) {
        let size_changed = self.viewport_bounds.size != bounds.size;
        let scale_changed = (self.device_scale_factor - scale_factor).abs() > f32::EPSILON;
        self.viewport_bounds = bounds;
        self.device_scale_factor = scale_factor;
        if size_changed || scale_changed {
            self.sync_viewport_to_host();
        }
    }

    fn view_origin(&self) -> Point<Pixels> {
        self.viewport_bounds.origin
    }

    fn confirm_navigation(
        &mut self,
        _: &menu::Confirm,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let url = self.address_field.read(cx).text(cx);
        let url = normalize_navigation_url(&url);
        self.navigate_to(url, cx);
    }

    fn sync_address_field_from_state(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let field_focused = self
            .address_field
            .focus_handle(cx)
            .contains_focused(window, cx);
        if field_focused {
            return;
        }
        let current = self.address_field.read(cx).text(cx);
        if current == self.address.as_ref() {
            return;
        }
        self.address_field.update(cx, |field, cx| {
            field.set_text(self.address.as_ref(), window, cx);
        });
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
        self.status = tr!("Navigating…");
        cx.notify();

        let host = self.host.clone();
        let browser_id = self.browser_id;
        cx.spawn(async move |this, cx| {
            let Some(id) = browser_id else {
                return;
            };
            if let Err(error) = host.navigate(id, url).await {
                this.update(cx, |this, cx| {
                    this.status = tr_f!("Navigate failed: {}", format!("{error:#}"));
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
                self.status = tr_f!("Loading {}…", url);
            }
            CefHostEvent::LoadEnd { http_status, .. } => {
                self.status = tr_f!("Loaded ({})", http_status);
            }
            CefHostEvent::LoadError { message, .. } => {
                if message == PLACEHOLDER_OSR_STATUS {
                    self.status = tr!("Paint placeholder — CEF OSR not wired yet");
                } else {
                    self.status = tr_f!("Load error: {}", message);
                }
            }
            CefHostEvent::Frame(frame) => {
                #[cfg(target_os = "macos")]
                {
                    match frame.to_cv_pixel_buffer() {
                        Ok(buffer) => self.surface_frame = Some(buffer),
                        Err(error) => {
                            log::warn!("CEF frame → CVPixelBuffer failed: {error:#}");
                        }
                    }
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
        let viewport_measure = {
            let entity = cx.entity().downgrade();
            canvas(
                move |bounds, window, cx| {
                    let scale_factor = window.scale_factor();
                    entity
                        .update(cx, |this, _cx| {
                            this.update_viewport_geometry(bounds, scale_factor);
                        })
                        .ok();
                },
                |_bounds, (), _window, _cx| {},
            )
            .size_full()
            .absolute()
        };

        // Capture relative to the viewport; Design Mode consumes input itself.
        div()
            .id("browser-viewport")
            .size_full()
            .track_focus(&self.focus_handle)
            .on_any_mouse_down(cx.listener(|this, event: &MouseDownEvent, window, cx| {
                this.focus_handle.focus(window, cx);
                if let Some(id) = this.browser_id {
                    this.host.set_focus(id, true).ok();
                }
                if this.design_mode.enabled {
                    this.handle_design_mouse_down(event, cx);
                    return;
                }
                let Some(id) = this.browser_id else {
                    return;
                };
                let origin = this.view_origin();
                crate::input_bridge::forward_mouse_down(&this.host, id, event, origin);
            }))
            .on_mouse_up(gpui::MouseButton::Left, cx.listener(Self::handle_mouse_up))
            .on_mouse_up(
                gpui::MouseButton::Middle,
                cx.listener(Self::handle_mouse_up),
            )
            .on_mouse_up(gpui::MouseButton::Right, cx.listener(Self::handle_mouse_up))
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
                let origin = this.view_origin();
                crate::input_bridge::forward_mouse_move(&this.host, id, event, origin);
            }))
            .on_scroll_wheel(cx.listener(|this, event: &ScrollWheelEvent, _window, _cx| {
                if this.design_mode.enabled {
                    return;
                }
                let Some(id) = this.browser_id else {
                    return;
                };
                let origin = this.view_origin();
                crate::input_bridge::forward_scroll(&this.host, id, event, origin);
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
            .child(viewport_measure)
            .child(content)
            .children(self.render_design_overlays())
            .into_any_element()
    }

    fn handle_mouse_up(
        &mut self,
        event: &MouseUpEvent,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.design_mode.enabled {
            if event.modifiers.shift {
                self.design_mode.finish_region();
                self.refresh_agent_context();
                cx.notify();
            }
            return;
        }
        let Some(id) = self.browser_id else {
            return;
        };
        let origin = self.view_origin();
        crate::input_bridge::forward_mouse_up(&self.host, id, event, origin);
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
            // Fill the DIP viewport; CEF paints physical pixels via device_scale_factor.
            return gpui::surface(frame.clone())
                .object_fit(ObjectFit::Fill)
                .size_full()
                .into_any_element();
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
    view.update(cx, |view, cx| {
        view.address_field.focus_handle(cx).focus(window, cx);
    });
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
            tr!("Browser")
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
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let design_label = if self.design_mode.enabled {
            tr!("Design Mode: On")
        } else {
            tr!("Design Mode: Off")
        };

        self.sync_address_field_from_state(window, cx);

        v_flex()
            .size_full()
            .key_context("Browser")
            .track_focus(&self.focus_handle(cx))
            .on_action(cx.listener(Self::confirm_navigation))
            .child(
                h_flex()
                    .w_full()
                    .gap_2()
                    .px_2()
                    .py_1()
                    .border_b_1()
                    .border_color(cx.theme().colors().border)
                    .child(
                        div()
                            .id("browser-address-bar")
                            .flex_1()
                            .min_w_0()
                            .child(self.address_field.clone()),
                    )
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
