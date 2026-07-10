use acp_thread::PlanFileOpenRequest;
use agent_client_protocol::schema::v1 as acp;
use anyhow::Result;
use futures::FutureExt as _;
use gpui::{App, SharedString, Task};
use language_model::LanguageModelToolResultContent;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::{collections::BTreeMap, sync::Arc};

use crate::{AgentTool, ToolCallEventStream, ToolInput};

const APPROVAL_FIELD: &str = "approval";
const FEEDBACK_FIELD: &str = "feedback";
const APPROVE: &str = "approve";
const REQUEST_CHANGES: &str = "request_changes";

/// Enter plan mode before doing implementation work.
///
/// Use this when the user explicitly asks for a plan, or when a complex task
/// would benefit from research and design before editing files. This switches
/// the thread to the read-only Plan profile and returns the plan file path. In
/// plan mode, only the plan file may be edited; all implementation changes must
/// wait until the user approves the plan through `exit_plan_mode`.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub struct EnterPlanModeToolInput {}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum EnterPlanModeToolOutput {
    Entered {
        plan_path: String,
        instructions: String,
    },
    Error {
        error: String,
    },
}

impl From<EnterPlanModeToolOutput> for LanguageModelToolResultContent {
    fn from(output: EnterPlanModeToolOutput) -> Self {
        serde_json::to_string(&output)
            .unwrap_or_else(|e| format!("Failed to serialize enter_plan_mode output: {e}"))
            .into()
    }
}

pub struct EnterPlanModeTool;

impl AgentTool for EnterPlanModeTool {
    type Input = EnterPlanModeToolInput;
    type Output = EnterPlanModeToolOutput;

    const NAME: &'static str = "enter_plan_mode";

    fn kind() -> acp::ToolKind {
        acp::ToolKind::Other
    }

    fn initial_title(
        &self,
        _input: Result<Self::Input, serde_json::Value>,
        _cx: &mut App,
    ) -> SharedString {
        "Enter plan mode".into()
    }

    fn run(
        self: Arc<Self>,
        input: ToolInput<Self::Input>,
        event_stream: ToolCallEventStream,
        cx: &mut App,
    ) -> Task<Result<Self::Output, Self::Output>> {
        cx.spawn(async move |cx| {
            input
                .recv()
                .await
                .map_err(|e| EnterPlanModeToolOutput::Error {
                    error: e.to_string(),
                })?;

            let plan_file = cx
                .update(|cx| event_stream.plan_mode_file(cx))
                .map_err(|error| EnterPlanModeToolOutput::Error {
                    error: error.to_string(),
                })?;

            let ensure_directory_task = cx.update(|cx| event_stream.ensure_plan_file_directory(cx));
            ensure_directory_task
                .await
                .map_err(|error| EnterPlanModeToolOutput::Error {
                    error: error.to_string(),
                })?;

            cx.update(|cx| event_stream.enter_plan_mode(cx))
                .map_err(|error| EnterPlanModeToolOutput::Error {
                    error: error.to_string(),
                })?;

            Ok(EnterPlanModeToolOutput::Entered {
                plan_path: plan_file.display_path.clone(),
                instructions: format!(
                    "Plan mode is active. Write the plan to `{}`. You may only edit this plan file. When the plan is complete, call `exit_plan_mode` to request approval before implementing.",
                    plan_file.display_path
                ),
            })
        })
    }
}

/// Exit plan mode after the plan file is complete.
///
/// This opens the plan file in a read-only editor view and asks the user to
/// approve it. If the user approves, the thread switches to the Write profile so
/// implementation tools can be used. If the user requests changes, remain in
/// plan mode and revise the plan using their feedback.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub struct ExitPlanModeToolInput {}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum ExitPlanModeToolOutput {
    Approved { plan_path: String, message: String },
    Rejected { plan_path: String, feedback: String },
    Canceled { canceled: bool },
    Error { error: String },
}

impl From<ExitPlanModeToolOutput> for LanguageModelToolResultContent {
    fn from(output: ExitPlanModeToolOutput) -> Self {
        serde_json::to_string(&output)
            .unwrap_or_else(|e| format!("Failed to serialize exit_plan_mode output: {e}"))
            .into()
    }
}

pub struct ExitPlanModeTool;

impl AgentTool for ExitPlanModeTool {
    type Input = ExitPlanModeToolInput;
    type Output = ExitPlanModeToolOutput;

    const NAME: &'static str = "exit_plan_mode";

    fn kind() -> acp::ToolKind {
        acp::ToolKind::Other
    }

    fn initial_title(
        &self,
        _input: Result<Self::Input, serde_json::Value>,
        _cx: &mut App,
    ) -> SharedString {
        "Exit plan mode".into()
    }

