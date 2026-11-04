//! Update notice: compares the running version with the latest GitHub release
//! of the project. It runs only when the user asks (the button in the
//! settings) or opted into the check at startup; the request carries nothing
//! but the version in its user agent. Installing is a separate step the user
//! starts (see `updater`); nothing is downloaded on its own.

use std::time::Duration;

use serde::Serialize;

/// Latest published release, per GitHub's REST API.
const RELEASES_API_URL: &str =
    "https://api.github.com/repos/EduardoViniciusJ/AnyCaptions/releases/latest";
/// The page the app opens for the user to download an update (a constant, so
/// the webview and the API response can never steer the browser elsewhere).
pub const RELEASES_PAGE_URL: &str =
    "https://github.com/EduardoViniciusJ/AnyCaptions/releases/latest";
pub const CURRENT_VERSION: &str = env!("CARGO_PKG_VERSION");

/// Responses larger than this are not release metadata.
const MAX_RESPONSE_BYTES: u64 = 1024 * 1024;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateCheck {
    pub current: String,
    pub latest: String,
    pub available: bool,
    /// Whether this build can install the update by itself.
    pub installable: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum UpdateError {
    Network,
    Malformed,
}

/// Where the latest release is read from; `ANYCAPTIONS_UPDATE_URL` overrides
/// it during development and tests.
fn api_url() -> String {
    std::env::var("ANYCAPTIONS_UPDATE_URL")
        .ok()
        .filter(|url| !url.is_empty())
        .unwrap_or_else(|| RELEASES_API_URL.to_owned())
}

pub fn check() -> Result<UpdateCheck, UpdateError> {
    check_at(&api_url())
}

/// Blocking; callers run it off the UI thread.
pub(crate) fn check_at(url: &str) -> Result<UpdateCheck, UpdateError> {
    let agent: ureq::Agent = ureq::Agent::config_builder()
        .timeout_global(Some(Duration::from_secs(15)))
        .build()
        .into();
    let mut response = agent
        .get(url)
        .header("Accept", "application/vnd.github+json")
        .header("User-Agent", &format!("AnyCaptions/{CURRENT_VERSION}"))
        .call()
        .map_err(|_| UpdateError::Network)?;
    let body = response
        .body_mut()
        .with_config()
        .limit(MAX_RESPONSE_BYTES)
        .read_to_string()
        .map_err(|_| UpdateError::Network)?;
    let release: serde_json::Value =
        serde_json::from_str(&body).map_err(|_| UpdateError::Malformed)?;
    let tag = release
        .get("tag_name")
        .and_then(|tag| tag.as_str())
        .ok_or(UpdateError::Malformed)?;
    let latest = tag.trim().trim_start_matches(['v', 'V']).to_owned();
    if latest.is_empty() || latest.len() > 64 {
        return Err(UpdateError::Malformed);
    }
    Ok(UpdateCheck {
        current: CURRENT_VERSION.to_owned(),
        available: newer(&latest, CURRENT_VERSION),
        installable: crate::updater::installable(),
        latest,
    })
}

/// Whether `latest` is a higher version than `current`: numeric components
/// compared in order (missing ones count as 0); a pre-release suffix after
/// `-` is ignored, so `0.2.0-beta` counts as `0.2.0`. Unparsable tags are
/// never "newer", so a renamed tag cannot nag the user.
pub(crate) fn newer(latest: &str, current: &str) -> bool {
    fn parts(version: &str) -> Option<Vec<u64>> {
        let core = version.split('-').next().unwrap_or_default();
        core.split('.')
            .map(|part| part.parse::<u64>().ok())
            .collect::<Option<Vec<u64>>>()
            .filter(|parts| !parts.is_empty())
    }
    let (Some(latest), Some(current)) = (parts(latest), parts(current)) else {
        return false;
    };
    let len = latest.len().max(current.len());
    let at = |parts: &[u64], index: usize| parts.get(index).copied().unwrap_or(0);
    (0..len).any(|index| at(&latest, index) != at(&current, index))
        && (0..len)
            .map(|index| (at(&latest, index), at(&current, index)))
            .find(|(left, right)| left != right)
            .is_some_and(|(left, right)| left > right)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{Read, Write};

    /// Serves one HTTP response with `body` and closes the connection.
    fn serve(body: &'static str) -> String {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        std::thread::spawn(move || {
            if let Ok((mut stream, _)) = listener.accept() {
                let mut request = [0_u8; 2048];
                let _ = stream.read(&mut request);
                let header = format!(
                    "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                    body.len()
                );
                let _ = stream.write_all(header.as_bytes());
                let _ = stream.write_all(body.as_bytes());
            }
        });
        format!("http://{addr}/releases/latest")
    }

    #[test]
    fn version_comparison_follows_numeric_components() {
        assert!(newer("0.1.2", "0.1.1"));
        assert!(newer("0.2", "0.1.9"));
        assert!(newer("1.0.0", "0.9.9"));
        assert!(newer("0.1.1.1", "0.1.1"));
        assert!(!newer("0.1.1", "0.1.1"));
        assert!(!newer("0.1.0", "0.1.1"));
        assert!(!newer("0.1.1", "0.1.1.0"));
        // Pre-release suffixes are ignored; garbage never counts as newer.
        assert!(newer("0.2.0-beta", "0.1.1"));
        assert!(!newer("latest", "0.1.1"));
        assert!(!newer("", "0.1.1"));
        assert!(!newer("0.1.2", "nightly"));
    }

    #[test]
    fn a_newer_release_is_reported_and_malformed_metadata_is_rejected() {
        let url = serve(r#"{"tag_name":"v9.9.9","html_url":"https://example.invalid/r"}"#);
        let check = check_at(&url).unwrap();
        assert_eq!(check.latest, "9.9.9");
        assert_eq!(check.current, CURRENT_VERSION);
        assert!(check.available);

        let same: &'static str = format!(r#"{{"tag_name":"v{CURRENT_VERSION}"}}"#).leak();
        let url = serve(same);
        let check = check_at(&url).unwrap();
        assert!(!check.available);

        let url = serve(r#"{"message":"Not Found"}"#);
        assert_eq!(check_at(&url), Err(UpdateError::Malformed));
        let url = serve("<html>");
        assert_eq!(check_at(&url), Err(UpdateError::Malformed));
        assert_eq!(
            check_at("http://127.0.0.1:1/releases/latest"),
            Err(UpdateError::Network)
        );
    }
}
