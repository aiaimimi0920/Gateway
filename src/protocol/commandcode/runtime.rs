//! 固定 Provider 目标；只按目录声明选原生协议，不重放已发送的推理。
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
    if req.endpoint_kind == EndpointKind::SystemOne {
        return super::systemone::plan(payload, req, model, stream);
    }
    if req.endpoint_kind == EndpointKind::Messages && super::messages::selected(payload) {
        return super::messages::plan(payload, req, model, stream);
    }
    if req.endpoint_kind == EndpointKind::Responses && payload.responses_path.is_some() {
        return super::responses::plan(payload, req, model, stream);
    }
    super::request::validate(req)?;
    // Do not infer reasoning-model parameters from a name. Preserve every system message.
    let mut body = openai::pack_openai(req, "", stream);
    body["model"] = serde_json::Value::String(model.into());
    body["messages"] = req
        .messages
        .iter()
        .map(openai::pack_openai_message)
        .collect();
    for local in [
        "store",
        "background",
        "previous_response_id",
        "conversation",
    ] {
        body.as_object_mut().unwrap().remove(local);
    }
    let plan = RequestPlan {
        method: rquest::Method::POST,
        url: format!("{BASE}/chat/completions"),
        query: vec![],
        body: Some(body),
        response_kind: EndpointKind::ChatCompletions,
    };
    validate_plan(payload, &plan)?;
    Ok(plan)
}
pub(super) async fn send(
    client: &UpstreamClient,
    payload: &ProviderAccountPayload,
    req: &CanonicalRelayRequest,
    model: &str,
    stream: bool,
) -> Result<rquest::Response, GatewayError> {
    let plan = plan(payload, req, model, stream)?;
    let mut headers = rquest::header::HeaderMap::new();
    finalize_headers(payload, &plan, &mut headers)?;
    #[cfg(test)]
    let url = super::test_support::transport_plan(payload, &plan)
        .unwrap()
        .url;
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
        .headers(headers)
        .send()
        .await
        .map_err(|_| response_error().with_code("commandcode_transport_failed"))?;
    if !response.status().is_success() {
        return Err(zdr::upstream_error(response).await);
    }
    Ok(response)
}
pub async fn execute(
    client: &UpstreamClient,
    payload: &ProviderAccountPayload,
    req: &CanonicalRelayRequest,
    model: &str,
) -> Result<CanonicalRelayResponse, GatewayError> {
    let response = send(client, payload, req, model, false).await?;
    if req.endpoint_kind == EndpointKind::Messages && super::messages::selected(payload) {
        return super::messages::read(response).await;
    }
    if req.endpoint_kind == EndpointKind::Responses && payload.responses_path.is_some() {
        return super::responses::read(response).await;
    }
    read_response(response).await
}
pub async fn execute_stream(
    client: &UpstreamClient,
    payload: &ProviderAccountPayload,
    req: &CanonicalRelayRequest,
    model: &str,
) -> Result<UpstreamStreamingResponse, GatewayError> {
    // Pipeline admission guards the raw Chat bytes before any public-protocol conversion.
    Ok(UpstreamStreamingResponse::Http(
        send(client, payload, req, model, true).await?,
    ))
}
