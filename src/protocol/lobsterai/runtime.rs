//! Fixed personal inference endpoints; resolved protocol and exact model never inherit caller routing headers.
use super::*;
use crate::protocol::{
    canonical::{CanonicalRelayRequest, CanonicalRelayResponse, EndpointKind},
    openai,
};
use crate::upstream::{
    client::UpstreamClient, common::RequestPlan, response_types::UpstreamStreamingResponse,
};
use crate::{error::GatewayError, routing::candidate::ProviderAccountPayload};
pub(crate) const VERSION: &str = concat!("neuro-gateway/", env!("CARGO_PKG_VERSION"));
pub(crate) fn plan(
    payload: &ProviderAccountPayload,
    req: &CanonicalRelayRequest,
    model: &str,
    stream: bool,
) -> Result<RequestPlan, GatewayError> {
    validation::validate(payload)?;
    let declaration = validation::model(payload, model, req)?;
    let model = declaration.model_id.as_str();
    if declaration.api_format.as_deref() == Some("anthropic")
        && !super::virtual_model::is_virtual(model)
    {
        return super::messages::plan(req, model, stream);
    }
    let mut body = openai::pack_openai(req, "", stream);
    body["model"] = model.into();
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
    Ok(RequestPlan {
        method: rquest::Method::POST,
        url: format!("{}/chat/completions", management_boundary::BASE),
        query: vec![],
        body: Some(body),
        response_kind: EndpointKind::ChatCompletions,
    })
}
async fn send(
    client: &UpstreamClient,
    payload: &ProviderAccountPayload,
    req: &CanonicalRelayRequest,
    model: &str,
    stream: bool,
) -> Result<(rquest::Response, EndpointKind), GatewayError> {
    let plan = plan(payload, req, model, stream)?;
    let metadata = metadata::read(payload)?;
    if virtual_model::is_virtual(model) && metadata.selection.is_none() {
        return Err(invalid());
    }
    let path = if plan.response_kind == EndpointKind::Messages {
        "/api/proxy/v1/messages"
    } else {
        "/api/proxy/v1/chat/completions"
    };
    let mut request = client
        .lobsterai_http
        .post(transport::target(path))
        .json(&plan.body)
        .default_headers(false)
        .redirect(rquest::redirect::Policy::none())
        .timeout(client.timeout)
        .bearer_auth(&payload.api_key)
        .header("content-type", "application/json")
        .header(
            "accept",
            if stream {
                "text/event-stream, application/json"
            } else {
                "application/json, text/event-stream"
            },
        )
        .header(
            "user-agent",
            concat!("NeuroGateway/", env!("CARGO_PKG_VERSION"), " lobsterai"),
        )
        .header("X-LobsterAI-Client-Version", VERSION);
    if metadata
        .selection
        .as_ref()
        .is_some_and(|s| virtual_model::is_virtual(&s.preset))
    {
        request = request.header("X-LobsterAI-Client-Capabilities", virtual_model::CAPABILITY);
    }
    if plan.response_kind == EndpointKind::Messages {
        request = request.header("anthropic-version", "2023-06-01");
    }
    let response = request
        .send()
        .await
        .map_err(|_| response_error().with_code("lobsterai_transport_failed"))?;
    admit_status(&response)?;
    Ok((response, plan.response_kind))
}
pub(super) fn admit_status(response: &rquest::Response) -> Result<(), GatewayError> {
    if !response.status().is_success() {
        let mut error = response_error().with_code("lobsterai_upstream_rejected");
        error.http_status = Some(if response.status().is_redirection() {
            502
        } else {
            response.status().as_u16()
        });
        return Err(error);
    }
    Ok(())
}
pub(crate) async fn execute(
    client: &UpstreamClient,
    payload: &ProviderAccountPayload,
    req: &CanonicalRelayRequest,
    model: &str,
) -> Result<CanonicalRelayResponse, GatewayError> {
    let (response, kind) = send(client, payload, req, model, false).await?;
    if kind == EndpointKind::Messages {
        messages::read(response, model).await
    } else {
        response::read(response).await
    }
}
pub(crate) async fn execute_probe(
    client: &UpstreamClient,
    payload: &ProviderAccountPayload,
    req: &CanonicalRelayRequest,
    model: &str,
) -> Result<CanonicalRelayResponse, GatewayError> {
    let (response, kind) = send(client, payload, req, model, false).await?;
    if kind == EndpointKind::Messages {
        messages::read_probe(response, model).await
    } else {
        response::read_probe(response).await
    }
}
pub(crate) async fn execute_stream(
    client: &UpstreamClient,
    payload: &ProviderAccountPayload,
    req: &CanonicalRelayRequest,
    model: &str,
) -> Result<UpstreamStreamingResponse, GatewayError> {
    let (response, kind) = send(client, payload, req, model, true).await?;
    if response::is_sse(&response)? {
        return Ok(UpstreamStreamingResponse::Http(response));
    }
    let canonical = if kind == EndpointKind::Messages {
        messages::read(response, model).await?
    } else {
        response::read(response).await?
    };
    let frames = crate::upstream::canonical_sse::canonical_response_to_openai_sse_bytes(
        req, model, &canonical,
    );
    Ok(UpstreamStreamingResponse::Bytes(Box::pin(
        futures::stream::iter(frames.into_iter().map(Ok)),
    )))
}
