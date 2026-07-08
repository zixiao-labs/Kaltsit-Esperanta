use agent_client_protocol::schema::v1 as acp;
use anyhow::Result;
use futures::FutureExt as _;
use gpui::{App, SharedString, Task};
use language_model::LanguageModelToolResultContent;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

use crate::{AgentTool, ToolCallEventStream, ToolInput};

const ANSWER_FIELD: &str = "answer";

/// Ask the user a question and wait for their response.
///
/// Use this when you need information or approval that cannot be inferred from
/// the project. Keep questions concise and include the decision or missing
/// detail you need from the user.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub struct AskUserQuestionToolInput {
    /// The question to show to the user.
    pub question: String,
    /// Optional additional context explaining why the answer is needed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub context: Option<String>,
    /// Optional placeholder/default text for the answer field.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub default_answer: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum AskUserQuestionToolOutput {
    Answer { answer: String },
    Canceled { canceled: bool },
    Error { error: String },
}

impl From<AskUserQuestionToolOutput> for LanguageModelToolResultContent {
    fn from(output: AskUserQuestionToolOutput) -> Self {
        serde_json::to_string(&output)
            .unwrap_or_else(|e| format!("Failed to serialize ask_user_question output: {e}"))
            .into()
    }
}

pub struct AskUserQuestionTool;

impl AgentTool for AskUserQuestionTool {
    type Input = AskUserQuestionToolInput;
    type Output = AskUserQuestionToolOutput;

    const NAME: &'static str = "ask_user_question";

    fn kind() -> acp::ToolKind {
        acp::ToolKind::Other
    }

    fn initial_title(
        &self,
        input: Result<Self::Input, serde_json::Value>,
        _cx: &mut App,
    ) -> SharedString {
        match input {
            Ok(input) => format!("Ask user: {}", input.question).into(),
            Err(_) => "Ask user".into(),
        }
    }

    fn run(
        self: Arc<Self>,
        input: ToolInput<Self::Input>,
        event_stream: ToolCallEventStream,
        cx: &mut App,
    ) -> Task<Result<Self::Output, Self::Output>> {
        cx.spawn(async move |cx| {
            let input = input.recv().await.map_err(|e| AskUserQuestionToolOutput::Error {
                error: e.to_string(),
            })?;

            let mut answer_schema = acp::StringPropertySchema::new()
                .title("Answer")
                .description("Your response to the agent");
            if let Some(default_answer) = input.default_answer {
                answer_schema = answer_schema.default_value(default_answer);
            }

            let schema = acp::ElicitationSchema::new().property(ANSWER_FIELD, answer_schema, true);
            let message = if let Some(context) = input.context.filter(|context| !context.is_empty()) {
                format!("{}\n\n{}", input.question, context)
            } else {
                input.question
            };

            let response_task = cx.update(|cx| event_stream.request_elicitation(schema, message, cx));
            let response = futures::select! {
                result = response_task.fuse() => result.map_err(|e| AskUserQuestionToolOutput::Error {
                    error: e.to_string(),
                })?,
                _ = event_stream.cancelled_by_user().fuse() => {
                    return Err(AskUserQuestionToolOutput::Canceled { canceled: true });
                }
            };

            let acp::ElicitationAction::Accept(action) = response.action else {
                return Err(AskUserQuestionToolOutput::Canceled { canceled: true });
            };

            let Some(value) = action.content.and_then(|mut content| content.remove(ANSWER_FIELD)) else {
                return Err(AskUserQuestionToolOutput::Error {
                    error: "User response did not include an answer".to_string(),
                });
            };

            let answer = serde_json::to_value(value)
                .ok()
                .and_then(|value| value.as_str().map(ToOwned::to_owned))
                .ok_or_else(|| AskUserQuestionToolOutput::Error {
                    error: "User response answer was not a string".to_string(),
                })?;

            Ok(AskUserQuestionToolOutput::Answer { answer })
        })
    }
}
