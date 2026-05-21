//! Right-dock side panel listing Wuling DevOps issues and merge requests
//! for the workspace's active repository.
//!
//! The panel has three observable surfaces:
//!
//!   1. **Signed-out empty state** — a "Sign in to Wuling DevOps" button that
//!      dispatches `ama10::SignIn`.
//!   2. **No-Wuling-repo empty state** — when the active project's origin
//!      remote URL doesn't match the configured Wuling server host, a hint
//!      tells the user the panel only lights up for Wuling-hosted repos.
//!   3. **List state** — tabs for Issues / Merge Requests, each lazily
//!      fetched on first display and re-fetched on the refresh button.
//!      Lists default to `state=open`; a single toggle flips between open and
//!      closed (merged for MRs is folded under closed in this iteration).
//!
//! Construction goes through [`WulingPanel::load`], called from
//! `crates/zed::ama10::init` exactly like every other panel in the editor.
//! `register_actions` wires `ama10::ToggleWulingPanel` to the workspace so
//! the command palette / keybindings can toggle focus.

use std::sync::Arc;

use ama10::auth::WulingClient;
use ama10::server_url::ServerUrl;
use ama10::wuling_api::{
    IssueStateLite, IssueSummary, MergeRequestSummary, MrStateLite, RepoCoords, WulingListClient,
    parse_repo_coords,
};
use anyhow::Result;
use credentials_provider::CredentialsProvider;
use gpui::{
    Action, App, AppContext as _, AsyncWindowContext, Context, Entity, EventEmitter, FocusHandle,
    Focusable, InteractiveElement as _, IntoElement, ParentElement as _, Render, SharedString,
    StatefulInteractiveElement as _, Styled as _, Subscription, Task, WeakEntity, Window, actions,
    div, px,
};
use project::Project;
use ui::{
    ActiveTheme as _, Button, ButtonCommon as _, ButtonSize, ButtonStyle, Clickable as _, Color,
    Disableable as _, FluentBuilder as _, Icon, IconButton, IconName, IconSize, Label,
    LabelCommon as _, LabelSize, Tooltip, h_flex, v_flex,
};
use workspace::{
    Workspace,
    dock::{DockPosition, Panel, PanelEvent},
};

use crate::account_state::{WulingAccountChanged, WulingAccountState};
use crate::settings::WulingConfig;
use crate::{SignIn, SignOut};

actions!(
    ama10,
    [
        /// Toggle focus of the Wuling DevOps side panel (Issues / MRs).
        ToggleWulingPanel,
        /// Refresh the currently-active list in the Wuling DevOps panel.
        RefreshWulingPanel,
    ]
);

const WULING_PANEL_KEY: &str = "WulingPanel";
const DEFAULT_PANEL_WIDTH: f32 = 320.0;
const LIST_LIMIT: u32 = 50;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Tab {
    Issues,
    MergeRequests,
}

#[derive(Debug)]
enum FetchState<T> {
    Idle,
    Loading,
    Loaded(Vec<T>),
    Error(SharedString),
}

impl<T> FetchState<T> {
    fn is_loading(&self) -> bool {
        matches!(self, FetchState::Loading)
    }
}

pub struct WulingPanel {
    focus_handle: FocusHandle,
    project: Entity<Project>,
    creds: Arc<dyn CredentialsProvider>,
    active_tab: Tab,
    issues: FetchState<IssueSummary>,
    merge_requests: FetchState<MergeRequestSummary>,
    issue_filter: IssueStateLite,
    mr_filter: MrStateLite,
    /// The repo coords used by the most recent fetch. When the active repo
    /// changes we invalidate the cached lists so the user doesn't briefly see
    /// the wrong project's data.
    last_fetched_for: Option<RepoCoords>,
    pending_fetch: Option<Task<()>>,
    _subscriptions: Vec<Subscription>,
}

impl WulingPanel {
    pub async fn load(
        workspace: WeakEntity<Workspace>,
        mut cx: AsyncWindowContext,
    ) -> Result<Entity<Self>> {
        workspace.update_in(&mut cx, |workspace, window, cx| Self::new(workspace, window, cx))
    }

