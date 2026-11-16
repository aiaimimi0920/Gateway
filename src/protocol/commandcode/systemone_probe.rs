//! 控制台单次决策探测；JSON 仅作为管理预览，不冒充公开 Chat 回复。
use super::*;
use crate::protocol::canonical::{CanonicalRelayRequest, CanonicalRelayResponse};
use serde_json::{json, Value};

pub(super) fn request(model: &str, prompt: &str) -> Result<CanonicalRelayRequest, GatewayError> {
    systemone::normalize(json!({
        "model":model,"state":prompt,
        "questions":{"probe":{"type":"noul","instructions":"Does the supplied state contain text?"}}
    }))
}

pub(super) fn unpack(
    body: &Value,
    request: &Value,
) -> Result<CanonicalRelayResponse, GatewayError> {
    systemone::validate_response(body, request)?;
    Ok(CanonicalRelayResponse {
        bedrock_turn: None,
        vertex_turn: None,
        model_declaration: crate::protocol::model_declaration::ModelDeclaration::from_body(body),
        model: body["model"].as_str().expect("validated model").into(),
        text: body["answers"].to_string(),
        sources: vec![],
        usage: systemone::observe(body),
        tool_calls: vec![],
        upstream_status: Some(200),
        finish_reason: Some("stop".into()),
    })
}
