use serde::{Deserialize, Serialize};

#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    Hash,
    Serialize,
    Deserialize,
    strum::Display,
    strum::EnumString,
)]
#[serde(rename_all = "snake_case")]
pub enum ConnectorId {
    #[strum(to_string = "Wuling DevOps")]
    Wuling,
    #[strum(to_string = "GitHub")]
    Github,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ConnectorAccount {
    pub connector: ConnectorId,
    pub username: String,
    pub display_name: String,
    pub avatar_url: Option<String>,
    pub profile_url: Option<String>,
}

impl ConnectorAccount {
    pub fn label(&self) -> &str {
        if self.display_name.is_empty() {
            &self.username
        } else {
            &self.display_name
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn falls_back_to_username_for_empty_display_name() {
        let account = ConnectorAccount {
            connector: ConnectorId::Github,
            username: "amiya".to_string(),
            display_name: String::new(),
            avatar_url: None,
            profile_url: None,
        };

        assert_eq!(account.label(), "amiya");
    }
}