    fn new(
        workspace: &mut Workspace,
        _window: &mut Window,
        cx: &mut Context<Workspace>,
    ) -> Entity<Self> {
        let project = workspace.project().clone();
        let creds = workspace
            .client()
            .credentials_provider();

        cx.new(|cx| {
            let focus_handle = cx.focus_handle();
            let mut subscriptions = Vec::new();
            if let Some(state) = WulingAccountState::try_global(cx) {
                subscriptions.push(cx.subscribe(
                    &state,
                    |this: &mut Self, _, _: &WulingAccountChanged, cx| {
                        // Sign-in state changed: drop cached lists so the next
                        // render re-fetches with the new credentials (or shows
                        // the signed-out empty state).
                        this.issues = FetchState::Idle;
                        this.merge_requests = FetchState::Idle;
                        this.last_fetched_for = None;
                        this.pending_fetch = None;
                        cx.notify();
                    },
                ));
            }
            Self {
                focus_handle,
                project,
                creds,
                active_tab: Tab::Issues,
                issues: FetchState::Idle,
                merge_requests: FetchState::Idle,
                issue_filter: IssueStateLite::Open,
                mr_filter: MrStateLite::Open,
                last_fetched_for: None,
                pending_fetch: None,
                _subscriptions: subscriptions,
            }
        })
    }

    fn current_repo_coords(&self, cx: &App) -> Option<RepoCoords> {
        let server = WulingConfig::load().server;
        let server_host = server.host().to_string();
        let git_store = self.project.read(cx).git_store().read(cx);
        let repo = git_store.active_repository()?;
        let url = repo.read(cx).default_remote_url()?;
        parse_repo_coords(&url, &server_host)
    }

    fn is_signed_in(&self, cx: &App) -> bool {
        WulingAccountState::try_global(cx)
            .and_then(|s| s.read(cx).account().cloned())
            .is_some()
    }

    fn ensure_loaded(&mut self, cx: &mut Context<Self>) {
        if self.pending_fetch.is_some() {
            return;
        }
        let needs_fetch = match self.active_tab {
            Tab::Issues => matches!(self.issues, FetchState::Idle),
            Tab::MergeRequests => matches!(self.merge_requests, FetchState::Idle),
        };
        if !needs_fetch {
            return;
        }
        self.refresh(cx);
    }

    fn refresh(&mut self, cx: &mut Context<Self>) {
        let Some(coords) = self.current_repo_coords(cx) else {
            return;
        };
        if !self.is_signed_in(cx) {
            return;
        }
        let config = WulingConfig::load();
        let server = config.server;
        let creds = self.creds.clone();
        let tab = self.active_tab;
        let issue_filter = self.issue_filter;
        let mr_filter = self.mr_filter;

        match tab {
            Tab::Issues => self.issues = FetchState::Loading,
            Tab::MergeRequests => self.merge_requests = FetchState::Loading,
        }
        self.last_fetched_for = Some(coords.clone());
        cx.notify();

        let task = cx.spawn(async move |this, cx| {
            let result = fetch_data(server, creds, tab, coords, issue_filter, mr_filter, cx).await;
            this.update(cx, |this, cx| {
                this.pending_fetch = None;
                match (tab, result) {
                    (Tab::Issues, Ok(FetchOutput::Issues(items))) => {
                        this.issues = FetchState::Loaded(items);
                    }
                    (Tab::MergeRequests, Ok(FetchOutput::MergeRequests(items))) => {
                        this.merge_requests = FetchState::Loaded(items);
                    }
                    (Tab::Issues, Err(err)) => {
                        this.issues = FetchState::Error(err.to_string().into());
                    }
                    (Tab::MergeRequests, Err(err)) => {
                        this.merge_requests = FetchState::Error(err.to_string().into());
                    }
                    // Tab/output mismatch is impossible by construction.
                    _ => {}
                }
                cx.notify();
            })
            .ok();
        });
        self.pending_fetch = Some(task);
    }

    fn set_tab(&mut self, tab: Tab, cx: &mut Context<Self>) {
        if self.active_tab == tab {
            return;
        }
        self.active_tab = tab;
        cx.notify();
        self.ensure_loaded(cx);
    }

