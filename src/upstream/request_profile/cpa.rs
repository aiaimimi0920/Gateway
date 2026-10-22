//! Pinned CPA CLI identity (20c9095); endpoint, credentials and session ownership stay separate.
use super::*;
use crate::upstream::common::insert_header_map_value as set;
use rquest::header::{HeaderMap, HeaderValue};

pub(super) const CODEX_UA: &str =
    "codex-tui/0.154.0 (Mac OS 26.5.2; arm64) iTerm.app/3.6.11 (codex-tui; 0.154.0)";
const CLAUDE_UA: &str = "claude-cli/2.1.280 (external, cli)";

// Like the other profiles, selecting CPA owns software identity even when a
// preset supplies stale identity headers. Actual custom authentication still wins.
fn software(headers: &mut HeaderMap, payload: &ProviderAccountPayload, name: &str, value: &str) {
    if !payload
        .auth_header_name
        .as_deref()
        .is_some_and(|key| key.eq_ignore_ascii_case(name))
    {
        set(headers, name, value);
    }
}

// Only the software fields we own may be restored here; never replay arbitrary headers.
fn put(headers: &mut HeaderMap, payload: &ProviderAccountPayload, name: &str, default: &str) {
    if payload
        .auth_header_name
        .as_deref()
        .is_some_and(|key| key.eq_ignore_ascii_case(name))
    {
        return;
    }
    let value = payload
        .headers
        .iter()
        .find(|(key, _)| key.eq_ignore_ascii_case(name))
        .map(|(_, value)| value.as_str())
        .unwrap_or(default);
    set(headers, name, value);
}

pub(super) fn apply(
    headers: &mut HeaderMap,
    payload: &ProviderAccountPayload,
    req: &CanonicalRelayRequest,
    model: &str,
    version: Option<HeaderValue>,
) {
    if !selected(payload).is_cpa() {
        return;
    }
    // CPA does not emit the relay's local request/audit identifier by default.
    if !payload
        .headers
        .keys()
        .any(|key| key.eq_ignore_ascii_case("x-request-id"))
        && !payload
            .auth_header_name
            .as_deref()
            .is_some_and(|key| key.eq_ignore_ascii_case("x-request-id"))
    {
        headers.remove("x-request-id");
    }
    if uses_claude(payload, req, model) {
        // Codex continuation state has no meaning on the CPA Claude branch. Do not
        // leak installation/window metadata or relabel it as a Claude account/session.
        let remove: Vec<_> = headers
            .keys()
            .filter(|name| {
                let key = name.as_str();
                (key.starts_with("x-codex-")
                    || key.starts_with("x-openai-")
                    || matches!(key, "session-id" | "thread-id" | "x-client-request-id"))
                    && !payload.headers.keys().any(|k| k.eq_ignore_ascii_case(key))
                    && !payload
                        .auth_header_name
                        .as_deref()
                        .is_some_and(|k| k.eq_ignore_ascii_case(key))
            })
            .cloned()
            .collect();
        for name in remove {
            headers.remove(name);
        }
        for (name, value) in [
            ("user-agent", CLAUDE_UA),
            ("x-app", "cli"),
            ("x-stainless-os", "MacOS"),
            ("x-stainless-arch", "arm64"),
            ("x-stainless-package-version", "0.112.1"),
            ("x-stainless-runtime-version", "v26.3.0"),
            ("x-stainless-runtime", "node"),
            ("x-stainless-lang", "js"),
            ("x-stainless-retry-count", "0"),
            ("x-stainless-timeout", "600"),
            ("anthropic-dangerous-direct-browser-access", "true"),
        ] {
            software(headers, payload, name, value);
        }
    } else {
        software(headers, payload, "user-agent", CODEX_UA);
        software(headers, payload, "originator", "codex-tui");
        // CPA forwards an actual Version but has no invented default version.
        if let Some(version) = version {
            headers.insert("version", version);
        }
    }
    transport(headers, payload, req, model, req.stream);
}

pub(crate) fn transport(
    headers: &mut HeaderMap,
    payload: &ProviderAccountPayload,
    req: &CanonicalRelayRequest,
    model: &str,
    stream: bool,
) {
    if !selected(payload).is_cpa() || !applies(payload, req) {
        return;
    }
    let claude = uses_claude(payload, req, model);
    let official = url::Url::parse(&payload.base_url)
        .ok()
        .is_some_and(|url| url.scheme() == "https" && url.host_str() == Some("api.anthropic.com"));
    let events = stream && (!claude || !official);
    put(
        headers,
        payload,
        "accept",
        if events {
            "text/event-stream"
        } else {
            "application/json"
        },
    );
    put(
        headers,
        payload,
        "connection",
        if claude { "keep-alive" } else { "Keep-Alive" },
    );
    put(
        headers,
        payload,
        "accept-encoding",
        if !claude {
            "gzip"
        } else if events {
            "identity"
        } else {
            "gzip, deflate, br, zstd"
        },
    );
}

pub(crate) fn identity(headers: &HeaderMap) -> bool {
    headers
        .get("user-agent")
        .and_then(|v| v.to_str().ok())
        .is_some_and(|ua| ua == CODEX_UA || ua == CLAUDE_UA)
}
