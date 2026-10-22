//! Bounded native request evidence, separate from provider-controlled canonical data.
use bytes::Bytes;
use serde_json::Value;

/// Only protocol/session headers, never inbound credentials or Gateway controls.
pub const FORWARDED_HEADERS: &[&str] = &[
    "user-agent",
    "originator",
    "accept",
    "session-id",
    "thread-id",
    "x-client-request-id",
    "x-codex-beta-features",
    "x-codex-turn-metadata",
    "x-codex-window-id",
    "x-codex-turn-state",
    "openai-beta",
    // Native CLI context, not Gateway controls or authentication. Preserve only
    // caller-supplied values; provider header overrides remain authoritative.
    "x-openai-subagent",
    "x-openai-internal-codex-residency",
    "x-openai-internal-codex-responses-lite",
    "x-codex-parent-thread-id",
    "x-codex-routing-hint",
    "x-openai-memgen-request",
];

pub fn valid_header(name: &str, value: &str) -> bool {
    if name == "x-openai-internal-codex-responses-lite" {
        return value == "true";
    }
    let limit = if name == "x-codex-turn-metadata" {
        16_384
    } else {
        1024
    };
    !value.is_empty() && value.len() <= limit && value.bytes().all(|b| (32..=126).contains(&b))
}

/// An identity is retained only as a consistent Codex software pair. Arbitrary
/// caller device headers do not become authoritative by selecting a profile.
pub fn native_identity(origin: &str, ua: &str) -> bool {
    matches!(origin, "codex_exec" | "codex-tui" | "codex_cli_rs")
        && valid_header("user-agent", ua)
        && ua.strip_prefix(origin).is_some_and(|rest| {
            rest.strip_prefix('/')
                .is_some_and(|v| v.as_bytes().first().is_some_and(u8::is_ascii_digit))
        })
}

#[derive(Clone)]
pub struct NativeRequestWire {
    /// Only an authenticated opt-in route can set this runtime-local scope.
    pub(crate) native_session: Option<crate::http_client::native_cookie_sessions::Scope>,
    /// Set only by the compact route, never by JSON or caller headers.
    pub(crate) compact: bool,
    pub(crate) websocket: Option<crate::upstream::client::NativeResponsesSocket>,
    pub(crate) connection: Option<crate::http_client::NativeCodexConnection>,
    pub body: Bytes,
    pub query: Option<String>,
    pub client_request_id: bool,
    pub header_order: Vec<String>,
}

impl NativeRequestWire {
    /// Never bypass model mapping, defaults, policy, or real tool conversion.
    pub fn unchanged_body(&self, planned: &Value) -> Option<Bytes> {
        serde_json::from_slice::<Value>(&self.body)
            .ok()
            .filter(|original| original == planned)
            .map(|_| self.body.clone())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn evidence_is_not_a_policy_bypass() {
        let wire = NativeRequestWire {
            native_session: None,
            compact: false,
            websocket: None,
            connection: None,
            body: Bytes::from_static(b"{ \"model\": \"a\", \"tools\": [] }\n"),
            query: None,
            client_request_id: false,
            header_order: Vec::new(),
        };
        assert!(wire
            .unchanged_body(&serde_json::json!({"model":"a","tools":[]}))
            .is_some());
        assert!(wire
            .unchanged_body(&serde_json::json!({"model":"b","tools":[]}))
            .is_none());
        assert!(!native_identity("codex_exec", "private-device/1"));
        assert!(!valid_header("session-id", "bad\r\nheader"));
        assert!(!valid_header("session-id", &"x".repeat(1025)));
        assert!(FORWARDED_HEADERS.contains(&"x-openai-internal-codex-responses-lite"));
        assert!(valid_header(
            "x-openai-internal-codex-responses-lite",
            "true"
        ));
        assert!(!valid_header(
            "x-openai-internal-codex-responses-lite",
            "false"
        ));
    }

    #[test]
    fn optional_codex_metadata_is_bounded_and_does_not_allow_arbitrary_headers() {
        for name in [
            "x-openai-subagent",
            "x-openai-internal-codex-residency",
            "x-codex-parent-thread-id",
            "x-codex-routing-hint",
            "x-openai-memgen-request",
        ] {
            assert!(FORWARDED_HEADERS.contains(&name));
            assert!(valid_header(name, "review"));
            assert!(valid_header(name, "us"));
            assert!(!valid_header(name, ""));
            assert!(!valid_header(name, "value\r\nx-internal-api-key: secret"));
            assert!(!valid_header(name, &"x".repeat(1025)));
        }
        for name in [
            "authorization",
            "cookie",
            "x-internal-api-key",
            "x-neuro-account-group",
            "x-openai-arbitrary",
        ] {
            assert!(!FORWARDED_HEADERS.contains(&name));
        }
    }
}