    fn toggle_state_filter(&mut self, cx: &mut Context<Self>) {
        match self.active_tab {
            Tab::Issues => {
                self.issue_filter = match self.issue_filter {
                    IssueStateLite::Open => IssueStateLite::Closed,
                    IssueStateLite::Closed => IssueStateLite::Open,
                };
                self.issues = FetchState::Idle;
            }
            Tab::MergeRequests => {
                self.mr_filter = match self.mr_filter {
                    MrStateLite::Open => MrStateLite::Merged,
                    MrStateLite::Merged => MrStateLite::Closed,
                    MrStateLite::Closed => MrStateLite::Open,
                };
                self.merge_requests = FetchState::Idle;
            }
        }
        self.refresh(cx);
    }

    fn render_header(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let filter_label: SharedString = match self.active_tab {
            Tab::Issues => match self.issue_filter {
                IssueStateLite::Open => "Open".into(),
                IssueStateLite::Closed => "Closed".into(),
            },
            Tab::MergeRequests => match self.mr_filter {
                MrStateLite::Open => "Open".into(),
                MrStateLite::Merged => "Merged".into(),
                MrStateLite::Closed => "Closed".into(),
            },
        };
        let loading = self.is_active_tab_loading();
        h_flex()
            .w_full()
            .px_2()
            .py_1()
            .gap_2()
            .justify_between()
            .child(
                h_flex()
                    .gap_1()
                    .child(Icon::new(IconName::PullRequest).size(IconSize::Small))
                    .child(Label::new("Wuling").size(LabelSize::Default)),
            )
            .child(
                h_flex()
                    .gap_1()
                    .child(
                        Button::new("wuling-filter-toggle", filter_label)
                            .style(ButtonStyle::Subtle)
                            .size(ButtonSize::Compact)
                            .on_click(cx.listener(|this, _, _, cx| this.toggle_state_filter(cx))),
                    )
                    .child(
                        IconButton::new("wuling-refresh", IconName::RotateCw)
                            .icon_size(IconSize::Small)
                            .tooltip(Tooltip::text("Refresh"))
                            .disabled(loading)
                            .on_click(cx.listener(|this, _, _, cx| {
                                // Force a re-fetch regardless of cached state.
                                match this.active_tab {
                                    Tab::Issues => this.issues = FetchState::Idle,
                                    Tab::MergeRequests => {
                                        this.merge_requests = FetchState::Idle
                                    }
                                }
                                this.refresh(cx);
                            })),
                    ),
            )
    }

    fn is_active_tab_loading(&self) -> bool {
        match self.active_tab {
            Tab::Issues => self.issues.is_loading(),
            Tab::MergeRequests => self.merge_requests.is_loading(),
        }
    }

    fn render_tab_bar(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let active_tab = self.active_tab;
        let issues_count = match &self.issues {
            FetchState::Loaded(v) => Some(v.len()),
            _ => None,
        };
        let mrs_count = match &self.merge_requests {
            FetchState::Loaded(v) => Some(v.len()),
            _ => None,
        };
        let issues_label: SharedString = match issues_count {
            Some(n) => format!("Issues ({n})").into(),
            None => "Issues".into(),
        };
        let mrs_label: SharedString = match mrs_count {
            Some(n) => format!("MRs ({n})").into(),
            None => "MRs".into(),
        };
        h_flex()
            .w_full()
            .px_2()
            .gap_1()
            .border_b_1()
            .border_color(cx.theme().colors().border_variant)
            .child(
                Button::new("wuling-tab-issues", issues_label)
                    .style(if active_tab == Tab::Issues {
                        ButtonStyle::Filled
                    } else {
                        ButtonStyle::Subtle
                    })
                    .size(ButtonSize::Compact)
                    .on_click(cx.listener(|this, _, _, cx| this.set_tab(Tab::Issues, cx))),
            )
            .child(
                Button::new("wuling-tab-mrs", mrs_label)
                    .style(if active_tab == Tab::MergeRequests {
                        ButtonStyle::Filled
                    } else {
                        ButtonStyle::Subtle
                    })
                    .size(ButtonSize::Compact)
                    .on_click(
                        cx.listener(|this, _, _, cx| this.set_tab(Tab::MergeRequests, cx)),
                    ),
            )
    }

