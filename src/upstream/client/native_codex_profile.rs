//! Codex envelope for an explicitly selected profile and identifiable non-Codex SDK.
//! Preserve caller business/session fields; never manufacture CLI state or capabilities.
use super::*;

const RESPONSE_ORDER: &[&str] = &[
    "x-codex-beta-features",
    "x-codex-window-id",
    "x-codex-turn-metadata",
    "x-openai-internal-codex-responses-lite",
    "x-client-request-id",
    "session-id",
    "thread-id",
    "accept",
    "content-type",
    "authorization",
    "chatgpt-account-id",
    "originator",
    "user-agent",
    "cookie",
];

pub(super) fn sdk_envelope(
    headers: &mut HeaderMap,
    payload: &ProviderAccountPayload,
    req: &CanonicalRelayRequest,
    extra: Option<&HashMap<String, String>>,
    wire: &NativeRequestWire,
) -> Option<Vec<String>> {
    if payload.request_client_profile != Some(RequestClientProfile::CodexCli)
        || wire.compact
        || wire.websocket.is_some()
    {
        return None;
    }
    let extra = extra?;
    let ua = extra.get("user-agent")?;
    if extra
        .get("originator")
        .is_some_and(|origin| crate::protocol::codex_wire::native_identity(origin, ua))
    {
        return None;
    }
    // SDK Accept commonly describes JSON even for an SSE operation. The selected
    // profile owns this generated envelope, but an explicit operator override wins.
    if !payload
        .headers
        .keys()
        .any(|name| name.eq_ignore_ascii_case("accept"))
    {
        headers.insert(
            "accept",
            HeaderValue::from_static(if req.stream {
                "text/event-stream"
            } else {
                "application/json"
            }),
        );
    }
    // Only order fields that already exist. Do not copy source Cookie/authentication
    // or infer thread/window/turn IDs from an unrelated SDK request ID.
    let mut order: Vec<String> = RESPONSE_ORDER
        .iter()
        .map(|name| (*name).to_owned())
        .collect();
    for name in &wire.header_order {
        if !order.contains(name) {
            order.push(name.clone());
        }
    }
    Some(order)
}
