//! Narrow Wuling DevOps API types generated from the vendored OpenAPI contract.
//!
//! The desktop integration only consumes discovery, device authorization,
//! token, and current-user payloads. Generating those serde types from a small
//! schema projection keeps the API contract checked while avoiding a generated
//! client for thousands of unrelated endpoints.

pub mod types {
    typify::import_types!("api/wuling-client-types.json");
}

pub(crate) fn user_agent(component: &str) -> String {
    format!(
        "Kaltsit-Esperanta/{} ({component}; +https://github.com/zixiao-labs/Kaltsit-Esperanta)",
        env!("CARGO_PKG_VERSION")
    )
}

#[cfg(test)]
mod tests {
    use super::types::{DeviceCodeResponse, OAuthTokenResponse, User, WellKnownDoc};

    #[test]
    fn generated_types_decode_client_payloads() {
        let well_known = serde_json::from_value::<WellKnownDoc>(serde_json::json!({
            "issuer": "https://wuling.example",
            "desktop_official_client_id": "desktop",
            "authorization_endpoint": "https://wuling.example/oauth/authorize",
            "token_endpoint": "https://wuling.example/oauth/token",
            "device_authorization_endpoint": "https://wuling.example/oauth/device",
            "revocation_endpoint": "https://wuling.example/oauth/revoke",
            "frontend_device_verification_uri": "https://wuling.example/device",
            "scopes_supported": ["user:read"]
        }))
        .expect("well-known fixture should match the generated type");
        assert_eq!(well_known.desktop_official_client_id, "desktop");

        let device_code = serde_json::from_value::<DeviceCodeResponse>(serde_json::json!({
            "device_code": "device",
            "user_code": "ABCD-1234",
            "verification_uri": "https://wuling.example/device",
            "verification_uri_complete": "https://wuling.example/device?code=ABCD-1234",
            "expires_in": 900,
            "interval": 5
        }))
        .expect("device-code fixture should match the generated type");
        assert_eq!(device_code.interval, 5);

        let token = serde_json::from_value::<OAuthTokenResponse>(serde_json::json!({
            "access_token": "access",
            "token_type": "Bearer",
            "expires_in": 3600,
            "refresh_token": "refresh",
            "scope": "user:read"
        }))
        .expect("token fixture should match the generated type");
        assert_eq!(token.refresh_token.as_deref(), Some("refresh"));

        let user = serde_json::from_value::<User>(serde_json::json!({
            "id": "00000000-0000-0000-0000-000000000001",
            "username": "amiya",
            "email": "amiya@example.com",
            "display_name": "Amiya",
            "avatar_url": "https://wuling.example/api/v1/users/amiya/avatar?v=1",
            "github_login": "Amiya167"
        }))
        .expect("user fixture should match the generated type");
        assert_eq!(
            user.avatar_url,
            "https://wuling.example/api/v1/users/amiya/avatar?v=1"
        );
    }
}