    fn render_body(&self, cx: &mut Context<Self>) -> impl IntoElement {
        if !self.is_signed_in(cx) {
            return self.render_signed_out(cx).into_any_element();
        }
        let Some(_coords) = self.current_repo_coords(cx) else {
            return self.render_no_wuling_repo(cx).into_any_element();
        };

        match self.active_tab {
            Tab::Issues => match &self.issues {
                FetchState::Idle | FetchState::Loading => loading_placeholder(cx).into_any_element(),
                FetchState::Error(message) => error_placeholder(message.clone(), cx).into_any_element(),
                FetchState::Loaded(items) if items.is_empty() => {
                    empty_list_placeholder("No issues match the current filter.", cx)
                        .into_any_element()
                }
                FetchState::Loaded(items) => render_issue_list(items, cx).into_any_element(),
            },
            Tab::MergeRequests => match &self.merge_requests {
                FetchState::Idle | FetchState::Loading => loading_placeholder(cx).into_any_element(),
                FetchState::Error(message) => error_placeholder(message.clone(), cx).into_any_element(),
                FetchState::Loaded(items) if items.is_empty() => {
                    empty_list_placeholder("No merge requests match the current filter.", cx)
                        .into_any_element()
                }
                FetchState::Loaded(items) => render_mr_list(items, cx).into_any_element(),
            },
        }
    }

    fn render_signed_out(&self, cx: &mut Context<Self>) -> impl IntoElement {
        v_flex()
            .size_full()
            .items_center()
            .justify_center()
            .gap_3()
            .p_4()
            .child(
                Icon::new(IconName::Public)
                    .size(IconSize::XLarge)
                    .color(Color::Muted),
            )
            .child(
                Label::new("Not signed in to Wuling DevOps")
                    .size(LabelSize::Default),
            )
            .child(
                Button::new("wuling-panel-sign-in", "Sign In")
                    .style(ButtonStyle::Filled)
                    .on_click(cx.listener(|_, _, window, cx| {
                        window.dispatch_action(SignIn.boxed_clone(), cx);
                    })),
            )
    }

    fn render_no_wuling_repo(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let server = WulingConfig::load().server;
        let server_host: SharedString = server.host().to_string().into();
        v_flex()
            .size_full()
            .items_center()
            .justify_center()
            .gap_3()
            .p_4()
            .child(
                Icon::new(IconName::Server)
                    .size(IconSize::XLarge)
                    .color(Color::Muted),
            )
            .child(Label::new("No Wuling-hosted repository in this workspace").size(LabelSize::Default))
            .child(
                Label::new(format!("Open a project with origin on {server_host} to see issues and merge requests."))
                    .size(LabelSize::Small)
                    .color(Color::Muted),
            )
            .child(
                Button::new("wuling-panel-sign-out", "Sign Out")
                    .style(ButtonStyle::Subtle)
                    .size(ButtonSize::Compact)
                    .on_click(cx.listener(|_, _, window, cx| {
                        window.dispatch_action(SignOut.boxed_clone(), cx);
                    })),
            )
    }
}

/// Render an issue row. Compact layout: state badge + #number + title (truncated)
/// + author on the right.
fn render_issue_list(items: &[IssueSummary], cx: &App) -> impl IntoElement {
    let mut rows = v_flex().w_full();
    for issue in items {
        rows = rows.child(render_issue_row(issue, cx));
    }
    div()
        .id("wuling-issues-scroll")
        .size_full()
        .overflow_y_scroll()
        .child(rows)
}

fn render_issue_row(issue: &IssueSummary, cx: &App) -> impl IntoElement {
    let (badge_color, badge_text): (Color, &'static str) = match issue.state {
        IssueStateLite::Open => (Color::Success, "open"),
        IssueStateLite::Closed => (Color::Muted, "closed"),
    };
    let author = issue
        .author
        .as_ref()
        .map(|a| {
            if !a.display_name.is_empty() {
                a.display_name.clone()
            } else {
                a.username.clone()
            }
        })
        .unwrap_or_default();
    h_flex()
        .w_full()
        .px_2()
        .py_1()
        .gap_2()
        .border_b_1()
        .border_color(cx.theme().colors().border_variant.opacity(0.5))
        .child(
            div().min_w(px(48.)).child(
                Label::new(badge_text)
                    .size(LabelSize::Small)
                    .color(badge_color),
            ),
        )
        .child(
            v_flex()
                .flex_1()
                .gap_0p5()
                .child(
                    Label::new(format!("#{} {}", issue.number, issue.title))
                        .size(LabelSize::Default),
                )
                .when(!author.is_empty() || issue.comment_count > 0, |this| {
                    let mut footer = h_flex().gap_2();
                    if !author.is_empty() {
                        footer = footer.child(
                            Label::new(format!("@{author}"))
                                .size(LabelSize::Small)
                                .color(Color::Muted),
                        );
                    }
                    if issue.comment_count > 0 {
                        footer = footer.child(
                            Label::new(format!("{} comments", issue.comment_count))
                                .size(LabelSize::Small)
                                .color(Color::Muted),
                        );
                    }
                    this.child(footer)
                }),
        )
}

