//! Token Plan Messages owns its endpoint, but uses the shared lossless Anthropic wire guards.
use super::*;
use crate::protocol::anthropic;
use serde_json::Value;

pub(crate) fn selected(payload: &ProviderAccountPayload, endpoint: EndpointKind) -> bool {
    owns_payload(payload) && endpoint == EndpointKind::Messages
}

pub(crate) fn plan(
    payload: &ProviderAccountPayload,
    req: &CanonicalRelayRequest,
    model: &str,
    stream: bool,
) -> Result<RequestPlan, GatewayError> {
    validate_request(payload, req)?;
    let body = crate::protocol::native_text_request::pack(req, model, stream, invalid)?;
    // Anthropic normalization labels all definitions as functions; check the final raw wire too.
    if body
        .get("mcp_servers")
        .is_some_and(|value| !value.is_null())
        || body.get("tools").is_some_and(|tools| {
            !tools.is_null()
                && tools.as_array().is_none_or(|tools| {
                    tools.iter().any(|tool| {
                        !tool.get("type").is_none_or(|kind| kind == "custom")
                            || !tool["name"]
                                .as_str()
                                .is_some_and(|name| !name.trim().is_empty())
                            || !tool["input_schema"].is_object()
                    })
                })
        })
    {
        return Err(invalid());
    }
    validate_image_models(&body, model)?;
    let plan = RequestPlan {
        method: rquest::Method::POST,
        url: format!("{BASE}/messages"),
        query: vec![],
        body: Some(body),
        response_kind: EndpointKind::Messages,
    };
    validate_plan(payload, &plan)?;
    Ok(plan)
}

fn validate_image_models(body: &Value, model: &str) -> Result<(), GatewayError> {
    fn parts(value: &Value) -> impl Iterator<Item = &Value> {
        value.as_array().into_iter().flatten()
    }
    let images = parts(&body["messages"])
        .flat_map(|message| parts(&message["content"]))
        .flat_map(|block| {
            std::iter::once(block).chain(
                (block["type"] == "tool_result")
                    .then(|| parts(&block["content"]))
                    .into_iter()
                    .flatten(),
            )
        });
    for block in images {
        match block["type"].as_str() {
            Some("image") => {
                if !matches!(
                    model,
                    "sensenova-6.8-flash-lite" | "sensenova-6.8-flash" | "deepseek-flash"
                ) {
                    return Err(GatewayError::bad_request(
                        "This SenseNova model is not documented to accept Messages images.",
                    )
                    .with_code("sensenova_messages_image_model_unsupported"));
                }
                // The fixed upstream owns image retrieval/decoding; Gateway sends no extra URL requests.
                if !matches!(block["source"]["type"].as_str(), Some("base64" | "url")) {
                    return Err(invalid());
                }
            }
            Some("document" | "audio" | "video") => return Err(invalid()),
            _ => {}
        }
    }
    Ok(())
}

pub(crate) async fn read(response: rquest::Response) -> Result<Value, GatewayError> {
    let bytes = crate::protocol::upstream_body::collect_bounded_upstream_body(
        response,
        "SenseNova Messages",
    )
    .await
    .map_err(|_| response_error())?;
    let body = crate::protocol::unique_json::parse(&bytes).map_err(|_| response_error())?;
    anthropic::native_admission::validate(&body).map_err(|_| response_error())?;
    Ok(body)
}

pub(crate) fn stream_error() -> crate::protocol::stream_error::ProtocolStreamError {
    crate::protocol::stream_error::ProtocolStreamError::classified(
        "invalid_sensenova_stream: incomplete or invalid Messages stream",
        response_error(),
    )
}
