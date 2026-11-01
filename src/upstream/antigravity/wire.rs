//! Cloud Code envelope boundary; caller extras cannot override project, model or identity.
use super::failure;
use crate::{
    error::GatewayError,
    protocol::{
        canonical::{CanonicalRelayRequest, EndpointKind},
        gemini::api::build_text_request_body,
    },
    routing::candidate::ProviderAccountPayload,
};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};

pub(super) fn envelope(
    payload: &ProviderAccountPayload,
    req: &CanonicalRelayRequest,
    model: &str,
) -> Result<Value, GatewayError> {
    if !matches!(
        req.endpoint_kind,
        EndpointKind::ChatCompletions
            | EndpointKind::Completions
            | EndpointKind::Responses
            | EndpointKind::Messages
    ) || req.previous_response_id.is_some()
    {
        return Err(failure(
            "antigravity_input_unsupported",
            "Antigravity supports canonical chat, not media jobs or stored Responses IDs",
        ));
    }
    let project = payload
        .extra_body
        .as_ref()
        .and_then(|b| b.get("antigravity_project_id"))
        .and_then(Value::as_str)
        .filter(|s| !s.trim().is_empty() && s.len() <= 256)
        .ok_or_else(|| {
            failure(
                "antigravity_project_missing",
                "Import an activated Antigravity account with a real Cloud Code project",
            )
        })?;
    if model.is_empty() || model.len() > 200 || model.chars().any(char::is_control) {
        return Err(failure(
            "antigravity_model_invalid",
            "Antigravity requires a catalog model ID",
        ));
    }
    let mut request = build_text_request_body(req, model);
    let object = request.as_object_mut().ok_or_else(|| {
        failure(
            "antigravity_request_invalid",
            "Invalid canonical Gemini request",
        )
    })?;
    for key in [
        "model",
        "stream",
        "sessionId",
        "project",
        "requestId",
        "userAgent",
        "requestType",
        "antigravity_project_id",
        "antigravity_account_id",
    ] {
        object.remove(key);
    }
    // Do not override an explicit tool policy. VALIDATED is the native agent default only.
    if !req.tools.is_empty() && req.tool_choice.is_none() {
        object.insert(
            "toolConfig".into(),
            json!({"functionCallingConfig":{"mode":"VALIDATED"}}),
        );
    }
    // Stable for an explicit conversation, opaque and separated by account/project.
    let seed = req
        .explicit_session_key
        .clone()
        .unwrap_or_else(|| uuid::Uuid::new_v4().to_string());
    let account = payload
        .extra_body
        .as_ref()
        .and_then(|b| b.get("antigravity_account_id"))
        .and_then(Value::as_str)
        .unwrap_or(project);
    let digest = Sha256::digest(format!("{account}:{project}:{seed}").as_bytes());
    let session =
        u64::from_be_bytes(digest[..8].try_into().expect("eight hash bytes")) & i64::MAX as u64;
    object.insert("sessionId".into(), json!(format!("-{session}")));
    let request_id = format!(
        "agent/{}/{}/{}/1",
        uuid::Uuid::new_v4(),
        time::OffsetDateTime::now_utc().unix_timestamp_nanos() / 1_000_000,
        uuid::Uuid::new_v4()
    );
    let body = json!({"project":project, "model":model, "requestId":request_id,
        "requestType":"agent", "userAgent":"antigravity", "request":request});
    if serde_json::to_vec(&body).map_or(true, |b| b.len() > 16 * 1024 * 1024) {
        return Err(failure(
            "antigravity_request_limit",
            "Antigravity request exceeds 16 MiB",
        ));
    }
    Ok(body)
}
