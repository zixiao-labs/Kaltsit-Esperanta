//! GPUI modal that drives connector device-flow sign-in.
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
//! On successful sign-in the modal updates the global connector account state
//! so observers redraw immediately. Persistent state still
//! lives in the keychain via `CredentialsProvider`; the global is a cached
//! view of it.

use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use ama10::auth::{PollResult, Tokens, WulingClient};
use ama10::connector::{ConnectorAccount, ConnectorId};
use ama10::github::{GithubClient, GithubPollResult};
use ama10_i18n::{tr, tr_f};
use anyhow::{Context as _, Result};
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

use crate::settings::ConnectorSettings;
use settings::Settings as _;

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

pub struct ConnectorSignInModal {
    state: State,
    connector: ConnectorId,
    server_label: SharedString,
    focus_handle: FocusHandle,
    _task: Task<()>,
}

impl ModalView for ConnectorSignInModal {}
impl EventEmitter<DismissEvent> for ConnectorSignInModal {}

impl Focusable for ConnectorSignInModal {
    fn focus_handle(&self, _: &gpui::App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

/// Open the modal in the supplied workspace and start the device flow. If the
/// workspace already has another modal up, the existing modal-layer behaviour
/// applies (the previous modal is closed first, then this one shows).
pub fn open_sign_in_modal(
    workspace: &mut Workspace,
    connector: ConnectorId,
    creds: Arc<dyn CredentialsProvider>,
    window: &mut Window,
    cx: &mut Context<Workspace>,
) {
    workspace.toggle_modal(window, cx, |_window, cx| {
        ConnectorSignInModal::new(connector, creds, cx)
    });
}

impl ConnectorSignInModal {
    fn new(
        connector: ConnectorId,
        creds: Arc<dyn CredentialsProvider>,
        cx: &mut Context<Self>,
    ) -> Self {
        let settings = ConnectorSettings::get_global(cx);
        let server_label = match connector {
            ConnectorId::Wuling => settings.wuling_server.as_str(),
            ConnectorId::Github => "github.com",
        }
        .to_string()
        .into();
        let tokio_handle = gpui_tokio::Tokio::handle(cx);
        let task = cx.spawn(async move |this, cx| {
            let outcome = run_flow(this.clone(), connector, creds, tokio_handle, cx).await;
            if let Err(err) = outcome {
                if let Err(update_error) = this.update(cx, |this, cx| {
                    this.state = State::Error {
                        message: format!("{err:#}").into(),
                    };
                    cx.notify();
                }) {
                    log::debug!("ama10: sign-in modal was dismissed: {update_error}");
                }
            }
        });
        Self {
            state: State::Discovering,
            connector,
            server_label,
            focus_handle: cx.focus_handle(),
            _task: task,
        }
    }

    fn dismiss(&mut self, _: &menu::Cancel, _: &mut Window, cx: &mut Context<Self>) {
        cx.emit(DismissEvent);
    }

    fn render_status_label(state: &State) -> SharedString {
        match state {
            State::Discovering => tr!("Contacting server…"),
            State::RequestingCode => tr!("Requesting device code…"),
            State::WaitingForApproval { .. } => tr!("Waiting for approval"),
            State::Success { .. } => tr!("Signed in"),
            State::Error { .. } => tr!("Sign-in failed"),
        }
    }
}

async fn run_flow(
    this: gpui::WeakEntity<ConnectorSignInModal>,
    connector: ConnectorId,
    creds: Arc<dyn CredentialsProvider>,
    tokio_handle: tokio::runtime::Handle,
    cx: &mut gpui::AsyncApp,
) -> Result<()> {
    match connector {
        ConnectorId::Wuling => run_wuling_flow(this, creds, tokio_handle, cx).await,
        ConnectorId::Github => run_github_flow(this, creds, tokio_handle, cx).await,
    }
}

async fn run_wuling_flow(
    this: gpui::WeakEntity<ConnectorSignInModal>,
    creds: Arc<dyn CredentialsProvider>,
    tokio_handle: tokio::runtime::Handle,
    cx: &mut gpui::AsyncApp,
) -> Result<()> {
    let server = cx.update(|cx| ConnectorSettings::get_global(cx).wuling_server.clone());
    let client = WulingClient::new(server, creds, tokio_handle)?;
    let well_known = client.discover().await?;

    this.update(cx, |this, cx| {
        this.state = State::RequestingCode;
        cx.notify();
    })?;

    let scopes = ["user:read", "repo:read", "git:read", "git:write"];
    let dev = client.device_flow_begin(&well_known, &scopes).await?;
    let expires_in =
        u64::try_from(dev.expires_in).context("Wuling returned a negative expires_in")?;
    let expires_at = SystemTime::now() + Duration::from_secs(expires_in);

    this.update(cx, |this, cx| {
        this.state = State::WaitingForApproval {
            user_code: dev.user_code.clone().into(),
            verification_uri: dev.verification_uri.clone().into(),
            verification_uri_complete: dev.verification_uri_complete.clone().into(),
            expires_at,
        };
        cx.notify();
    })?;

    let mut interval_secs =
        u64::try_from(dev.interval).context("Wuling returned a negative polling interval")?;
    interval_secs = interval_secs.max(1);
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
                let account = finalise_wuling(&client, cx, &tokens).await?;
                let username = account.username.clone();
                cx.update(|cx| {
                    crate::account_state::set_account(cx, ConnectorId::Wuling, Some(account));
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

async fn finalise_wuling(
    client: &WulingClient,
    cx: &mut gpui::AsyncApp,
    tokens: &Tokens,
) -> Result<ConnectorAccount> {
    let me = client.current_user(&tokens.access_token).await?;
    let now = i64::try_from(
        UNIX_EPOCH
            .elapsed()
            .context("system clock is before the Unix epoch")?
            .as_secs(),
    )
    .context("current Unix timestamp does not fit in i64")?;
    let expires_at = now
        .checked_add(tokens.expires_in)
        .context("Wuling token expiration timestamp overflowed")?;
    client
        .save_credentials(cx, &me.username, tokens, expires_at)
        .await?;
    Ok(ConnectorAccount {
        connector: ConnectorId::Wuling,
        display_name: me.display_name,
        username: me.username,
        avatar_url: (!me.avatar_url.is_empty()).then_some(me.avatar_url),
        profile_url: Some(client.server().as_str().to_string()),
    })
}

async fn run_github_flow(
    this: gpui::WeakEntity<ConnectorSignInModal>,
    creds: Arc<dyn CredentialsProvider>,
    tokio_handle: tokio::runtime::Handle,
    cx: &mut gpui::AsyncApp,
) -> Result<()> {
    let client_id = cx.update(|cx| ConnectorSettings::get_global(cx).github_client_id.clone());
    let client = GithubClient::new(client_id, creds, tokio_handle)?;
    this.update(cx, |this, cx| {
        this.state = State::RequestingCode;
        cx.notify();
    })?;
    let device_code = client.device_flow_begin().await?;
    let expires_at = SystemTime::now() + Duration::from_secs(device_code.expires_in);
    this.update(cx, |this, cx| {
        this.state = State::WaitingForApproval {
            user_code: device_code.user_code.clone().into(),
            verification_uri: device_code.verification_uri.clone().into(),
            verification_uri_complete: device_code.verification_uri.clone().into(),
            expires_at,
        };
        cx.notify();
    })?;

    let mut interval_seconds = device_code.interval.max(1);
    loop {
        if SystemTime::now() > expires_at {
            anyhow::bail!("Verification code expired before approval");
        }
        cx.background_executor()
            .timer(Duration::from_secs(interval_seconds))
            .await;
        match client.device_flow_poll(&device_code.device_code).await? {
            GithubPollResult::Pending => continue,
            GithubPollResult::SlowDown => {
                interval_seconds = interval_seconds.saturating_add(5).min(30);
            }
            GithubPollResult::Denied => anyhow::bail!("Sign-in denied"),
            GithubPollResult::Expired => {
                anyhow::bail!("Verification code expired before approval")
            }
            GithubPollResult::Issued(access_token) => {
                let account = client.current_account(&access_token).await?;
                let username = account.username.clone();
                client
                    .save_credentials(access_token, account.clone(), cx)
                    .await?;
                cx.update(|cx| {
                    crate::account_state::set_account(cx, ConnectorId::Github, Some(account));
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

impl Render for ConnectorSignInModal {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let header = h_flex()
            .gap_2()
            .child(
                Icon::new(IconName::Person)
                    .size(IconSize::Small)
                    .color(Color::Accent),
            )
            .child(Headline::new(tr_f!("Sign in to {}", self.connector)).size(HeadlineSize::Small));

        let server_line = Label::new(self.server_label.clone())
            .size(LabelSize::Small)
            .color(Color::Muted);

        let status = Label::new(Self::render_status_label(&self.state))
            .size(LabelSize::Small)
            .color(Color::Muted);

        let body: gpui::AnyElement = match &self.state {
            State::Discovering | State::RequestingCode => v_flex()
                .gap_2()
                .items_center()
                .child(Label::new(tr_f!("Connecting to {}…", self.connector)))
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
                        Label::new(tr!(
                            "Visit the URL below in your browser, then enter this code:"
                        ))
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
                                            if copied { tr!("Copied!") } else { tr!("Copy") },
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
                                Button::new("open-browser", tr!("Open browser"))
                                    .style(ButtonStyle::Filled)
                                    .full_width()
                                    .on_click(move |_, _, cx| cx.open_url(&open_url)),
                            )
                            .child(
                                Button::new("cancel", tr!("Cancel"))
                                    .style(ButtonStyle::Subtle)
                                    .on_click(cx.listener(|_, _, _, cx| cx.emit(DismissEvent))),
                            ),
                    )
                    .child(
                        Label::new(tr_f!(
                            "Or visit: {}  ·  Code expires in {}s",
                            fallback_url,
                            remaining
                        ))
                        .size(LabelSize::Small)
                        .color(Color::Muted),
                    )
                    .into_any_element()
            }
            State::Success { username } => v_flex()
                .gap_3()
                .child(Label::new(tr_f!("Signed in as {}.", username)).color(Color::Success))
                .child(
                    Button::new("done", tr!("Done"))
                        .style(ButtonStyle::Filled)
                        .full_width()
                        .on_click(cx.listener(|_, _, _, cx| cx.emit(DismissEvent))),
                )
                .into_any_element(),
            State::Error { message } => v_flex()
                .gap_3()
                .child(Label::new(message.clone()).color(Color::Error))
                .child(
                    Button::new("close", tr!("Close"))
                        .style(ButtonStyle::Subtle)
                        .full_width()
                        .on_click(cx.listener(|_, _, _, cx| cx.emit(DismissEvent))),
                )
                .into_any_element(),
        };

        let focus_handle = self.focus_handle.clone();
        v_flex()
            .key_context("ConnectorSignIn")
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
