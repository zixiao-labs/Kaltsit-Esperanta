//! Default Wuling DevOps server URL and a small helper for normalising
//! user-entered overrides.
//!
//! Esperanta is intended for both the SaaS instance and self-hosted
//! deployments; we ship a default so the SaaS case works zero-config, while
//! still letting users point at their company instance via a setting. The
//! actual settings plumbing lives in `ama10-ui::settings`; this file is just
//! the constant and a parser so the SDK doesn't depend on the GPUI
//! settings layer.

use std::fmt;

/// The default SaaS Wuling DevOps instance.
pub const DEFAULT_SERVER_URL: &str = "https://wuling.zixiaolabs.com";

/// A validated server URL. Always rendered with no trailing slash so we can
/// concatenate `format!("{base}/api/v1/...")` without worrying about doubled
/// slashes — Wuling's chi router treats `/api//v1` and `/api/v1` as
/// different routes.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ServerUrl(String);

impl ServerUrl {
    /// Parse a user-entered server URL. Accepts `https://host`, `http://host`,
    /// or a bare host (assumed https). Returns an `anyhow::Error` rather than
    /// a typed error because every caller bubbles up to a toast / error
    /// banner — losing the variant nuance is fine.
    pub fn parse(input: &str) -> anyhow::Result<Self> {
        let trimmed = input.trim();
        if trimmed.is_empty() {
            anyhow::bail!("server URL is empty");
        }
        let with_scheme = if trimmed.contains("://") {
            trimmed.to_string()
        } else {
            format!("https://{trimmed}")
        };
        let parsed = url::Url::parse(&with_scheme)?;
        match parsed.scheme() {
            "http" | "https" => {}
            other => anyhow::bail!("unsupported scheme: {other}"),
        }
        if parsed.host().is_none() {
            anyhow::bail!("missing host");
        }
        // Strip path/query/fragment — only the origin is meaningful as a
        // server URL.
        let mut canonical = format!("{}://{}", parsed.scheme(), parsed.host_str().unwrap());
        if let Some(port) = parsed.port() {
            canonical.push(':');
            canonical.push_str(&port.to_string());
        }
        Ok(ServerUrl(canonical))
    }

    /// Construct an unchecked ServerUrl from a string already known to be
    /// canonical. Intended for the compile-time default and for tests.
    pub fn from_canonical(s: &str) -> Self {
        ServerUrl(s.trim_end_matches('/').to_string())
    }

    /// The default SaaS server.
    pub fn default_saas() -> Self {
        ServerUrl(DEFAULT_SERVER_URL.to_string())
    }

    /// Return the canonical origin string (no trailing slash).
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Build an absolute URL by appending `path` (which must start with `/`).
    pub fn join(&self, path: &str) -> String {
        if path.is_empty() {
            self.0.clone()
        } else if path.starts_with('/') {
            format!("{}{}", self.0, path)
        } else {
            format!("{}/{}", self.0, path)
        }
    }

    /// Just the host (used as the cache key when matching git push URLs).
    pub fn host(&self) -> &str {
        let rest = self
            .0
            .strip_prefix("https://")
            .or_else(|| self.0.strip_prefix("http://"))
            .unwrap_or(&self.0);
        if let Some(after_bracket) = rest.strip_prefix('[') {
            if let Some(end) = after_bracket.find(']') {
                return &rest[..end + 2];
            }
        }
        rest.split(':').next().unwrap_or(rest)
    }
}

impl fmt::Display for ServerUrl {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl Default for ServerUrl {
    fn default() -> Self {
        Self::default_saas()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_bare_host() {
        let s = ServerUrl::parse("wuling.zixiaolabs.com").unwrap();
        assert_eq!(s.as_str(), "https://wuling.zixiaolabs.com");
    }

    #[test]
    fn keeps_port_and_scheme() {
        let s = ServerUrl::parse("http://127.0.0.1:8080").unwrap();
        assert_eq!(s.as_str(), "http://127.0.0.1:8080");
        assert_eq!(s.host(), "127.0.0.1");
    }

    #[test]
    fn strips_trailing_path() {
        let s = ServerUrl::parse("https://host.example/wat?x=y").unwrap();
        assert_eq!(s.as_str(), "https://host.example");
    }

    #[test]
    fn rejects_empty() {
        assert!(ServerUrl::parse("").is_err());
        assert!(ServerUrl::parse("   ").is_err());
    }

    #[test]
    fn join_is_one_slash() {
        let s = ServerUrl::parse("https://host.example").unwrap();
        assert_eq!(s.join("/api/v1/me"), "https://host.example/api/v1/me");
    }

    #[test]
    fn ipv6_host_keeps_brackets() {
        let s = ServerUrl::parse("http://[::1]:8080").unwrap();
        assert_eq!(s.as_str(), "http://[::1]:8080");
        assert_eq!(s.host(), "[::1]");
    }
}
