use serde::{Deserialize, Serialize};
use url::Url;

/// Capability granting an extension permission to perform HTTP fetch requests.
///
/// Host and path matching follows the same rules as [`DownloadFileCapability`].
#[derive(Debug, PartialEq, Eq, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct HttpFetchCapability {
    pub host: String,
    pub path: Vec<String>,
}

impl HttpFetchCapability {
    /// Returns whether the capability allows fetching the given URL.
    pub fn allows(&self, url: &Url) -> bool {
        let Some(desired_host) = url.host_str() else {
            return false;
        };

        let Some(desired_path) = url.path_segments() else {
            return false;
        };
        let desired_path = desired_path.collect::<Vec<_>>();

        if self.host != desired_host && self.host != "*" {
            return false;
        }

        for (ix, path_segment) in self.path.iter().enumerate() {
            if path_segment == "**" {
                return true;
            }

            if ix >= desired_path.len() {
                return false;
            }

            if path_segment != "*" && path_segment != desired_path[ix] {
                return false;
            }
        }

        if self.path.len() < desired_path.len() {
            return false;
        }

        true
    }
}

#[cfg(test)]
mod tests {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn test_allows_api_host() {
        let capability = HttpFetchCapability {
            host: "api.github.com".to_string(),
            path: vec!["**".to_string()],
        };
        assert_eq!(
            capability.allows(
                &"https://api.github.com/repos/zed/zed/pulls"
                    .parse()
                    .unwrap()
            ),
            true
        );
        assert_eq!(
            capability.allows(&"https://gitlab.com/api/v4/projects".parse().unwrap()),
            false
        );
    }
}
