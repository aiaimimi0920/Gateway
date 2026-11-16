//! One explicitly configured deployment, native Chat wire and no inference replay.
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
    if !matches!(
        req.endpoint_kind,
        EndpointKind::ChatCompletions | EndpointKind::Messages | EndpointKind::Responses
    ) || model != endpoint.deployment
        || req.previous_response_id.is_some()
        || req
            .raw_body
            .get("conversation")
            .is_some_and(|v| !v.is_null())
        || req.raw_body.get("store").is_some_and(|v| v == true)
        || req.raw_body.get("background").is_some_and(|v| v == true)
        || req.reasoning.as_ref().is_some_and(|v| !v.is_null())
        || req.raw_body.get("reasoning_effort").is_some_and(|v| !v.is_null())
        // Canonical messages do not preserve Responses reasoning or hosted-tool history.
        || req.endpoint_kind == EndpointKind::Responses && req.raw_body.get("input")
            .and_then(serde_json::Value::as_array).is_some_and(|items| items.iter().any(|item|
                !matches!(item.get("type").and_then(serde_json::Value::as_str),
                    None | Some("message" | "function_call" | "function_call_output"))))
        || req.tools.iter().any(|t| t.tool_type != "function")
    {
        return Err(invalid());
    }
    if req
        .messages
        .iter()
        .flat_map(|m| &m.content)
        .any(|p| p.as_text().is_none())
    {
        return Err(invalid());
    }
    // The deployment selects the upstream model. Never send the gateway's deployment alias as model.
    let mut body = openai::pack_openai(req, "", stream);
    body.as_object_mut().ok_or_else(invalid)?.remove("model");
    let version = payload.extra_body.as_ref().unwrap()["sap_api_version"]
        .as_str()
        .unwrap();
    // Suppress only our synthetic usage request for old versions; explicit caller intent is preserved.
    if &version[..10] < "2024-10-21" && req.raw_body.get("stream_options").is_none() {
        body.as_object_mut().unwrap().remove("stream_options");
    }
    for local in [
        "store",
        "background",
        "previous_response_id",
        "conversation",
    ] {
        body.as_object_mut().unwrap().remove(local);
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
    let plan = plan(payload, req, model, stream)?;
    let token = client.sap_aicore.get(client.client(), payload).await?;
    #[cfg(test)]
    let url = super::test_support::url(&plan.url);
    #[cfg(not(test))]
    let url = plan.url;
    let response = client
        .client()
        .post(url)
        .query(&plan.query)
        .json(&plan.body)
        .default_headers(false)
        .redirect(rquest::redirect::Policy::none())
        .timeout(client.timeout)
        .bearer_auth(&token)
        .header("AI-Resource-Group", &payload.headers["AI-Resource-Group"])
        .header(
            "accept",
            if stream {
                "text/event-stream"
            } else {
                "application/json"
            },
        )
        .send()
        .await
        .map_err(|_| failure("sap_aicore_transport_failed"))?;
    if !response.status().is_success() {
        return Err(failure("sap_aicore_upstream_rejected"));
    }
    Ok((response, token))
}
pub async fn execute(
    client: &UpstreamClient,
    payload: &ProviderAccountPayload,
    req: &CanonicalRelayRequest,
    model: &str,
) -> Result<CanonicalRelayResponse, GatewayError> {
    let (response, _) = send(client, payload, req, model, false).await?;
    let bytes = crate::protocol::upstream_body::collect_bounded_upstream_body(response, "SAP Chat")
        .await
        .map_err(|_| failure("sap_aicore_response_failed"))?;
    let body = serde_json::from_slice(&bytes).map_err(|_| failure("sap_aicore_response_failed"))?;
    openai::chat_admission::unpack(&body).map_err(|_| failure("sap_aicore_response_failed"))
}
pub async fn execute_stream(
    client: &UpstreamClient,
    payload: &ProviderAccountPayload,
    req: &CanonicalRelayRequest,
    model: &str,
) -> Result<UpstreamStreamingResponse, GatewayError> {
    // Pipeline admission guards the raw Chat bytes before any public-protocol conversion.
    Ok(UpstreamStreamingResponse::Http(
        send(client, payload, req, model, true).await?.0,
    ))
}
