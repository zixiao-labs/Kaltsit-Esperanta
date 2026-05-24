//! GPUI modal that drives the Wuling DevOps device-flow sign-in.
//!
//! The modal owns the entire flow lifecycle: discover → device authorization →
//! poll → persist tokens. State transitions trigger a redraw so the user
//! always sees what's happening. The modal is also responsible for opening the
//! verification URL in the browser and providing a copy button for the code,
//! so the user never has to read it out of a log file.
//!
//! Construction is via [`open_sign_in_modal`], which a workspace action handler
//! calls. The modal handle is not exposed — once shown, dismissal happens via
//! the user pressing Escape, clicking Cancel, or the flow reaching a terminal
//! state and the user pressing Done.
//!
//! On successful sign-in the modal updates the global `WulingAccountState` so
//! observers (title bar chip, etc.) redraw immediately. Persistent state still
//! lives in the keychain via `CredentialsProvider`; the global is a cached
//! view of it.

use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use ama10::auth::{PollResult, Tokens, WulingClient};
use ama10::server_url::ServerUrl;
use anyhow::Result;
use credentials_provider::CredentialsProvider;
use gpui::{
    ClipboardItem, Context, DismissEvent, EventEmitter, FocusHandle, Focusable, MouseDownEvent,
    ParentElement as _, Render, SharedString, Styled as _, Task, Window, div,
};
use ui::{
    ActiveTheme as _, Button, ButtonCommon as _, ButtonSize, ButtonStyle, Clickable as _, Color,
    FixedWidth as _, Headline, HeadlineSize, Icon, IconName, IconSize, InteractiveElement as _,
    IntoElement, Label, LabelCommon as _, LabelSize, StyledExt as _, h_flex, v_flex,
};
use workspace::{ModalView, Workspace};

use crate::settings::WulingConfig;

/// All the states the sign-in modal can be in. The shape is intentionally
/// flat so `match` in `render` reads top-to-bottom as the flow progresses.
enum State {
    /// Hitting `/.well-known/wuling-clients` for the discovery doc.
    Discovering,
    /// Posted `device_authorization`, waiting for the server to mint a code.
    RequestingCode,
    /// Server gave us a user_code. Showing it to the user while polling.
    WaitingForApproval {
        user_code: SharedString,
        verification_uri: SharedString,
        verification_uri_complete: SharedString,
        expires_at: SystemTime,
    },
    /// Tokens persisted; awaiting user to dismiss the modal.
    Success { username: SharedString },
    /// Any failure surfaces here and replaces the previous state.
    Error { message: SharedString },
}

pub struct WulingSignInModal {
    state: State,
    server: ServerUrl,
    focus_handle: FocusHandle,
    _task: Task<()>,
}

impl ModalView for WulingSignInModal {}
impl EventEmitter<DismissEvent> for WulingSignInModal {}