    fn run(
        self: Arc<Self>,
        input: ToolInput<Self::Input>,
        event_stream: ToolCallEventStream,
        cx: &mut App,
    ) -> Task<Result<Self::Output, Self::Output>> {
        cx.spawn(async move |cx| {
            input.recv().await.map_err(|e| ExitPlanModeToolOutput::Error {
                error: e.to_string(),
            })?;

            let plan_file = cx
                .update(|cx| event_stream.plan_mode_file(cx))
                .map_err(|error| ExitPlanModeToolOutput::Error {
                    error: error.to_string(),
                })?;
            let fs = event_stream.fs().ok_or_else(|| ExitPlanModeToolOutput::Error {
                error: "Cannot read the plan file without an owning filesystem".to_string(),
            })?;
            let markdown = fs
                .load(&plan_file.absolute_path)
                .await
                .map_err(|error| ExitPlanModeToolOutput::Error {
                    error: format!(
                        "Failed to read plan file `{}`: {error}",
                        plan_file.display_path
                    ),
                })?;

            if markdown.trim().is_empty() {
                return Err(ExitPlanModeToolOutput::Error {
                    error: format!(
                        "Plan file `{}` is empty. Write the implementation plan before exiting plan mode.",
                        plan_file.display_path
                    ),
                });
            }

            cx.update(|_cx| {
                event_stream.open_plan_file(PlanFileOpenRequest {
                    display_path: plan_file.display_path.clone(),
                    absolute_path: plan_file.absolute_path.clone(),
                    markdown: markdown.clone(),
                });
                anyhow::Ok(())
            })
            .map_err(|error| ExitPlanModeToolOutput::Error {
                error: error.to_string(),
            })?;

            let response_task = cx.update(|cx| {
                event_stream.request_elicitation(
                    approval_schema(),
                    format!(
                        "Plan at `{}` is complete. Approve it to switch to Write mode and start implementing, or request changes with feedback.",
                        plan_file.display_path
                    ),
                    cx,
                )
            });
            let response = futures::select! {
                result = response_task.fuse() => result.map_err(|error| ExitPlanModeToolOutput::Error {
                    error: error.to_string(),
                })?,
                _ = event_stream.cancelled_by_user().fuse() => {
                    return Err(ExitPlanModeToolOutput::Canceled { canceled: true });
                }
            };

            let acp::ElicitationAction::Accept(action) = response.action else {
                return Err(ExitPlanModeToolOutput::Canceled { canceled: true });
            };

            let decision = approval_from_content(action.content.unwrap_or_default())
                .map_err(|error| ExitPlanModeToolOutput::Error { error })?;
            match decision {
                PlanApproval::Approved => {
                    cx.update(|cx| event_stream.exit_plan_mode(cx)).map_err(|error| {
                        ExitPlanModeToolOutput::Error {
                            error: error.to_string(),
                        }
                    })?;
                    Ok(ExitPlanModeToolOutput::Approved {
                        plan_path: plan_file.display_path.clone(),
                        message: "User approved the plan. Write mode is active; implement the plan.".to_string(),
                    })
                }
                PlanApproval::Rejected { feedback } => Ok(ExitPlanModeToolOutput::Rejected {
                    plan_path: plan_file.display_path.clone(),
                    feedback,
                }),
            }
        })
    }
}

fn approval_schema() -> acp::ElicitationSchema {
    acp::ElicitationSchema::new()
        .property(
            APPROVAL_FIELD,
            acp::StringPropertySchema::new()
                .title("Approval")
                .description("Approve the plan or request changes")
                .one_of(vec![
                    acp::EnumOption::new(APPROVE, "Approve"),
                    acp::EnumOption::new(REQUEST_CHANGES, "Request changes"),
                ])
                .default_value(APPROVE.to_string()),
            true,
        )
        .property(
            FEEDBACK_FIELD,
            acp::StringPropertySchema::new()
                .title("Improvement suggestions")
                .description("If requesting changes, describe what should be improved in the plan"),
            false,
        )
}

enum PlanApproval {
    Approved,
    Rejected { feedback: String },
}

fn approval_from_content(
    mut content: BTreeMap<String, acp::ElicitationContentValue>,
) -> Result<PlanApproval, String> {
    let approval = required_string(&mut content, APPROVAL_FIELD)?;
    match approval.as_str() {
        APPROVE => Ok(PlanApproval::Approved),
        REQUEST_CHANGES => Ok(PlanApproval::Rejected {
            feedback: optional_string(&mut content, FEEDBACK_FIELD)?
                .filter(|feedback| !feedback.trim().is_empty())
                .unwrap_or_else(|| {
                    "The user requested changes but did not provide details.".to_string()
                }),
        }),
        other => Err(format!("Unknown plan approval response: {other}")),
    }
}

fn required_string(
    content: &mut BTreeMap<String, acp::ElicitationContentValue>,
    field: &str,
) -> Result<String, String> {
    optional_string(content, field)?.ok_or_else(|| format!("User response did not include {field}"))
}

fn optional_string(
    content: &mut BTreeMap<String, acp::ElicitationContentValue>,
    field: &str,
) -> Result<Option<String>, String> {
    let Some(value) = content.remove(field) else {
        return Ok(None);
    };

    let value = serde_json::to_value(value)
        .map_err(|error| format!("Failed to read user response: {error}"))?;
    value
        .as_str()
        .map(|value| Some(value.to_string()))
        .ok_or_else(|| format!("User response {field} was not a string"))
}
