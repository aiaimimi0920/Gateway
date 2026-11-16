//! Manual music tests share Suno's ordinary generator and decoder, never a text/default plan.
#[cfg(feature = "line-suno-web-reverse-api")]
use super::media_probe_request::MediaProbeOperation;
use super::{media_probe_request::MediaProbeRequest, ConsoleProbeRequest};
#[cfg(feature = "line-suno-web-reverse-api")]
use crate::protocol::suno;
use crate::{
    error::GatewayError, protocol::canonical::CanonicalRelayRequest,
    routing::candidate::ProviderAccountPayload,
};
use serde_json::{json, Value};

#[cfg(feature = "line-suno-web-reverse-api")]
pub(super) fn canonical(
    media: &MediaProbeRequest,
    request: &ConsoleProbeRequest,
    model: &str,
    payload: &ProviderAccountPayload,
) -> Result<CanonicalRelayRequest, GatewayError> {
    // No caller-supplied wait/count/continuation/challenge/download operation can turn this
    // one confirmed basic-generation test into another task or an unverified pending success.
    if !matches!(media.operation, MediaProbeOperation::Music)
        || !media.parameters.is_empty()
        || !payload.call_flow.is_some_and(|flow| {
            flow.special_adapter == crate::protocol::call_flow::SpecialAdapter::SunoWeb
        })
    {
        return Err(invalid());
    }
    let prepared = crate::protocol::call_flow::prepare(payload)?;
    if prepared.executor != "suno_compatible" {
        return Err(invalid());
    }
    suno::resolve_model(model)?;
    let canonical = suno::normalize_music_generations(json!({
        "model": request.model, "prompt": request.prompt, "waitCompletion": true
    }))?;
    Ok(canonical)
}

#[cfg(not(feature = "line-suno-web-reverse-api"))]
pub(super) fn canonical(
    _media: &MediaProbeRequest,
    _request: &ConsoleProbeRequest,
    _model: &str,
    _payload: &ProviderAccountPayload,
) -> Result<CanonicalRelayRequest, GatewayError> {
    Err(invalid())
}

pub(super) fn completed(body: &Value) -> Result<(), GatewayError> {
    if body["completed"] == true && body["data"].as_array().is_some_and(|data| !data.is_empty()) {
        return Ok(());
    }
    // Violating this invariant must be an error, not a successful health recovery. The
    // ordinary sender verifies all playback bytes before emitting this complete response.
    Err(
        GatewayError::server_error("Suno music test did not produce completed, verified audio.")
            .with_provider("suno_compatible")
            .with_code("suno_probe_incomplete"),
    )
}
pub(super) fn message(body: &Value, model: &str) -> String {
    format!(
        "Suno 音乐测试通过；model={model}；wireModel={}；已验证 {} 首 MP3；供应商扣额未知。",
        verified_wire_model(model),
        body["data"]
            .as_array()
            .expect("completed probe clips")
            .len()
    )
}
pub(super) fn audit(body: &Value, model: &str) -> Value {
    // Closed projection only: never persist clips, signed URLs, prompts, lyrics, metadata,
    // Clerk material or estimated prices in model-test audits or text results.
    json!({"verified":true,"count":body["data"].as_array().expect("completed probe clips").len(),
        "wireModel":verified_wire_model(model),"supplierCharge":null})
}
fn verified_wire_model(model: &str) -> &str {
    #[cfg(feature = "line-suno-web-reverse-api")]
    {
        suno::resolve_model(model).expect("validated probe model")
    }
    #[cfg(not(feature = "line-suno-web-reverse-api"))]
    {
        let _ = model;
        unreachable!("disabled Suno music probe cannot succeed")
    }
}
fn invalid() -> GatewayError {
    GatewayError::bad_request(
        "Suno music tests require the typed executor, music operation and no parameter overrides.",
    )
    .with_provider("suno_compatible")
    .with_code("suno_probe_input_invalid")
}
