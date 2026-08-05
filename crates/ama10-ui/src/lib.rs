//! Connector accounts and authentication UI for Kal'tsit·Esperanta.
//!
//! Wuling DevOps and GitHub share the same platform-neutral account state,
//! device-flow modal, credential storage boundary, and title-bar integration.
//! Pipeline configurators (runner-config + workflows) live alongside that.

pub mod account_state;
pub mod askpass_delegate;
pub mod job_flow_canvas;
pub mod runner_config_view;
pub mod settings;
pub mod sign_in;
pub mod sign_in_modal;
pub mod workflow_view;
pub mod wuling_client;

use gpui::{App, actions};

pub use account_state::{ConnectorAccountState, ConnectorAccountsChanged};
pub use ama10::connector::{ConnectorAccount, ConnectorId};
pub use askpass_delegate::{LookupResult, lookup_for_host};
pub use runner_config_view::{OpenRunnerConfig, open_or_reuse_runner_config};
pub use settings::{ConnectorSettings, WulingConfig};
pub use sign_in::spawn_disconnect;
pub use sign_in_modal::open_sign_in_modal;
pub use workflow_view::{OpenWulingWorkflow, open_or_reuse_workflow};

pub const DEFAULT_SERVER_URL: &str = ama10::server_url::DEFAULT_SERVER_URL;

actions!(
    ama10,
    [
        /// Begin the OAuth 2.1 Device Authorization Grant flow against the
        /// configured Wuling DevOps server, prompting the user to visit the
        /// verification URL and enter the displayed user_code.
        ConnectWuling,
        /// Revoke the stored Wuling DevOps access token and clear the
        /// platform keychain entry. Best-effort: a network failure during
        /// revoke still clears the local credentials.
        DisconnectWuling,
        /// Connect a GitHub account with an OAuth App device flow.
        ConnectGithub,
        /// Remove the locally stored GitHub OAuth token.
        DisconnectGithub,
    ]
);

pub fn init(cx: &mut App) {
    use ::settings::Settings as _;
    ConnectorSettings::register(cx);
    runner_config_view::init(cx);
    workflow_view::init(cx);
}
