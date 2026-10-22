//! Ordinary Responses opt-in admission only. Network initialization happens after the pipeline.
use super::*;
use crate::protocol::codex_wire::{native_identity, NativeRequestWire};

pub(crate) async fn attach_sdk_session(
    state: &Arc<AppState>,
    token: Option<&str>,
    headers: &HeaderMap,
    body: &serde_json::Value,
    wire: &mut NativeRequestWire,
) -> Result<(), GatewayError> {
    if std::env::var("GATEWAY_EXPERIMENTAL_CODEX_SESSION").as_deref() != Ok("1")
        || wire.native_session.is_some()
        || wire.compact
        || wire.websocket.is_some()
    {
        return Ok(());
    }
    let native = headers
        .get("originator")
        .and_then(|v| v.to_str().ok())
        .zip(headers.get("user-agent").and_then(|v| v.to_str().ok()))
        .is_some_and(|(origin, ua)| native_identity(origin, ua));
    if native {
        return Ok(());
    }
    // Missing identity stays stateless. A request ID or cache key is not a session.
    let Some(session) = headers.get("session-id").and_then(|v| v.to_str().ok()) else {
        return Ok(());
    };
    let model = body.get("model").and_then(|v| v.as_str());
    let candidates = state
        .route_config
        .snapshot()
        .resolve_candidates_with_constraint(model, None)
        .candidates;
    if !candidates
        .iter()
        .any(|candidate| crate::upstream::client::native_session::eligible(&candidate.payload))
    {
        return Ok(());
    }
    let model = model
        .ok_or_else(|| GatewayError::bad_request("Native session requires an explicit model"))?;
    // Reuse standalone owner authentication and the unambiguous official-candidate
    // restriction. This only assigns a scope; it never sends settings before filtering.
    let (scope, _) = admit(
        state,
        token,
        headers,
        session,
        model,
        "/v1/responses",
        "POST",
    )
    .await?;
    if body.get("stream").and_then(|v| v.as_bool()) != Some(true) {
        return Err(GatewayError::bad_request(
            "Native session requires streaming Responses",
        ));
    }
    wire.native_session = Some(scope);
    Ok(())
}
