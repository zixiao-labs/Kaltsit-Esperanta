use serde::{Deserialize, Serialize};

/// Metadata describing a pull-request / merge-request provider contributed by an extension.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PullRequestProviderMetadata {
    pub id: String,
    pub label: String,
    #[serde(default)]
    pub supports_review_comments: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ParsedGitRemote {
    pub owner: String,
    pub repo: String,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct PullRequestQuery {
    #[serde(default)]
    pub query: Option<String>,
    #[serde(default)]
    pub limit: Option<u32>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PullRequestSummary {
    pub number: u32,
    pub title: String,
    pub url: String,
    #[serde(default)]
    pub author: Option<String>,
    #[serde(default)]
    pub is_draft: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReviewBatchComment {
    pub file_path: String,
    pub start_line: u32,
    pub end_line: u32,
    pub body: String,
    #[serde(default)]
    pub excerpt: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReviewBatch {
    pub comments: Vec<ReviewBatchComment>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PullRequestReviewThread {
    pub id: String,
    pub file_path: String,
    pub start_line: u32,
    pub end_line: u32,
    #[serde(default)]
    pub excerpt: Option<String>,
    #[serde(default)]
    pub is_resolved: bool,
    pub comments: Vec<PullRequestReviewComment>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PullRequestReviewComment {
    pub id: String,
    pub author: String,
    pub body: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PullRequestDetail {
    pub summary: PullRequestSummary,
    pub files: Vec<String>,
    pub threads: Vec<PullRequestReviewThread>,
}
