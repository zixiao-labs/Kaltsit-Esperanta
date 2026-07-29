use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use settings_macros::MergeFrom;

#[derive(
    Copy,
    Clone,
    Debug,
    Default,
    PartialEq,
    Eq,
    Serialize,
    Deserialize,
    JsonSchema,
    MergeFrom,
    strum::VariantArray,
    strum::VariantNames,
)]
#[serde(rename_all = "snake_case")]
pub enum ConnectorAvatarSource {
    #[default]
    Zed,
    #[strum(serialize = "Wuling DevOps")]
    Wuling,
    #[strum(serialize = "GitHub")]
    Github,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, JsonSchema, MergeFrom)]
pub struct WulingConnectorSettingsContent {
    /// Base URL of the Wuling DevOps instance.
    pub server_url: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, JsonSchema, MergeFrom)]
pub struct GithubConnectorSettingsContent {
    /// Client ID of a GitHub OAuth App with Device Flow enabled.
    pub client_id: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, JsonSchema, MergeFrom)]
pub struct ConnectorsSettingsContent {
    /// Account whose avatar is displayed in the title bar.
    pub avatar_source: Option<ConnectorAvatarSource>,
    pub wuling: Option<WulingConnectorSettingsContent>,
    pub github: Option<GithubConnectorSettingsContent>,
}
