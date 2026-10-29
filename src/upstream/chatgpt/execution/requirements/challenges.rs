//! Existing challenge handling shared by the legacy and two-stage protocol contracts.
use super::super::super::PROVIDER;
use super::solver;
use crate::{error::GatewayError, protocol::chatgpt::web_reverse as surface};
use serde_json::Value;
use tracing::debug;

pub(super) struct ChallengeTokens {
    pub proof_token: Option<String>,
    pub turnstile_token: Option<String>,
}

pub(super) async fn resolve(
    data: &Value,
    source: &str,
    bootstrap: &surface::ChatGptWebBootstrap,
    user_agent: &str,
) -> Result<ChallengeTokens, GatewayError> {
    let required = |name| {
        data.get(name)
            .and_then(Value::as_object)
            .and_then(|record| record.get("required"))
            .and_then(Value::as_bool)
            .unwrap_or(false)
    };
    if required("arkose") {
        return Err(GatewayError::service_unavailable(
            "ChatGPT Web reverse upstream requested Arkose; this route currently requires a browser-backed refresh or manual challenge clearance.",
        ).with_provider(PROVIDER).with_code(surface::CHATGPT_WEB_BROWSER_CHALLENGE_REQUIRED_CODE));
    }
    let proof_token = if required("proofofwork") {
        let proof = &data["proofofwork"];
        Some(
            solver::proof(
                bootstrap,
                user_agent,
                proof
                    .get("seed")
                    .and_then(Value::as_str)
                    .unwrap_or_default(),
                proof
                    .get("difficulty")
                    .and_then(Value::as_str)
                    .unwrap_or("0fffff"),
            )
            .await?,
        )
    } else {
        None
    };
    let turnstile = data.get("turnstile").and_then(Value::as_object);
    let dx = turnstile
        .and_then(|record| record.get("dx"))
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty());
    let token = turnstile
        .and_then(|record| record.get("token"))
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string);
    let turnstile_token = match (token, dx) {
        (Some(token), _) => Some(token),
        (None, Some(dx)) => solver::turnstile(dx, source).await?,
        (None, None) => None,
    };
    debug!(
        turnstile_required = required("turnstile"),
        turnstile_has_dx = dx.is_some(),
        turnstile_dx_len = dx.map(str::len).unwrap_or_default(),
        turnstile_solved = turnstile_token.is_some(),
        "chatgpt web turnstile requirements"
    );
    if required("turnstile") && turnstile_token.is_none() {
        return Err(GatewayError::service_unavailable(
            "ChatGPT Web reverse upstream requested Turnstile and the browserless dx solver did not produce a token; this route currently requires a browser-backed refresh or manual challenge clearance.",
        ).with_provider(PROVIDER).with_code(surface::CHATGPT_WEB_BROWSER_CHALLENGE_REQUIRED_CODE));
    }
    Ok(ChallengeTokens {
        proof_token,
        turnstile_token,
    })
}
