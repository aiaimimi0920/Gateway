//! One explicitly configured deployment, one SAP sender and no inference replay.
use super::*;
use crate::protocol::{
    canonical::{CanonicalRelayRequest, CanonicalRelayResponse, EndpointKind},
    openai,
};
use crate::upstream::{
    client::UpstreamClient, common::RequestPlan, response_types::UpstreamStreamingResponse,
};
pub fn plan(
    payload: &ProviderAccountPayload,
    req: &CanonicalRelayRequest,
    model: &str,
    stream: bool,
) -> Result<RequestPlan, GatewayError> {
    validate(payload, payload.canonical_adapter())?;
    let endpoint = Endpoint::parse(&payload.base_url).ok_or_else(invalid)?;
    if model != endpoint.deployment {
        return Err(invalid());
    }
    if bedrock::selected(payload) {
        return bedrock::plan(payload, req, model, stream);
    }
    if let Some(orchestration_model) = orchestration::model(payload) {
        return orchestration::plan(payload, req, orchestration_model, stream);
    }
    if native_responses::selected(payload, req.endpoint_kind) {
        return native_responses::plan(payload, req, model, stream);
    }
    if !matches!(
        req.endpoint_kind,
        EndpointKind::ChatCompletions | EndpointKind::Messages | EndpointKind::Responses
    )
        || req.previous_response_id.is_some()
        || req
            .raw_body
            .get("conversation")
            .is_some_and(|v| !v.is_null())
        || req.raw_body.get("store").is_some_and(|v| v == true)
        || req.raw_body.get("background").is_some_and(|v| v == true)
        || req.reasoning.as_ref().is_some_and(|v| !v.is_null())
        // Canonical messages do not preserve Responses reasoning or hosted-tool history.
        || req.endpoint_kind == EndpointKind::Responses && req.raw_body.get("input")
            .and_then(serde_json::Value::as_array).is_some_and(|items| items.iter().any(|item|
                !matches!(item.get("type").and_then(serde_json::Value::as_str),
                    None | Some("message" | "function_call" | "function_call_output"))))
        || req.tools.iter().any(|t| t.tool_type != "function")
    {
        return Err(invalid());
    }
    let settings = payload.extra_body.as_ref().unwrap();
    image_input::validate(
        req,
        settings.get(config::IMAGE_INPUT).is_some_and(|v| v == true),
    )?;
    // The deployment selects the upstream model. Never send the gateway's deployment alias as model.
    let mut body = openai::pack_openai(req, "", stream);
    let fields = body.as_object_mut().ok_or_else(invalid)?;
    reasoning::adapt(
        req,
        fields,
        settings.get(config::O3_MINI).is_some_and(|v| v == true),
    )?;
    fields.remove("model");
    let version = settings["sap_api_version"].as_str().unwrap();
    // Suppress only our synthetic usage request for old versions; explicit caller intent is preserved.
    if &version[..10] < "2024-10-21" && req.raw_body.get("stream_options").is_none() {
        fields.remove("stream_options");
    }
    for local in [
        "store",
        "background",
        "previous_response_id",
        "conversation",
        "sap_api_version",
        config::IMAGE_INPUT,
        config::O3_MINI,
        config::NATIVE_RESPONSES,
        config::NATIVE_RESPONSES_IMAGE_INPUT,
        config::STORED_RESPONSES,
    ] {
        fields.remove(local);
    }
    Ok(RequestPlan {
        method: rquest::Method::POST,
        url: format!("{}/chat/completions", payload.base_url),
        query: vec![("api-version".into(), version.into())],
        body: Some(body),
        response_kind: EndpointKind::ChatCompletions,
    })
}
pub(super) async fn send(
    client: &UpstreamClient,
    payload: &ProviderAccountPayload,
    req: &CanonicalRelayRequest,
    model: &str,
    stream: bool,
) -> Result<(rquest::Response, String), GatewayError> {
    sender::Prepared::prepare(client, payload, req, model, stream)
        .await?
        .send()
        .await
}
pub async fn execute(
    client: &UpstreamClient,
    payload: &ProviderAccountPayload,
    req: &CanonicalRelayRequest,
    model: &str,
) -> Result<CanonicalRelayResponse, GatewayError> {
    let (response, _) = send(client, payload, req, model, false).await?;
    decode(response, payload, req, model).await
}

pub(crate) async fn decode(
    response: rquest::Response,
    payload: &ProviderAccountPayload,
    req: &CanonicalRelayRequest,
    model: &str,
) -> Result<CanonicalRelayResponse, GatewayError> {
    let bytes = crate::protocol::upstream_body::collect_bounded_upstream_body(response, "SAP Chat")
        .await
        .map_err(|_| failure("sap_aicore_body_incomplete"))?;
    let failed = || failure("sap_aicore_response_failed");
    if bedrock::selected(payload) {
        let body = crate::protocol::unique_json::parse(&bytes).map_err(|_| failed())?;
        return bedrock::unpack(&body, req, req.requested_model.as_deref().unwrap_or(model))
            .ok_or_else(failed);
    }
    if orchestration::model(payload).is_some() {
        let body = crate::protocol::unique_json::parse(&bytes).map_err(|_| failed())?;
        let result =
            orchestration::final_result(&body, !req.tools.is_empty()).ok_or_else(failed)?;
        openai::chat_admission::validate(result).map_err(|_| failed())?;
        return openai::unpack_native_chat_response(result).map_err(|_| failed());
    }
    let body = serde_json::from_slice(&bytes).map_err(|_| failed())?;
    openai::chat_admission::unpack(&body).map_err(|_| failed())
}
pub async fn execute_stream(
    client: &UpstreamClient,
    payload: &ProviderAccountPayload,
    req: &CanonicalRelayRequest,
    model: &str,
) -> Result<UpstreamStreamingResponse, GatewayError> {
    let (response, _) = send(client, payload, req, model, true).await?;
    decode_stream(response, payload, req, model)
}

pub(crate) fn decode_stream(
    response: rquest::Response,
    payload: &ProviderAccountPayload,
    req: &CanonicalRelayRequest,
    model: &str,
) -> Result<UpstreamStreamingResponse, GatewayError> {
    if bedrock::selected(payload) {
        return bedrock::stream(response, req, model);
    }
    if orchestration::model(payload).is_some() {
        let is_sse = response
            .headers()
            .get("content-type")
            .and_then(|value| value.to_str().ok())
            .and_then(|value| value.split(';').next())
            .is_some_and(|mime| mime.trim().eq_ignore_ascii_case("text/event-stream"));
        if !is_sse {
            return Err(failure("sap_aicore_response_failed"));
        }
        let reply_model = req
            .requested_model
            .as_deref()
            .map(str::trim)
            .filter(|name| !name.is_empty())
            .unwrap_or(model)
            .to_owned();
        return Ok(UpstreamStreamingResponse::NativeEvents(Box::pin(
            orchestration_stream::translate(
                response,
                reply_model,
                !req.tools.is_empty(),
                orchestration::reasoning::enabled(payload),
            ),
        )));
    }
    // Pipeline admission guards the Chat bytes before public-protocol conversion.
    Ok(UpstreamStreamingResponse::Http(response))
}