fn render_mr_list(items: &[MergeRequestSummary], cx: &App) -> impl IntoElement {
    let mut rows = v_flex().w_full();
    for mr in items {
        rows = rows.child(render_mr_row(mr, cx));
    }
    div()
        .id("wuling-mrs-scroll")
        .size_full()
        .overflow_y_scroll()
        .child(rows)
}

fn render_mr_row(mr: &MergeRequestSummary, cx: &App) -> impl IntoElement {
    let (badge_color, badge_text): (Color, &'static str) = match mr.state {
        MrStateLite::Open => (Color::Success, "open"),
        MrStateLite::Merged => (Color::Accent, "merged"),
        MrStateLite::Closed => (Color::Muted, "closed"),
    };
    let author = mr
        .author
        .as_ref()
        .map(|a| {
            if !a.display_name.is_empty() {
                a.display_name.clone()
            } else {
                a.username.clone()
            }
        })
        .unwrap_or_default();
    h_flex()
        .w_full()
        .px_2()
        .py_1()
        .gap_2()
        .border_b_1()
        .border_color(cx.theme().colors().border_variant.opacity(0.5))
        .child(
            div().min_w(px(56.)).child(
                Label::new(badge_text)
                    .size(LabelSize::Small)
                    .color(badge_color),
            ),
        )
        .child(
            v_flex()
                .flex_1()
                .gap_0p5()
                .child(
                    Label::new(format!("!{} {}", mr.number, mr.title))
                        .size(LabelSize::Default),
                )
                .child(
                    h_flex()
                        .gap_2()
                        .when(!author.is_empty(), |this| {
                            this.child(
                                Label::new(format!("@{author}"))
                                    .size(LabelSize::Small)
                                    .color(Color::Muted),
                            )
                        })
                        .when(!mr.source_ref.is_empty() && !mr.target_ref.is_empty(), |this| {
                            this.child(
                                Label::new(format!(
                                    "{} → {}",
                                    short_ref(&mr.source_ref),
                                    short_ref(&mr.target_ref)
                                ))
                                .size(LabelSize::Small)
                                .color(Color::Muted),
                            )
                        }),
                ),
        )
}

fn short_ref(refname: &str) -> String {
    refname
        .strip_prefix("refs/heads/")
        .or_else(|| refname.strip_prefix("refs/tags/"))
        .unwrap_or(refname)
        .to_string()
}

fn loading_placeholder(_cx: &App) -> impl IntoElement {
    v_flex()
        .size_full()
        .items_center()
        .justify_center()
        .gap_2()
        .child(
            Icon::new(IconName::LoadCircle)
                .size(IconSize::Medium)
                .color(Color::Muted),
        )
        .child(
            Label::new("Loading…")
                .size(LabelSize::Small)
                .color(Color::Muted),
        )
}

fn empty_list_placeholder(text: &'static str, _cx: &App) -> impl IntoElement {
    v_flex()
        .size_full()
        .items_center()
        .justify_center()
        .child(
            Label::new(text)
                .size(LabelSize::Small)
                .color(Color::Muted),
        )
}

fn error_placeholder(message: SharedString, _cx: &App) -> impl IntoElement {
    v_flex()
        .size_full()
        .items_center()
        .justify_center()
        .gap_2()
        .p_4()
        .child(
            Icon::new(IconName::Warning)
                .size(IconSize::Medium)
                .color(Color::Warning),
        )
        .child(
            Label::new("Failed to load")
                .size(LabelSize::Default),
        )
        .child(
            Label::new(message)
                .size(LabelSize::Small)
                .color(Color::Muted),
        )
}

enum FetchOutput {
    Issues(Vec<IssueSummary>),
    MergeRequests(Vec<MergeRequestSummary>),
}

