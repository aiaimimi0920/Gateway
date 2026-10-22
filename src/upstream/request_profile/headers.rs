//! A selected profile owns software identity; credentials and transport remain adapter-owned.
use super::*;
use crate::protocol::claude_wire as claude;
use crate::upstream::common::insert_header_map_value as set;
use rquest::header::{HeaderMap, HeaderName};

// Software fingerprint fields are authoritative. Session/turn state is not a software fingerprint.
fn owned_header(name: &str) -> bool {
    let name = name.to_ascii_lowercase();
    name.starts_with("x-stainless-")
        || matches!(
            name.as_str(),
            "user-agent"
                | "originator"
                | "version"
                | "x-app"
                | "anthropic-dangerous-direct-browser-access"
                | "x-ide-type"
                | "x-ide-name"
                | "x-ide-version"
                | "x-product"
                | "x-codebuddy-request"
                | "x-requested-with"
        )
}

pub(crate) fn apply_headers(
    headers: &mut HeaderMap,
    payload: &ProviderAccountPayload,
    req: &CanonicalRelayRequest,
    model: &str,
) {
    if !applies(payload, req) {
        return;
    }
    let version = headers.get("version").cloned();
    let native_claude = headers
        .get("user-agent")
        .and_then(|v| v.to_str().ok())
        .zip(headers.get("x-app").and_then(|v| v.to_str().ok()))
        .is_some_and(|(ua, app)| claude::identity(ua, app));
    let claude_headers: Vec<_> = claude::HEADERS
        .iter()
        .filter_map(|name| {
            let value = headers.get(*name)?;
            claude::valid(value.to_str().ok()?).then(|| ((*name).to_owned(), value.clone()))
        })
        .collect();
    let native = headers
        .get("originator")
        .and_then(|v| v.to_str().ok())
        .zip(headers.get("user-agent").and_then(|v| v.to_str().ok()))
        .filter(|(origin, ua)| crate::protocol::codex_wire::native_identity(origin, ua))
        .map(|(origin, ua)| (origin.to_owned(), ua.to_owned()));
    // Custom authentication is a stronger authority than software identity, even if names overlap.
    let auth = match payload.canonical_adapter() {
        "custom_http"
        | "gemini_api_compatible"
        | "gemini_api_modular_compatible"
        | "cohere_compatible"
        | "bedrock_converse_compatible"
        | "dashscope_compatible"
        | "dashscope_multimodal_compatible" => payload.auth_header_name.as_deref(),
        _ => None,
    }
    .and_then(|name| {
        let name = HeaderName::from_bytes(name.as_bytes()).ok()?;
        let value = headers.get(&name)?.clone();
        Some((name, value))
    });
    let names: Vec<_> = headers
        .keys()
        .filter(|name| owned_header(name.as_str()))
        .cloned()
        .collect();
    for name in names {
        headers.remove(name);
    }
    // Explicit beta negotiation stays intact; selecting an identity never grants OAuth scopes.
    if selected(payload) == RequestClientProfile::Workbuddy {
        super::workbuddy_codex::apply(headers, payload, native.is_some());
        super::workbuddy::apply(headers);
    } else if uses_claude(payload, req, model) {
        for (name, value) in [
            ("user-agent", claude::USER_AGENT),
            ("x-app", "cli"),
            ("x-stainless-lang", "js"),
            ("x-stainless-runtime", "node"),
            ("x-stainless-package-version", "0.112.1"),
            ("x-stainless-runtime-version", "v26.3.0"),
            ("x-stainless-os", claude::platform()),
            ("x-stainless-arch", claude::architecture()),
            ("x-stainless-retry-count", "0"),
            ("x-stainless-timeout", "600"),
            ("anthropic-dangerous-direct-browser-access", "true"),
        ] {
            set(headers, name, value);
        }
        if !headers.contains_key("accept") {
            set(headers, "accept", "application/json");
        }
        if native_claude {
            for (name, value) in claude_headers {
                if let Ok(name) = HeaderName::from_bytes(name.as_bytes()) {
                    headers.insert(name, value);
                }
            }
        }
        if payload.canonical_adapter() == "anthropic_compatible" && !native_claude {
            set(
                headers,
                "anthropic-version",
                payload.anthropic_version.as_deref().unwrap_or("2023-06-01"),
            );
            merge_beta(headers, "claude-code-20250219");
            merge_beta(headers, "interleaved-thinking-2025-05-14");
            if !req.tools.is_empty() {
                merge_beta(headers, "fine-grained-tool-streaming-2025-05-14");
            }
        }
    } else {
        let cpa = selected(payload).is_cpa();
        if !cpa && !headers.contains_key("accept") {
            set(
                headers,
                "accept",
                if req.stream {
                    "text/event-stream"
                } else {
                    "application/json"
                },
            );
        }
        set(
            headers,
            "user-agent",
            if cpa {
                super::cpa::CODEX_UA
            } else {
                super::codex_identity::user_agent()
            },
        );
        set(
            headers,
            "originator",
            if cpa {
                "codex-tui"
            } else {
                super::codex_identity::ORIGINATOR
            },
        );
        if let Some((origin, ua)) = native.filter(|_| !cpa) {
            set(headers, "user-agent", &ua);
            set(headers, "originator", &origin);
        }
    }
    super::cpa::apply(headers, payload, req, model, version);
    if let Some((name, value)) = auth {
        headers.insert(name, value);
    }
}

fn merge_beta(headers: &mut HeaderMap, beta: &str) {
    let existing = headers
        .get("anthropic-beta")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");
    if existing.split(',').any(|v| v.trim() == beta) {
        return;
    }
    let value = if existing.is_empty() {
        beta.to_string()
    } else {
        format!("{existing},{beta}")
    };
    set(headers, "anthropic-beta", &value);
}
