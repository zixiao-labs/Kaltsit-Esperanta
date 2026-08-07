use std::path::PathBuf;
use std::sync::Arc;

use agent_client_protocol::schema::v1 as acp;
use anyhow::Result;
use extension_cef::CdpSession;
use futures::FutureExt as _;
use gpui::{App, AppContext as _, Task};
use parking_lot::Mutex;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use ui::SharedString;
use util::markdown::MarkdownInlineCode;

use crate::sandboxing::{NetworkRequest, SandboxRequest};
use crate::{AgentTool, ToolCallEventStream, ToolInput, ToolPermissionContext};

/// Controls the embedded browser used by the frontend-enhancement agent tools.
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
pub struct BrowserToolInput {
    pub action: BrowserAction,
}

#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum BrowserAction {
    Navigate { url: String },
    Click { x: f32, y: f32 },
    Type { text: String },
    Scroll { delta_x: f32, delta_y: f32 },
    Screenshot,
    Console { max_lines: Option<usize> },
    Network { max_entries: Option<usize> },
}

#[derive(Debug, Serialize, Deserialize)]
pub struct BrowserToolOutput {
    pub ok: bool,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub screenshot_path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub console: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub network: Option<Vec<String>>,
}

impl From<BrowserToolOutput> for language_model::LanguageModelToolResultContent {
    fn from(value: BrowserToolOutput) -> Self {
        let json = serde_json::to_string_pretty(&value).unwrap_or_else(|_| value.message.clone());
        Self::Text(json.into())
    }
}

pub struct BrowserTool {
    session: Arc<Mutex<Option<Arc<CdpSession>>>>,
}

impl BrowserTool {
    pub fn new() -> Self {
        Self {
            session: Arc::new(Mutex::new(None)),
        }
    }

    async fn session(&self) -> Result<Arc<CdpSession>> {
        if let Some(session) = self.session.lock().clone() {
            return Ok(session);
        }
        let session = CdpSession::create("about:blank").await?;
        *self.session.lock() = Some(session.clone());
        Ok(session)
    }
}

impl Default for BrowserTool {
    fn default() -> Self {
        Self::new()
    }
}

fn fail(message: impl Into<String>) -> BrowserToolOutput {
    BrowserToolOutput {
        ok: false,
        message: message.into(),
        screenshot_path: None,
        console: None,
        network: None,
    }
}

impl AgentTool for BrowserTool {
    type Input = BrowserToolInput;
    type Output = BrowserToolOutput;

    const NAME: &'static str = "browser";

    fn kind() -> acp::ToolKind {
        acp::ToolKind::Other
    }

    fn allow_in_restricted_mode() -> bool {
        false
    }

    fn initial_title(
        &self,
        input: Result<Self::Input, serde_json::Value>,
        _cx: &mut App,
    ) -> SharedString {
        match input {
            Ok(input) => match input.action {
                BrowserAction::Navigate { ref url } => {
                    format!("Browser navigate {}", MarkdownInlineCode(url)).into()
                }
                BrowserAction::Click { x, y } => format!("Browser click ({x}, {y})").into(),
                BrowserAction::Type { ref text } => {
                    format!("Browser type {}", MarkdownInlineCode(text)).into()
                }
                BrowserAction::Scroll { .. } => "Browser scroll".into(),
                BrowserAction::Screenshot => "Browser screenshot".into(),
                BrowserAction::Console { .. } => "Browser console".into(),
                BrowserAction::Network { .. } => "Browser network".into(),
            },
            Err(_) => "Browser".into(),
        }
    }

    fn run(
        self: Arc<Self>,
        input: ToolInput<Self::Input>,
        event_stream: ToolCallEventStream,
        cx: &mut App,
    ) -> Task<Result<Self::Output, Self::Output>> {
        cx.spawn(async move |cx| {
            let input: BrowserToolInput = input
                .recv()
                .await
                .map_err(|error| fail(format!("invalid browser tool input: {error}")))?;

            let permission_context = match &input.action {
                BrowserAction::Navigate { url } => {
                    ToolPermissionContext::new(Self::NAME, vec![url.clone()])
                }
                _ => ToolPermissionContext::new(Self::NAME, Vec::new()),
            };
            let authorize_title = match &input.action {
                BrowserAction::Navigate { url } => {
                    format!("Browser navigate {}", MarkdownInlineCode(url))
                }
                BrowserAction::Click { x, y } => format!("Browser click ({x}, {y})"),
                BrowserAction::Type { text } => format!("Browser type {}", MarkdownInlineCode(text)),
                BrowserAction::Scroll { .. } => "Browser scroll".into(),
                BrowserAction::Screenshot => "Browser screenshot".into(),
                BrowserAction::Console { .. } => "Browser console".into(),
                BrowserAction::Network { .. } => "Browser network".into(),
            };

            let authorize = cx.update(|cx| {
                event_stream.authorize(authorize_title.clone(), permission_context, cx)
            });
            futures::select! {
                result = authorize.fuse() => result.map_err(|error| fail(error.to_string()))?,
                _ = event_stream.cancelled_by_user().fuse() => {
                    return Err(fail("Browser action cancelled by user"));
                }
            };

            if let BrowserAction::Navigate { url } = &input.action {
                if url != "about:blank" {
                    let host = url::Url::parse(url)
                        .ok()
                        .and_then(|parsed| parsed.host_str().map(str::to_owned))
                        .ok_or_else(|| fail(format!("invalid URL: {url}")))?;
                    let pattern = http_proxy::HostPattern::parse(&host).map_err(|error| {
                        fail(format!("cannot authorize browser navigate to {host:?}: {error}"))
                    })?;
                    let request = SandboxRequest {
                        network: NetworkRequest::Hosts(vec![pattern]),
                        ..SandboxRequest::default()
                    };
                    let approve = cx.update(|cx| {
                        event_stream.authorize_sandbox(
                            request,
                            format!("Browser navigate to {host}"),
                            cx,
                        )
                    });
                    approve
                        .await
                        .map_err(|error| fail(format!("browser navigate blocked: {error:#}")))?;
                }
            }

            let tool = self.clone();
            cx.background_spawn(async move { run_browser_action(tool, input).await })
                .await
                .map_err(|error| fail(format!("{error:#}")))
        })
    }
}

