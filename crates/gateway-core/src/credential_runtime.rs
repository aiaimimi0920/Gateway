use serde::{Deserialize, Serialize};

/// Session-backed auth material carried alongside a normal credential record.
///
/// The gateway still routes and sends requests using the unified credential
/// pipeline; this metadata only describes how the current credential should be
/// rendered on the wire and when a keepalive preflight is needed.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SessionAuthConfig {
    /// Session transport style. Today we primarily use `cookie`.
    #[serde(default = "default_session_transport")]
    pub transport: String,
    /// Cookie/header name that receives the primary session token.
    #[serde(default, rename = "primaryCookieName", alias = "primary_cookie_name")]
    pub primary_cookie_name: Option<String>,
    /// Optional mirrored cookie name that receives the same token.
    #[serde(
        default,
        rename = "secondaryCookieName",
        alias = "secondary_cookie_name"
    )]
    pub secondary_cookie_name: Option<String>,
    /// Header name used when the session material rides on a bearer/custom header.
    #[serde(default, rename = "headerName", alias = "header_name")]
    pub header_name: Option<String>,
    /// Optional session expiry time. If present, the gateway can trigger
    /// keepalive before the session becomes stale.
    #[serde(default, rename = "expiresAt", alias = "expires_at")]
    pub expires_at: Option<String>,
}

impl Default for SessionAuthConfig {
    fn default() -> Self {
        Self::cookie_defaults()
    }
}

impl SessionAuthConfig {
    pub fn cookie_defaults() -> Self {
        Self {
            transport: default_session_transport(),
            primary_cookie_name: Some("sso".to_string()),
            secondary_cookie_name: Some("sso-rw".to_string()),
            header_name: None,
            expires_at: None,
        }
    }

    pub fn primary_cookie_name(&self) -> &str {
        self.primary_cookie_name.as_deref().unwrap_or("sso")
    }

    pub fn secondary_cookie_name(&self) -> Option<&str> {
        self.secondary_cookie_name.as_deref()
    }

    pub fn header_name(&self) -> Option<&str> {
        self.header_name
            .as_deref()
            .or_else(|| match self.transport.as_str() {
                "bearer" => Some("authorization"),
                "header" => Some("x-session-token"),
                _ => None,
            })
    }

    pub fn expires_within_secs(&self, window_secs: u64) -> bool {
        match self
            .expires_at
            .as_deref()
            .and_then(parse_rfc3339_utc_millis)
        {
            Some(expires_ms) => {
                let now_ms = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_millis() as i128;
                expires_ms - now_ms <= (window_secs as i128 * 1000)
            }
            None => true,
        }
    }
}

/// Configuration for an external keepalive service that refreshes or validates
/// session-backed credentials.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct KeepaliveConfig {
    /// Base URL of the keepalive service.
    #[serde(rename = "serviceUrl", alias = "service_url")]
    pub service_url: String,
    /// Ensure endpoint path. Defaults to `/v1/credentials/ensure`.
    #[serde(default, rename = "ensurePath", alias = "ensure_path")]
    pub ensure_path: Option<String>,
    /// Optional bearer token used to authenticate to the keepalive service.
    #[serde(default, rename = "authToken", alias = "auth_token")]
    pub auth_token: Option<String>,
    /// Per-call timeout in seconds. Defaults to 10.
    #[serde(default, rename = "timeoutSecs", alias = "timeout_secs")]
    pub timeout_secs: Option<u64>,
    /// Trigger keepalive when the session expires within this many seconds.
    /// Defaults to 300 seconds.
    #[serde(default, rename = "refreshBeforeSecs", alias = "refresh_before_secs")]
    pub refresh_before_secs: Option<u64>,
}

impl KeepaliveConfig {
    pub fn ensure_url(&self) -> String {
        format!(
            "{}{}",
            self.service_url.trim().trim_end_matches('/'),
            self.ensure_path
                .as_deref()
                .unwrap_or("/v1/credentials/ensure")
        )
    }

