use agent_client_protocol::schema::v1 as acp;
use anyhow::Result;
use futures::FutureExt as _;
use gpui::{App, SharedString, Task};
use language_model::LanguageModelToolResultContent;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::{collections::BTreeMap, sync::Arc};

use crate::{AgentTool, ToolCallEventStream, ToolInput};

const ANSWER_FIELD: &str = "answer";
const ANSWERS_FIELD: &str = "answers";
const CUSTOM_ANSWER_FIELD: &str = "custom_answer";

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
    /// Optional placeholder/default text for a free-form answer or the default selected option value.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub default_answer: Option<String>,
    /// Optional choices to present to the user. Omit this for a free-form text answer.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub options: Vec<AskUserQuestionOption>,
    /// Allow selecting multiple options. Only used when options are provided.
    #[serde(default)]
    pub allow_multiple: bool,
    /// Include an optional free-form answer field alongside the provided options.
    #[serde(default)]
    pub allow_custom_answer: bool,
    /// Default selected option values for multi-select questions.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub default_answers: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub struct AskUserQuestionOption {
    /// Display label for this option.
    pub label: String,
    /// Stable value returned to the agent. Defaults to the label.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub value: Option<String>,
    /// Optional explanation for when to choose this option.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
}

impl AskUserQuestionOption {
    fn value(&self) -> String {
        self.value.clone().unwrap_or_else(|| self.label.clone())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum AskUserQuestionToolOutput {
    Answer {
        answer: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        answers: Option<Vec<String>>,
        #[serde(skip_serializing_if = "Option::is_none")]
        custom_answer: Option<String>,
    },
    Canceled {
        canceled: bool,
    },
    Error {
        error: String,
    },
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

            let schema = elicitation_schema(&input);
            let message = message(&input);
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

            let content = action.content.unwrap_or_default();
            answer_from_content(content, &input)
                .map_err(|error| AskUserQuestionToolOutput::Error { error })
        })
    }
}

fn elicitation_schema(input: &AskUserQuestionToolInput) -> acp::ElicitationSchema {
    let has_options = !input.options.is_empty();
    let mut schema = acp::ElicitationSchema::new();

    if !has_options {
        let mut answer_schema = acp::StringPropertySchema::new()
            .title("Answer")
            .description("Your response to the agent");
        if let Some(default_answer) = &input.default_answer {
            answer_schema = answer_schema.default_value(default_answer.clone());
        }
        return schema.property(ANSWER_FIELD, answer_schema, true);
    }

    let options = input
        .options
        .iter()
        .map(|option| acp::EnumOption::new(option.value(), option.label.clone()))
        .collect::<Vec<_>>();
    let description = answer_description(&input.options, input.allow_multiple);
    let requires_option_answer = !input.allow_custom_answer;

    if input.allow_multiple {
        let mut answers_schema = acp::MultiSelectPropertySchema::titled(options)
            .title("Answers")
            .description(description);
        let mut default_answers = input.default_answers.clone();
        if default_answers.is_empty()
            && let Some(default_answer) = &input.default_answer
        {
            default_answers.push(default_answer.clone());
        }
        if !default_answers.is_empty() {
            answers_schema = answers_schema.default_value(default_answers);
        }
        if requires_option_answer {
            answers_schema = answers_schema.min_items(1);
        }
        schema = schema.property(ANSWERS_FIELD, answers_schema, requires_option_answer);
    } else {
        let mut answer_schema = acp::StringPropertySchema::new()
            .title("Answer")
            .description(description)
            .one_of(options);
        if let Some(default_answer) = &input.default_answer {
            answer_schema = answer_schema.default_value(default_answer.clone());
        }
        schema = schema.property(ANSWER_FIELD, answer_schema, requires_option_answer);
    }

    if input.allow_custom_answer {
        schema = schema.property(
            CUSTOM_ANSWER_FIELD,
            acp::StringPropertySchema::new()
                .title("Custom answer")
                .description("Optional free-form answer if none of the choices fit"),
            false,
        );
    }

    schema
}

fn message(input: &AskUserQuestionToolInput) -> String {
    if let Some(context) = input.context.as_ref().filter(|context| !context.is_empty()) {
        format!("{}\n\n{}", input.question, context)
    } else {
        input.question.clone()
    }
}

fn answer_description(options: &[AskUserQuestionOption], allow_multiple: bool) -> String {
    let mut description = if allow_multiple {
        String::from("Choose one or more of the provided options")
    } else {
        String::from("Choose one of the provided options")
    };
    let options_with_descriptions = options
        .iter()
        .filter(|option| {
            option
                .description
                .as_ref()
                .is_some_and(|description| !description.is_empty())
        })
        .collect::<Vec<_>>();
    if options_with_descriptions.is_empty() {
        return description;
    }

    description.push_str(".\n\nOptions:");
    for option in options_with_descriptions {
        description.push_str("\n- ");
        description.push_str(&option.label);
        description.push_str(": ");
        if let Some(option_description) = &option.description {
            description.push_str(option_description);
        }
    }
    description
}

fn answer_from_content(
    mut content: BTreeMap<String, acp::ElicitationContentValue>,
    input: &AskUserQuestionToolInput,
) -> Result<AskUserQuestionToolOutput, String> {
    if input.options.is_empty() {
        let answer = required_string(&mut content, ANSWER_FIELD)?;
        return Ok(AskUserQuestionToolOutput::Answer {
            answer,
            answers: None,
            custom_answer: None,
        });
    }

    let mut answers = if input.allow_multiple {
        optional_string_array(&mut content, ANSWERS_FIELD)?
    } else {
        optional_string(&mut content, ANSWER_FIELD)?
            .into_iter()
            .collect()
    };
    let custom_answer = if input.allow_custom_answer {
        optional_string(&mut content, CUSTOM_ANSWER_FIELD)?
            .filter(|answer| !answer.trim().is_empty())
    } else {
        None
    };

    if let Some(custom_answer) = &custom_answer {
        answers.push(custom_answer.clone());
    }

    if answers.is_empty() {
        return Err("User response did not include an answer".to_string());
    }

    Ok(AskUserQuestionToolOutput::Answer {
        answer: answers.join(", "),
        answers: Some(answers),
        custom_answer,
    })
}

fn required_string(
    content: &mut BTreeMap<String, acp::ElicitationContentValue>,
    field: &str,
) -> Result<String, String> {
    optional_string(content, field)?
        .ok_or_else(|| "User response did not include an answer".to_string())
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

fn optional_string_array(
    content: &mut BTreeMap<String, acp::ElicitationContentValue>,
    field: &str,
) -> Result<Vec<String>, String> {
    let Some(value) = content.remove(field) else {
        return Ok(Vec::new());
    };

    let value = serde_json::to_value(value)
        .map_err(|error| format!("Failed to read user response: {error}"))?;
    let Some(values) = value.as_array() else {
        return Err(format!("User response {field} was not an array"));
    };

    values
        .iter()
        .map(|value| {
            value
                .as_str()
                .map(ToOwned::to_owned)
                .ok_or_else(|| format!("User response {field} included a non-string value"))
        })
        .collect()
}