async fn fetch_data(
    server: ServerUrl,
    creds: Arc<dyn CredentialsProvider>,
    tab: Tab,
    coords: RepoCoords,
    issue_filter: IssueStateLite,
    mr_filter: MrStateLite,
    cx: &mut gpui::AsyncApp,
) -> Result<FetchOutput> {
    let tokio_handle = cx.update(|cx| gpui_tokio::Tokio::handle(cx));
    // Pull the freshest access_token out of the keychain on every fetch so the
    // background refresh in `askpass_delegate` / `account_state` keeps us in
    // sync without an explicit notify on this panel.
    let auth_client = WulingClient::new(server.clone(), creds.clone(), tokio_handle.clone());
    let stored = auth_client
        .load_credentials(cx)
        .await?
        .ok_or_else(|| anyhow::anyhow!("not signed in"))?;
    let list_client = WulingListClient::new(server, creds, tokio_handle);
    match tab {
        Tab::Issues => {
            let items = list_client
                .list_issues(
                    &stored.access_token,
                    &coords.org_slug,
                    &coords.project_slug,
                    Some(issue_filter),
                    LIST_LIMIT,
                )
                .await?;
            Ok(FetchOutput::Issues(items))
        }
        Tab::MergeRequests => {
            let items = list_client
                .list_merge_requests(
                    &stored.access_token,
                    &coords.org_slug,
                    &coords.project_slug,
                    &coords.repo_slug,
                    Some(mr_filter),
                    LIST_LIMIT,
                )
                .await?;
            Ok(FetchOutput::MergeRequests(items))
        }
    }
}

impl Render for WulingPanel {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.ensure_loaded(cx);
        let header = self.render_header(cx).into_any_element();
        let tab_bar = self.render_tab_bar(cx).into_any_element();
        let body = self.render_body(cx).into_any_element();
        let bg = cx.theme().colors().panel_background;
        v_flex()
            .key_context("WulingPanel")
            .track_focus(&self.focus_handle)
            .size_full()
            .bg(bg)
            .child(header)
            .child(tab_bar)
            .child(div().flex_1().min_h_0().child(body))
    }
}

impl Focusable for WulingPanel {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl EventEmitter<PanelEvent> for WulingPanel {}

impl Panel for WulingPanel {
    fn persistent_name() -> &'static str {
        "WulingPanel"
    }

    fn panel_key() -> &'static str {
        WULING_PANEL_KEY
    }

    fn position(&self, _: &Window, _: &App) -> DockPosition {
        DockPosition::Right
    }

    fn position_is_valid(&self, position: DockPosition) -> bool {
        matches!(position, DockPosition::Left | DockPosition::Right)
    }

    fn set_position(&mut self, _position: DockPosition, _: &mut Window, _: &mut Context<Self>) {
        // Position is not persisted at the moment — the user can move the panel
        // for the lifetime of the window, but it returns to Right on relaunch.
        // Once we add panel-level settings to wuling.json we'll persist here.
    }

    fn default_size(&self, _: &Window, _: &App) -> gpui::Pixels {
        px(DEFAULT_PANEL_WIDTH)
    }

    fn icon(&self, _: &Window, _: &App) -> Option<IconName> {
        Some(IconName::PullRequest)
    }

    fn icon_tooltip(&self, _: &Window, _: &App) -> Option<&'static str> {
        Some("Wuling: Issues / Merge Requests")
    }

    fn toggle_action(&self) -> Box<dyn Action> {
        Box::new(ToggleWulingPanel)
    }

    fn activation_priority(&self) -> u32 {
        10
    }
}

pub fn register_workspace_actions(workspace: &mut Workspace) {
    workspace
        .register_action(|workspace, _: &ToggleWulingPanel, window, cx| {
            workspace.toggle_panel_focus::<WulingPanel>(window, cx);
        })
        .register_action(|workspace, _: &RefreshWulingPanel, _window, cx| {
            if let Some(panel) = workspace.panel::<WulingPanel>(cx) {
                panel.update(cx, |panel, cx| {
                    match panel.active_tab {
                        Tab::Issues => panel.issues = FetchState::Idle,
                        Tab::MergeRequests => panel.merge_requests = FetchState::Idle,
                    }
                    panel.refresh(cx);
                });
            }
        });
}
