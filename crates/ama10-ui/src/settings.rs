use ama10::server_url::ServerUrl;
use settings::{ConnectorAvatarSource, RegisterSetting, Settings, SettingsContent};

#[derive(Clone, Debug, RegisterSetting)]
pub struct ConnectorSettings {
    pub avatar_source: ConnectorAvatarSource,
    pub wuling_server: ServerUrl,
    pub github_client_id: String,
}

impl Settings for ConnectorSettings {
    fn from_settings(settings: &SettingsContent) -> Self {
        let connectors = settings.connectors.as_ref();
        let raw_server = connectors
            .and_then(|connectors| connectors.wuling.as_ref())
            .and_then(|wuling| wuling.server_url.as_deref())
            .unwrap_or(ama10::server_url::DEFAULT_SERVER_URL);
        let wuling_server = ServerUrl::parse(raw_server).unwrap_or_else(|error| {
            log::warn!(
                "ama10: invalid connectors.wuling.server_url {raw_server:?}: {error}; using the default"
            );
            ServerUrl::default_saas()
        });
        let github_client_id = connectors
            .and_then(|connectors| connectors.github.as_ref())
            .and_then(|github| github.client_id.clone())
            .unwrap_or_default();

        Self {
            avatar_source: connectors
                .and_then(|connectors| connectors.avatar_source)
                .unwrap_or_default(),
            wuling_server,
            github_client_id,
        }
    }
}

#[derive(Debug, Clone)]
pub struct WulingConfig {
    pub server: ServerUrl,
}

impl WulingConfig {
    pub fn load(cx: &gpui::App) -> Self {
        Self {
            server: ConnectorSettings::get_global(cx).wuling_server.clone(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn invalid_wuling_url_falls_back_to_default() {
        let mut content = SettingsContent::default();
        content
            .connectors
            .get_or_insert_default()
            .wuling
            .get_or_insert_default()
            .server_url = Some("file:///tmp/wuling".to_string());

        let settings = ConnectorSettings::from_settings(&content);
        assert_eq!(
            settings.wuling_server.as_str(),
            ama10::server_url::DEFAULT_SERVER_URL
        );
    }

    #[test]
    fn loads_connector_values() {
        let mut content = SettingsContent::default();
        let connectors = content.connectors.get_or_insert_default();
        connectors.avatar_source = Some(ConnectorAvatarSource::Github);
        connectors.wuling.get_or_insert_default().server_url =
            Some("https://wuling.example/".to_string());
        connectors.github.get_or_insert_default().client_id = Some("client-id".to_string());

        let settings = ConnectorSettings::from_settings(&content);
        assert_eq!(settings.avatar_source, ConnectorAvatarSource::Github);
        assert_eq!(settings.wuling_server.as_str(), "https://wuling.example");
        assert_eq!(settings.github_client_id, "client-id");
    }
}
