//! Native Responses uses declared per-model routes and product-owned failure admission.
use super::*;
use crate::protocol::canonical::{CanonicalRelayRequest, CanonicalRelayResponse};

pub(super) fn plan(
    payload: &ProviderAccountPayload,
    req: &CanonicalRelayRequest,
    model: &str,
    stream: bool,
) -> Result<RequestPlan, GatewayError> {
    let plan = RequestPlan {
        method: rquest::Method::POST,
        url: format!("{BASE}/responses"),
        query: vec![],
        body: Some(crate::protocol::native_text_request::pack(
            req, model, stream, invalid,
        )?),
        response_kind: EndpointKind::Responses,
    };
    validate_plan(payload, &plan)?;
    Ok(plan)
}

pub(super) async fn read(
    response: rquest::Response,
) -> Result<CanonicalRelayResponse, GatewayError> {
    let bytes = crate::protocol::upstream_body::collect_bounded_upstream_body(
        response,
        "Command Code Responses",
    )
    .await
    .map_err(|_| response_error())?;
    let body = crate::protocol::unique_json::parse(&bytes).map_err(|_| response_error())?;
    unpack(&body)
}

pub(super) fn unpack(body: &serde_json::Value) -> Result<CanonicalRelayResponse, GatewayError> {
    validate_native(body)?;
    // Probe and cross-protocol callers still require a lossless canonical representation.
    for item in body["output"].as_array().unwrap() {
        match item["type"].as_str() {
            Some("message")
                if item["content"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .all(|p| p["type"] == "output_text") => {}
            Some("function_call") => {}
            _ => return Err(response_error()),
        }
    }
    crate::protocol::responses::unpack_responses_response(body).map_err(|_| response_error())
}

pub(super) fn validate_native(body: &serde_json::Value) -> Result<(), GatewayError> {
    if body["object"] != "response"
        || !body["error"].is_null()
        || !matches!(body["status"].as_str(), Some("completed" | "incomplete"))
        || !body["id"].as_str().is_some_and(|s| !s.is_empty())
    {
        return Err(response_error());
    }
    let output = body["output"].as_array().ok_or_else(response_error)?;
    for item in output {
        match native_json::content_type(item)? {
            "message" => {
                if item["role"] != "assistant" {
                    return Err(response_error());
                }
                for part in item["content"].as_array().ok_or_else(response_error)? {
                    match native_json::content_type(part)? {
                        "output_text" if !part["text"].is_string() => return Err(response_error()),
                        "refusal" if !part["refusal"].is_string() => return Err(response_error()),
                        _ => {}
                    }
                }
            }
            "function_call" | "custom_tool_call" => {
                let input = if item["type"] == "custom_tool_call" {
                    "input"
                } else {
                    "arguments"
                };
                if !["call_id", "name", input]
                    .iter()
                    .all(|k| item[k].is_string())
                {
                    return Err(response_error());
                }
            }
            // Native clients own extension interpretation; preserve typed items rather than dropping them.
            _ => {}
        }
    }
    if let Some(usage) = body.get("usage").filter(|v| !v.is_null()) {
        if !usage.is_object()
            || [
                "input_tokens",
                "output_tokens",
                "total_tokens",
                "prompt_tokens",
                "completion_tokens",
            ]
            .iter()
            .any(|key| usage.get(key).is_some_and(|value| value.as_u64().is_none()))
        {
            return Err(response_error());
        }
        if let (Some(input), Some(output)) = (
            usage
                .get("input_tokens")
                .or_else(|| usage.get("prompt_tokens"))
                .and_then(serde_json::Value::as_u64),
            usage
                .get("output_tokens")
                .or_else(|| usage.get("completion_tokens"))
                .and_then(serde_json::Value::as_u64),
        ) {
            let total = input.checked_add(output).ok_or_else(response_error)?;
            if usage["total_tokens"]
                .as_u64()
                .is_some_and(|reported| reported != total)
            {
                return Err(response_error());
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    #[test]
    fn commandcode_native_response_rejects_failed_and_malformed_success() {
        let body = json!({"id":"r","object":"response","status":"completed","model":"m",
            "output":[{"type":"message","role":"assistant","content":[{"type":"output_text","text":"ok"}]}]});
        assert_eq!(unpack(&body).unwrap().text, "ok");
        for (key, value) in [
            ("status", json!("failed")),
            ("error", json!({"message":"secret"})),
            ("usage", json!({"input_tokens":-1})),
            ("output", json!([{"type":"future"}])),
        ] {
            let mut invalid = body.clone();
            invalid[key] = value;
            let error = unpack(&invalid).unwrap_err();
            assert!(!error.to_string().contains("secret"));
            assert!(!error.retryable);
        }
    }
}
