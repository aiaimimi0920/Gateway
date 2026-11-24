//! Wire admission for stored SAP Responses. The caller must resolve and authorize an
//! owned record before constructing an operation; stored transport never chooses a route.
#![allow(dead_code)] // Public lifecycle dispatch waits for durable ownership and settlement.

use super::{config, failure, invalid, native_responses, Endpoint};
use crate::{
    error::GatewayError,
    protocol::{
        canonical::{CanonicalRelayRequest, TokenUsage},
        responses::native_admission,
    },
    routing::candidate::ProviderAccountPayload,
    upstream::common::RequestPlan,
};
use rquest::Method;
pub(crate) mod evidence;
pub(crate) mod store;
mod transport;
use serde_json::Value;

#[derive(Debug, Clone, Eq, PartialEq)]
pub(super) struct UpstreamId(String);

impl UpstreamId {
    pub(super) fn parse(value: &str) -> Option<Self> {
        (!value.is_empty()
            && value.len() <= 256
            && !value.starts_with("resp_gw_")
            && value
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_')))
        .then(|| Self(value.to_owned()))
    }
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub(crate) enum Operation {
    Retrieve,
    Cancel,
    Delete,
}

fn deployment(payload: &ProviderAccountPayload) -> Result<Endpoint, GatewayError> {
    if !enabled(payload) {
        return Err(invalid());
    }
    config::validate(payload, payload.canonical_adapter())?;
    Endpoint::parse(&payload.base_url).ok_or_else(invalid)
}

pub(crate) fn enabled(payload: &ProviderAccountPayload) -> bool {
    native_responses::selected(payload, crate::protocol::canonical::EndpointKind::Responses)
        && payload
            .extra_body
            .as_ref()
            .and_then(|v| v.get(config::STORED_RESPONSES))
            == Some(&Value::Bool(true))
}
pub(super) fn operation_plan(
    payload: &ProviderAccountPayload,
    id: &UpstreamId,
    op: Operation,
) -> Result<RequestPlan, GatewayError> {
    deployment(payload)?;
    let (method, tail) = match op {
        Operation::Retrieve => (Method::GET, ""),
        Operation::Cancel => (Method::POST, "/cancel"),
        Operation::Delete => (Method::DELETE, ""),
    };
    Ok(native_responses::responses_plan(
        payload,
        method,
        &format!("/{}{tail}", id.0),
        None,
    ))
}

/// The authorization owner supplies the upstream predecessor, never the HTTP caller.
/// `previous` contains both the exact local reference and its resolved provider identity.
pub(super) fn create_plan(
    payload: &ProviderAccountPayload,
    req: &CanonicalRelayRequest,
    model: &str,
    previous: Option<(&str, &UpstreamId)>,
) -> Result<RequestPlan, GatewayError> {
    if deployment(payload)?.deployment != model
        || !native_responses::selected(payload, req.endpoint_kind)
        || req.stream
        || !stored_intent(&req.raw_body)
    {
        return Err(invalid());
    }
    let stream = req.stream;
    let mut body =
        crate::protocol::native_text_request::pack_stored_responses(req, model, stream, invalid)?;
    let images = payload
        .extra_body
        .as_ref()
        .and_then(|settings| settings.get(config::NATIVE_RESPONSES_IMAGE_INPUT))
        == Some(&Value::Bool(true));
    if !stored_intent(&body)
        || !native_responses::supported(&body, images)
        || body.get("background") != req.raw_body.get("background")
    {
        return Err(invalid());
    }
    let previous_public = previous.map(|(public, _)| public);
    let matches_previous = |v: &Value| match previous_public {
        Some(id) => v.as_str() == Some(id),
        None => v.is_null(),
    };
    if !matches_previous(&req.raw_body["previous_response_id"])
        || !matches_previous(&body["previous_response_id"])
        || req.previous_response_id.as_deref() != previous_public
    {
        return Err(invalid());
    }
    let fields = body.as_object_mut().ok_or_else(invalid)?;
    for key in ["model", "conversation", "previous_response_id"] {
        fields.remove(key);
    }
    if let Some((_, upstream)) = previous {
        fields.insert("previous_response_id".into(), upstream.0.clone().into());
    }
    Ok(native_responses::responses_plan(
        payload,
        Method::POST,
        "",
        Some(body),
    ))
}

fn stored_intent(body: &Value) -> bool {
    body["store"] == true
        && body["conversation"].is_null()
        && body.get("background").is_none_or(Value::is_boolean)
}

#[derive(Clone)]
pub(super) enum State {
    Pending,
    Completed(Option<TokenUsage>),
    Failed(Option<TokenUsage>),
}

/// A pending receipt may identify the job, but can never settle its usage.
pub(super) fn observe(
    body: &Value,
    expected: Option<&UpstreamId>,
) -> Result<(UpstreamId, State), GatewayError> {
    let error = || failure("sap_aicore_response_failed");
    let id = body["id"]
        .as_str()
        .and_then(UpstreamId::parse)
        .ok_or_else(error)?;
    if expected.is_some_and(|expected| *expected != id)
        || body["object"] != "response"
        || body["store"] != true
        || !body["conversation"].is_null()
        || !body["output"].is_array()
    {
        return Err(error());
    }
    let (usage, _) = native_admission::observe(body);
    if !body["usage"].is_null() && usage.is_none() {
        return Err(error());
    }
    let state = match body["status"].as_str() {
        Some("queued" | "in_progress") if body["error"].is_null() && usage.is_none() => {
            State::Pending
        }
        Some("completed" | "incomplete") if native_admission::valid_terminal(body) => {
            State::Completed(usage)
        }
        Some("failed" | "cancelled") => State::Failed(usage),
        _ => return Err(error()),
    };
    Ok((id, state))
}

pub(super) fn validate_deleted(body: &Value, expected: &UpstreamId) -> Result<(), GatewayError> {
    if body["object"] == "response.deleted" && body["id"] == expected.0 && body["deleted"] == true {
        Ok(())
    } else {
        Err(failure("sap_aicore_response_failed"))
    }
}

#[cfg(all(test, feature = "line-sap-aicore-chat"))]
mod tests;