async fn run_browser_action(
    tool: Arc<BrowserTool>,
    input: BrowserToolInput,
) -> Result<BrowserToolOutput> {
    let session = tool.session().await?;
    match input.action {
        BrowserAction::Navigate { url } => {
            session.navigate(&url).await?;
            Ok(BrowserToolOutput {
                ok: true,
                message: format!("navigated to {url}"),
                screenshot_path: None,
                console: None,
                network: None,
            })
        }
        BrowserAction::Click { x, y } => {
            session.click(x, y)?;
            Ok(BrowserToolOutput {
                ok: true,
                message: format!("clicked at ({x}, {y})"),
                screenshot_path: None,
                console: None,
                network: None,
            })
        }
        BrowserAction::Type { text } => {
            session.type_text(&text)?;
            Ok(BrowserToolOutput {
                ok: true,
                message: format!("typed {} characters", text.chars().count()),
                screenshot_path: None,
                console: None,
                network: None,
            })
        }
        BrowserAction::Scroll { delta_x, delta_y } => {
            session.scroll(delta_x, delta_y)?;
            Ok(BrowserToolOutput {
                ok: true,
                message: format!("scrolled by ({delta_x}, {delta_y})"),
                screenshot_path: None,
                console: None,
                network: None,
            })
        }
        BrowserAction::Screenshot => {
            let frame = session.screenshot().await?;
            let path = write_screenshot_file(&frame)?;
            Ok(BrowserToolOutput {
                ok: true,
                message: format!(
                    "screenshot saved ({}×{}, {} bytes BGRA)",
                    frame.width,
                    frame.height,
                    frame.bgra.len()
                ),
                screenshot_path: Some(path.display().to_string()),
                console: None,
                network: None,
            })
        }
        BrowserAction::Console { max_lines } => {
            let max_lines = max_lines.unwrap_or(100).min(500);
            let lines: Vec<String> = session
                .console_messages()
                .into_iter()
                .rev()
                .take(max_lines)
                .map(|entry| format!("[{}] {}", entry.level, entry.text))
                .collect::<Vec<_>>()
                .into_iter()
                .rev()
                .collect();
            Ok(BrowserToolOutput {
                ok: true,
                message: format!("{} console lines", lines.len()),
                screenshot_path: None,
                console: Some(lines),
                network: None,
            })
        }
        BrowserAction::Network { max_entries } => {
            let max_entries = max_entries.unwrap_or(100).min(500);
            let entries: Vec<String> = session
                .network_entries()
                .into_iter()
                .rev()
                .take(max_entries)
                .map(|entry| {
                    format!(
                        "{} {} {}",
                        entry.method,
                        entry
                            .status
                            .map(|status| status.to_string())
                            .unwrap_or_else(|| "-".into()),
                        entry.url
                    )
                })
                .collect::<Vec<_>>()
                .into_iter()
                .rev()
                .collect();
            Ok(BrowserToolOutput {
                ok: true,
                message: format!("{} network entries", entries.len()),
                screenshot_path: None,
                console: None,
                network: Some(entries),
            })
        }
    }
}

fn write_screenshot_file(frame: &extension_cef::SharedPaintFrame) -> Result<PathBuf> {
    let dir = paths::temp_dir().join("browser-screenshots");
    std::fs::create_dir_all(&dir)?;
    let path = dir.join(format!(
        "shot-{}-{}x{}.bgra",
        uuid::Uuid::new_v4(),
        frame.width,
        frame.height
    ));
    std::fs::write(&path, frame.bgra.as_ref())?;
    Ok(path)
}
