use agent_client_protocol::schema::v1 as acp;
use anyhow::Result;
use gpui::{App, SharedString, Task};
use language_model::LanguageModelToolResultContent;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

use crate::{AgentTool, ToolCallEventStream, ToolInput};

/// Replace the current visible plan for the agent thread.
///
/// Use this for multi-step tasks to show the user what is being worked on and
/// keep step status current as work progresses. Always send the complete list of
/// plan entries; the previous plan is replaced by each update.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub struct UpdatePlanToolInput {
    /// Complete ordered list of plan entries to display.
    pub entries: Vec<UpdatePlanEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub struct UpdatePlanEntry {
    /// Human-readable task description.
    pub content: String,
    /// Current execution status for this task.
    #[serde(default)]
    pub status: UpdatePlanEntryStatus,
    /// Relative importance of this task.
    #[serde(default)]
    pub priority: UpdatePlanEntryPriority,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum UpdatePlanEntryStatus {
    /// The task has not started yet.
    Pending,
    /// The task is currently being worked on.
    InProgress,
    /// The task has been completed successfully.
    Completed,
}

impl Default for UpdatePlanEntryStatus {
    fn default() -> Self {
        Self::Pending
    }
}

impl From<UpdatePlanEntryStatus> for acp::PlanEntryStatus {
    fn from(status: UpdatePlanEntryStatus) -> Self {
        match status {
            UpdatePlanEntryStatus::Pending => acp::PlanEntryStatus::Pending,
            UpdatePlanEntryStatus::InProgress => acp::PlanEntryStatus::InProgress,
            UpdatePlanEntryStatus::Completed => acp::PlanEntryStatus::Completed,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum UpdatePlanEntryPriority {
    /// Critical to the overall goal.
    High,
    /// Important but not critical.
    Medium,
    /// Nice to have or supporting work.
    Low,
}

impl Default for UpdatePlanEntryPriority {
    fn default() -> Self {
        Self::Medium
    }
}

impl From<UpdatePlanEntryPriority> for acp::PlanEntryPriority {
    fn from(priority: UpdatePlanEntryPriority) -> Self {
        match priority {
            UpdatePlanEntryPriority::High => acp::PlanEntryPriority::High,
            UpdatePlanEntryPriority::Medium => acp::PlanEntryPriority::Medium,
            UpdatePlanEntryPriority::Low => acp::PlanEntryPriority::Low,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum UpdatePlanToolOutput {
    Updated { entries: Vec<UpdatePlanEntry> },
    Error { error: String },
}

impl From<UpdatePlanToolOutput> for LanguageModelToolResultContent {
    fn from(output: UpdatePlanToolOutput) -> Self {
        serde_json::to_string(&output)
            .unwrap_or_else(|e| format!("Failed to serialize update_plan output: {e}"))
            .into()
    }
}

pub struct UpdatePlanTool;

impl AgentTool for UpdatePlanTool {
    type Input = UpdatePlanToolInput;
    type Output = UpdatePlanToolOutput;

    const NAME: &'static str = "update_plan";

    fn kind() -> acp::ToolKind {
        acp::ToolKind::Other
    }

    fn initial_title(
        &self,
        input: Result<Self::Input, serde_json::Value>,
        _cx: &mut App,
    ) -> SharedString {
        match input {
            Ok(input) => format!("Update plan ({} steps)", input.entries.len()).into(),
            Err(_) => "Update plan".into(),
        }
    }

    fn run(
        self: Arc<Self>,
        input: ToolInput<Self::Input>,
        event_stream: ToolCallEventStream,
        cx: &mut App,
    ) -> Task<Result<Self::Output, Self::Output>> {
        cx.spawn(async move |_cx| {
            let input = input
                .recv()
                .await
                .map_err(|e| UpdatePlanToolOutput::Error {
                    error: e.to_string(),
                })?;

            let plan_entries = input
                .entries
                .iter()
                .cloned()
                .map(|entry| {
                    acp::PlanEntry::new(entry.content, entry.priority.into(), entry.status.into())
                })
                .collect();
            event_stream.update_plan(acp::Plan::new(plan_entries));

            Ok(UpdatePlanToolOutput::Updated {
                entries: input.entries,
            })
        })
    }
}
