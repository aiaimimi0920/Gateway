use std::collections::HashMap;
use std::time::Duration;

use rquest::Client;
use serde_json::{json, Value};
use tracing::debug;

use crate::error::GatewayError;
use crate::protocol::chatgpt::web_reverse as surface;
use crate::protocol::upstream_body::collect_bounded_upstream_charset_text_with_provider;
use crate::routing::candidate::ProviderAccountPayload;

use super::super::common::classify_chatgpt_web_text_response;
use super::super::web_reverse::ChatGptWebRequestContext;
use super::super::PROVIDER;
use super::transport::post_json;

mod solver;

pub(super) async fn get_requirements(
    http: &Client,
    timeout: Duration,
    payload: &ProviderAccountPayload,
    request_context: &ChatGptWebRequestContext,
    bootstrap: &surface::ChatGptWebBootstrap,
    extra_headers: Option<&HashMap<String, String>>,
) -> Result<surface::ChatRequirements, GatewayError> {
    let user_agent = request_context.user_agent.as_str();
    let legacy_token = solver::legacy(bootstrap, user_agent).await?;
    let request_body = json!({ "p": legacy_token });
    let response = post_json(
        http,
        timeout,
        payload,
        request_context,
        extra_headers,
        surface::CHATGPT_WEB_DEFAULT_REQUIREMENTS_PATH,
        &request_body,
        None,
        false,
        None,
    )
    .await?;
    let status = response.status().as_u16();
    let content_type = response
        .headers()
        .get(rquest::header::CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .map(str::to_string);
    let body_text = collect_bounded_upstream_charset_text_with_provider(
        response,
        "ChatGPT Web reverse requirements body",
        PROVIDER,
    )
    .await?;
    classify_chatgpt_web_text_response(status, content_type.as_deref(), &body_text)?;

    let value: Value = serde_json::from_str(&body_text).map_err(|error| {
        GatewayError::server_error(format!(
            "ChatGPT Web reverse failed to decode chat requirements JSON: {error}"
        ))
        .with_provider(PROVIDER)
        .with_code("chatgpt_web_requirements_decode_failed")
    })?;
    let Some(object) = value.as_object() else {
        return Err(GatewayError::server_error(
            "ChatGPT Web reverse chat requirements payload was not a JSON object.",
        )
        .with_provider(PROVIDER)
        .with_code("chatgpt_web_requirements_invalid_shape"));
    };

    if object
        .get("arkose")
        .and_then(Value::as_object)
        .and_then(|record| record.get("required"))
        .and_then(Value::as_bool)
        .unwrap_or(false)
    {
        return Err(GatewayError::service_unavailable(
            "ChatGPT Web reverse upstream requested Arkose; this route currently requires a browser-backed refresh or manual challenge clearance.",
        )
        .with_provider(PROVIDER)
        .with_code(surface::CHATGPT_WEB_BROWSER_CHALLENGE_REQUIRED_CODE));
    }

    let mut proof_token = None;
    if object
        .get("proofofwork")
        .and_then(Value::as_object)
        .and_then(|record| record.get("required"))
        .and_then(Value::as_bool)
        .unwrap_or(false)
    {
        let proof = object
            .get("proofofwork")
            .and_then(Value::as_object)
            .unwrap();
        let seed = proof
            .get("seed")
            .and_then(Value::as_str)
            .unwrap_or_default();
        let difficulty = proof
            .get("difficulty")
            .and_then(Value::as_str)
            .unwrap_or("0fffff");
        proof_token = Some(solver::proof(bootstrap, user_agent, seed, difficulty).await?);
    }

    let turnstile = object.get("turnstile").and_then(Value::as_object);
    let turnstile_required = turnstile
        .and_then(|record| record.get("required"))
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let turnstile_dx = turnstile
        .and_then(|record| record.get("dx"))
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty());
    let turnstile_token = turnstile
        .and_then(|record| record.get("token"))
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string);
    let turnstile_token = match (turnstile_token, turnstile_dx) {
        (Some(token), _) => Some(token),
        (None, Some(dx)) => solver::turnstile(dx, &legacy_token).await?,
        (None, None) => None,
    };
    debug!(
        turnstile_required,
        turnstile_has_dx = turnstile_dx.is_some(),
        turnstile_dx_len = turnstile_dx.map(str::len).unwrap_or_default(),
        turnstile_solved = turnstile_token.is_some(),
        "chatgpt web turnstile requirements"
    );

    if turnstile_required && turnstile_token.is_none() {
        return Err(GatewayError::service_unavailable(
            "ChatGPT Web reverse upstream requested Turnstile and the browserless dx solver did not produce a token; this route currently requires a browser-backed refresh or manual challenge clearance.",
        )
        .with_provider(PROVIDER)
        .with_code(surface::CHATGPT_WEB_BROWSER_CHALLENGE_REQUIRED_CODE));
    }

    let token = object
        .get("token")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| {
            GatewayError::server_error(
                "ChatGPT Web reverse chat requirements payload did not include token.",
            )
            .with_provider(PROVIDER)
            .with_code("chatgpt_web_requirements_missing_token")
        })?;

    Ok(surface::ChatRequirements {
        token: token.to_string(),
        proof_token,
        turnstile_token,
        so_token: object
            .get("so_token")
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_string),
    })
}
