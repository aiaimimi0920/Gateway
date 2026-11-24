//! Directory and protocol probes use the same software identity as normal inference.
use super::{registry, DiscoveredProtocol as P};
use crate::routing::candidate::ProviderAccountPayload;
use rquest::header::HeaderMap;
use serde_json::json;

pub(super) fn headers(protocol: P, key: &str) -> HeaderMap {
    let (adapter, profile) = match protocol {
        P::ChatCompletions | P::Responses | P::Completions => ("openai_compatible", "codex_cli"),
        P::Messages => ("anthropic_compatible", "claude_cli"),
        _ => return HeaderMap::new(),
    };
    let payload: ProviderAccountPayload = serde_json::from_value(json!({
        "adapter": adapter, "base_url": "", "api_key": key, "request_client_profile": profile,
    }))
    .expect("static discovery identity must remain a valid provider payload");
    // The explicit profile follows the wire protocol, never a model-name heuristic.
    let model = "discovery";
    let (_, body) = registry::request(protocol, model);
    let request = match protocol {
        P::ChatCompletions => crate::protocol::openai::normalize_chat_completions(body),
        P::Responses => crate::protocol::responses::normalize_responses(body),
        P::Completions => crate::protocol::openai::normalize_legacy_completions(body),
        P::Messages => crate::protocol::anthropic::normalize_messages(body),
        _ => unreachable!(),
    }
    .expect("static discovery request must remain a valid protocol request");
    let mut headers =
        crate::upstream::headers::build_inference_headers(&payload, &request, model, None);
    if profile == "codex_cli" {
        // Discovery owns this short-lived request context. Never borrow a user's
        // installation/session ID or add probe metadata to normal inference.
        headers.insert(
            "x-codex-window-id",
            uuid::Uuid::new_v4().to_string().parse().unwrap(),
        );
    }
    headers
}