impl Focusable for WulingSignInModal {
    fn focus_handle(&self, _: &gpui::App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

/// Open the modal in the supplied workspace and start the device flow. If the
/// workspace already has another modal up, the existing modal-layer behaviour
/// applies (the previous modal is closed first, then this one shows).
pub fn open_sign_in_modal(
    workspace: &mut Workspace,
    creds: Arc<dyn CredentialsProvider>,
    window: &mut Window,
    cx: &mut Context<Workspace>,
) {
    workspace.toggle_modal(window, cx, |_window, cx| WulingSignInModal::new(creds, cx));
}

impl WulingSignInModal {
    fn new(creds: Arc<dyn CredentialsProvider>, cx: &mut Context<Self>) -> Self {
        let config = WulingConfig::load();
        let server = config.server;
        let tokio_handle = gpui_tokio::Tokio::handle(cx);
        let task = cx.spawn(async move |this, cx| {
            let outcome = run_flow(this.clone(), creds, tokio_handle, cx).await;
            if let Err(err) = outcome {
                this.update(cx, |this, cx| {
                    this.state = State::Error {
                        message: format!("{err:#}").into(),
                    };
                    cx.notify();
                })
                .ok();
            }
        });
        Self {
            state: State::Discovering,
            server,
            focus_handle: cx.focus_handle(),
            _task: task,
        }
    }

    fn dismiss(&mut self, _: &menu::Cancel, _: &mut Window, cx: &mut Context<Self>) {
        cx.emit(DismissEvent);
    }

    fn render_status_label(state: &State) -> SharedString {
        match state {
            State::Discovering => "Contacting server…".into(),
            State::RequestingCode => "Requesting device code…".into(),
            State::WaitingForApproval { .. } => "Waiting for approval".into(),
            State::Success { .. } => "Signed in".into(),
            State::Error { .. } => "Sign-in failed".into(),
        }
    }
}

async fn run_flow(
    this: gpui::WeakEntity<WulingSignInModal>,
    creds: Arc<dyn CredentialsProvider>,
    tokio_handle: tokio::runtime::Handle,
    cx: &mut gpui::AsyncApp,
) -> Result<()> {
    let config = WulingConfig::load();
    let client = WulingClient::new(config.server.clone(), creds, tokio_handle);

    let well_known = client.discover().await?;

    this.update(cx, |this, cx| {
        this.state = State::RequestingCode;
        cx.notify();
    })?;

    let scopes = [
        "user:read",
        "repo:read",
        "issue:read",
        "mr:read",
        "git:read",
        "git:write",
    ];
    let dev = client.device_flow_begin(&well_known, &scopes).await?;
    let expires_at = SystemTime::now() + Duration::from_secs(dev.expires_in);

    this.update(cx, |this, cx| {
        this.state = State::WaitingForApproval {
            user_code: dev.user_code.clone().into(),
            verification_uri: dev.verification_uri.clone().into(),
            verification_uri_complete: dev.verification_uri_complete.clone().into(),
            expires_at,
        };
        cx.notify();
    })?;

    let mut interval_secs = dev.interval.max(1);
    loop {
        if SystemTime::now() > expires_at {
            anyhow::bail!("Verification code expired before approval");
        }
        cx.background_executor()
            .timer(Duration::from_secs(interval_secs))
            .await;
        let poll = client
            .device_flow_poll(&well_known, &dev.device_code)
            .await?;
        match poll {
            PollResult::Pending => continue,
            PollResult::SlowDown => {
                interval_secs = interval_secs.saturating_add(5).min(30);
            }
            PollResult::Denied => anyhow::bail!("Sign-in denied"),
            PollResult::Expired => anyhow::bail!("Verification code expired before approval"),
            PollResult::Issued(tokens) => {
                let username = finalise(&client, cx, &tokens).await?;
                let server_for_state = client.server().clone();
                let username_for_state = username.clone();
                cx.update(|cx| {
                    crate::account_state::set_account(
                        cx,
                        Some(crate::account_state::WulingAccount {
                            username: username_for_state,
                            server: server_for_state,
                        }),
                    );
                });
                this.update(cx, |this, cx| {
                    this.state = State::Success {
                        username: username.into(),
                    };
                    cx.notify();
                })?;
                return Ok(());
            }
        }
    }
}

async fn finalise(
    client: &WulingClient,
    cx: &mut gpui::AsyncApp,
    tokens: &Tokens,
) -> Result<String> {
    let me = client.current_user(&tokens.access_token).await?;
    let expires_at = UNIX_EPOCH
        .elapsed()
        .map(|d| d.as_secs() as i64 + tokens.expires_in as i64)
        .unwrap_or(0);
    client
        .save_credentials(cx, &me.username, tokens, expires_at)
        .await?;
    Ok(me.username)
}

impl Render for WulingSignInModal {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let header = h_flex()
            .gap_2()
            .child(
                Icon::new(IconName::Person)
                    .size(IconSize::Small)
                    .color(Color::Accent),
            )
            .child(Headline::new("Sign in to Wuling DevOps").size(HeadlineSize::Small));

        let server_line = Label::new(self.server.as_str().to_string())
            .size(LabelSize::Small)
            .color(Color::Muted);

        let status = Label::new(Self::render_status_label(&self.state))
            .size(LabelSize::Small)
            .color(Color::Muted);

        let body: gpui::AnyElement = match &self.state {
            State::Discovering | State::RequestingCode => v_flex()
                .gap_2()
                .items_center()
                .child(Label::new("Connecting to the Wuling DevOps server…"))
                .into_any_element(),
            State::WaitingForApproval {
                user_code,
                verification_uri,
                verification_uri_complete,
                expires_at,
            } => {
                let remaining = expires_at
                    .duration_since(SystemTime::now())
                    .map(|d| d.as_secs())
                    .unwrap_or(0);
                let code_for_copy = user_code.to_string();
                let copied = cx
                    .read_from_clipboard()
                    .and_then(|item| item.text())
                    .as_deref()
                    == Some(user_code.as_ref());
                let open_url = verification_uri_complete.to_string();
                let fallback_url = verification_uri.clone();

                v_flex()
                    .gap_3()
                    .child(
                        Label::new("Visit the URL below in your browser, then enter this code:")
                            .color(Color::Muted),
                    )
                    .child(
                        div()
                            .id("user-code-row")
                            .w_full()
                            .px_3()
                            .py_2()
                            .rounded_md()
                            .bg(cx.theme().colors().element_background)
                            .border_1()
                            .border_color(cx.theme().colors().border)
                            .child(
                                h_flex()
                                    .w_full()
                                    .justify_between()
                                    .child(
                                        Headline::new(user_code.clone()).size(HeadlineSize::Large),
                                    )
                                    .child(
                                        Button::new(
                                            "copy-code",
                                            if copied { "Copied!" } else { "Copy" },
                                        )
                                        .size(ButtonSize::Compact)
                                        .style(ButtonStyle::Outlined)
                                        .on_click(
                                            move |_, _, cx| {
                                                cx.write_to_clipboard(ClipboardItem::new_string(
                                                    code_for_copy.clone(),
                                                ));
                                                cx.refresh_windows();
                                            },
                                        ),
                                    ),
                            ),
                    )
                    .child(
                        h_flex()
                            .gap_2()
                            .child(
                                Button::new("open-browser", "Open browser")
                                    .style(ButtonStyle::Filled)
                                    .full_width()
                                    .on_click(move |_, _, cx| cx.open_url(&open_url)),
                            )
                            .child(
                                Button::new("cancel", "Cancel")
                                    .style(ButtonStyle::Subtle)
                                    .on_click(cx.listener(|_, _, _, cx| cx.emit(DismissEvent))),
                            ),
                    )
                    .child(
                        Label::new(format!(
                            "Or visit: {fallback_url}  ·  Code expires in {remaining}s"
                        ))
                        .size(LabelSize::Small)
                        .color(Color::Muted),
                    )
                    .into_any_element()
            }
            State::Success { username } => v_flex()
                .gap_3()
                .child(Label::new(format!("Signed in as {username}.")).color(Color::Success))
                .child(
                    Button::new("done", "Done")
                        .style(ButtonStyle::Filled)
                        .full_width()
                        .on_click(cx.listener(|_, _, _, cx| cx.emit(DismissEvent))),
                )
                .into_any_element(),
            State::Error { message } => v_flex()
                .gap_3()
                .child(Label::new(message.clone()).color(Color::Error))
                .child(
                    Button::new("close", "Close")
                        .style(ButtonStyle::Subtle)
                        .full_width()
                        .on_click(cx.listener(|_, _, _, cx| cx.emit(DismissEvent))),
                )
                .into_any_element(),
        };

        let focus_handle = self.focus_handle.clone();
        v_flex()
            .key_context("WulingSignIn")
            .track_focus(&focus_handle)
            .on_action(cx.listener(Self::dismiss))
            .on_any_mouse_down(cx.listener(|this, _: &MouseDownEvent, window, cx| {
                window.focus(&this.focus_handle, cx);
            }))
            .elevation_3(cx)
            .w(gpui::px(420.0))
            .p_4()
            .gap_3()
            .child(header)
            .child(server_line)
            .child(status)
            .child(
                div()
                    .w_full()
                    .h(gpui::px(1.0))
                    .bg(cx.theme().colors().border_variant),
            )
            .child(body)
    }
}