    pub fn timeout_secs(&self) -> u64 {
        self.timeout_secs.unwrap_or(10)
    }

    pub fn refresh_before_secs(&self) -> u64 {
        self.refresh_before_secs.unwrap_or(300)
    }

    pub fn should_ensure(&self, session_auth: Option<&SessionAuthConfig>) -> bool {
        match session_auth {
            Some(session) => session.expires_within_secs(self.refresh_before_secs()),
            None => true,
        }
    }
}

fn default_session_transport() -> String {
    "cookie".to_string()
}

fn parse_rfc3339_utc_millis(s: &str) -> Option<i128> {
    let s = s.trim().trim_end_matches('Z');
    let (date_part, time_part) = s.split_once('T')?;

    let mut date_parts = date_part.splitn(3, '-');
    let year: i128 = date_parts.next()?.parse().ok()?;
    let month: i128 = date_parts.next()?.parse().ok()?;
    let day: i128 = date_parts.next()?.parse().ok()?;

    let (time_no_frac, millis) = match time_part.split_once('.') {
        Some((base, frac)) => {
            let mut ms = frac.chars().take(3).collect::<String>();
            while ms.len() < 3 {
                ms.push('0');
            }
            (base, ms.parse::<i128>().ok().unwrap_or(0))
        }
        None => (time_part, 0),
    };

    let mut time_parts = time_no_frac.splitn(3, ':');
    let hour: i128 = time_parts.next()?.parse().ok()?;
    let minute: i128 = time_parts.next()?.parse().ok()?;
    let second: i128 = time_parts.next().unwrap_or("0").parse().ok()?;

    let y = if month <= 2 { year - 1 } else { year };
    let m = if month <= 2 { month + 9 } else { month - 3 };
    let days = 365 * y + y / 4 - y / 100 + y / 400 + (153 * m + 2) / 5 + day - 719_469;
    Some((days * 86_400 + hour * 3_600 + minute * 60 + second) * 1_000 + millis)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn session_defaults_use_cookie_transport() {
        let session = SessionAuthConfig::default();
        assert_eq!(session.transport, "cookie");
        assert_eq!(session.primary_cookie_name(), "sso");
        assert_eq!(session.secondary_cookie_name(), Some("sso-rw"));
    }

    #[test]
    fn keepalive_defaults_build_standard_ensure_url() {
        let cfg = KeepaliveConfig {
            service_url: "http://grok-keepalive:8080/".to_string(),
            ensure_path: None,
            auth_token: None,
            timeout_secs: None,
            refresh_before_secs: None,
        };
        assert_eq!(
            cfg.ensure_url(),
            "http://grok-keepalive:8080/v1/credentials/ensure"
        );
        assert_eq!(cfg.timeout_secs(), 10);
        assert_eq!(cfg.refresh_before_secs(), 300);
    }

    #[test]
    fn keepalive_requires_preflight_when_expiry_missing() {
        let cfg = KeepaliveConfig {
            service_url: "http://grok-keepalive:8080".to_string(),
            ensure_path: None,
            auth_token: None,
            timeout_secs: None,
            refresh_before_secs: Some(300),
        };
        assert!(cfg.should_ensure(Some(&SessionAuthConfig::cookie_defaults())));
    }

    #[test]
    fn session_not_near_expiry_skips_preflight() {
        let cfg = KeepaliveConfig {
            service_url: "http://grok-keepalive:8080".to_string(),
            ensure_path: None,
            auth_token: None,
            timeout_secs: None,
            refresh_before_secs: Some(60),
        };
        let session = SessionAuthConfig {
            transport: "cookie".to_string(),
            primary_cookie_name: Some("sso".to_string()),
            secondary_cookie_name: Some("sso-rw".to_string()),
            header_name: None,
            expires_at: Some("2099-01-01T00:00:00.000Z".to_string()),
        };
        assert!(!cfg.should_ensure(Some(&session)));
    }
}
