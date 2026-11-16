//! System One 的独立决策请求，复用产品 sender 而不经聊天协议转换。
use super::*;
use crate::protocol::canonical::{
    CanonicalMessage, CanonicalRelayRequest, ContentPart, MessageRole, ProtocolFamily,
};
use serde_json::Value;

pub(crate) fn normalize(body: Value) -> Result<CanonicalRelayRequest, GatewayError> {
    systemone_request::validate(&body)?;
    Ok(CanonicalRelayRequest {
        protocol_family: ProtocolFamily::SystemOne,
        endpoint_kind: EndpointKind::SystemOne,
        requested_model: body["model"].as_str().map(str::to_owned),
        stream: false,
        messages: vec![CanonicalMessage {
            role: MessageRole::User,
            content: vec![ContentPart::Text {
                text: systemone_request::projection(&body),
            }],
            name: None,
            tool_call_id: None,
            tool_calls: vec![],
        }],
        tools: vec![],
        tool_choice: None,
        reasoning: None,
        metadata: None,
        previous_response_id: None,
        explicit_session_key: None,
        raw_body: body,
        extra: Default::default(),
    })
}

pub(super) fn plan(
    payload: &ProviderAccountPayload,
    req: &CanonicalRelayRequest,
    model: &str,
    stream: bool,
) -> Result<RequestPlan, GatewayError> {
    systemone_request::validate(&req.raw_body)?;
    // 合成文本仅供内容过滤；不可逆的聊天策略修改必须显式拒绝，不能发旧 raw 绕过策略。
    if stream
        || req.stream
        || req.protocol_family != ProtocolFamily::SystemOne
        || req.endpoint_kind != EndpointKind::SystemOne
        || !projection_matches(req)
        || !req.tools.is_empty()
        || req.tool_choice.is_some()
        || req.reasoning.is_some()
        || req.previous_response_id.is_some()
        || !req.extra.is_empty()
    {
        return Err(GatewayError::bad_request(
            "Chat or streaming transformations cannot be applied to System One.",
        )
        .with_code("commandcode_systemone_policy_conflict"));
    }
    let mut body = req.raw_body.clone();
    body["model"] = model.into();
    body.as_object_mut()
        .expect("validated object")
        .remove("stream");
    let plan = RequestPlan {
        method: rquest::Method::POST,
        url: format!("{BASE}/systemone"),
        query: vec![],
        body: Some(body),
        response_kind: EndpointKind::SystemOne,
    };
    validate_plan(payload, &plan)?;
    Ok(plan)
}

fn projection_matches(req: &CanonicalRelayRequest) -> bool {
    let [message] = req.messages.as_slice() else {
        return false;
    };
    matches!(message.content.as_slice(), [ContentPart::Text { text }]
        if *text == systemone_request::projection(&req.raw_body))
        && message.role == MessageRole::User
        && message.name.is_none()
        && message.tool_call_id.is_none()
        && message.tool_calls.is_empty()
}

pub(super) fn validate_response(body: &Value, request: &Value) -> Result<(), GatewayError> {
    let questions = systemone_request::validate(request)?;
    systemone_response::validate(body, questions)
}

pub(crate) fn observe(body: &Value) -> Option<crate::protocol::canonical::TokenUsage> {
    systemone_response::usage(body).ok()
}
