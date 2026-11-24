//! Vercel chat uses a curated catalog and one locked outbound wire.
use crate::{
    error::GatewayError,
    routing::{candidate::ProviderAccountPayload, config::ProviderConfigYaml},
    upstream::common::RequestPlan,
};
use rquest::header::{HeaderMap, HeaderValue, ACCEPT, AUTHORIZATION, CONTENT_TYPE};

pub(crate) const PRESET: &str = "vercel-ai-gateway";
pub(crate) const AUTH_MODE: &str = "vercel_free_key";
const CREDIT_MODE: &str = "vercel_monthly_credit_key";
pub(crate) const MODEL: &str = "nvidia/nemotron-3.5-lightning-free";
const ZERO: &[&str] = &[MODEL, "recursive/flint-v1.0"];
// Accept old declarations on load, but never advertise or send these models.
const RETIRED: &[&str] = &[
    "reflection/beam-free",
    "humansand/ampersand-v0.1",
    "stealth/glyph-cluster",
];
// Positive-price chat models marked eligible for free credits in Vercel's model browser.
const CREDIT: &[&str] = &[
    "alibaba/qwen-3-14b",
    "alibaba/qwen-3-30b",
    "alibaba/qwen3-coder-30b-a3b",
    "alibaba/qwen3.5-flash",
    "deepseek/deepseek-v3.1",
    "inclusionai/ling-3.0-flash",
    "liquid/lfm2.5-24b-a2b",
    "meta/llama-3.3-70b",
    "openai/gpt-5-mini",
    "openai/gpt-4.1-mini",
    "google/gemini-2.5-flash-lite",
    "google/gemini-2.5-flash",
    "minimax/minimax-m2.7",
    "zai/glm-4.7-flash",
    "moonshotai/kimi-k2.7-code",
    "alibaba/qwen3-coder-next",
    "mistral/mistral-small",
];
const BASE: &str = "https://ai-gateway.vercel.sh/v1";
const CHAT: &str = "https://ai-gateway.vercel.sh/v1/chat/completions";

fn invalid() -> GatewayError {
    GatewayError::bad_request(
        "Vercel AI Gateway requires its curated chat models and official endpoint.",
    )
    .with_code("vercel_free_configuration")
}

pub(crate) fn owns_payload(payload: &ProviderAccountPayload) -> bool {
    matches!(payload.auth_mode.as_deref(), Some(AUTH_MODE | CREDIT_MODE))
}

pub(crate) fn zero_model(model: &str) -> bool {
    ZERO.contains(&model)
}

pub(crate) fn monthly_credit(payload: &ProviderAccountPayload) -> bool {
    payload.auth_mode.as_deref() == Some(CREDIT_MODE)
}

pub(crate) fn allowed_model(model: &str) -> bool {
    zero_model(model) || CREDIT.contains(&model)
}

pub(crate) fn account_allows(payload: &ProviderAccountPayload, model: Option<&str>) -> bool {
    !owns_payload(payload)
        || model
            .or(payload.default_model.as_deref())
            .is_some_and(|id| zero_model(id) || (monthly_credit(payload) && CREDIT.contains(&id)))
}

pub(crate) fn validate(
    cfg: &ProviderConfigYaml,
    payload: &ProviderAccountPayload,
) -> Result<(), GatewayError> {
    if cfg.preset.as_deref() != Some(PRESET) && !owns_payload(payload) {
        return Ok(());
    }
    if cfg.preset.as_deref() != Some(PRESET)
        || !owns_payload(payload)
        || !valid_payload(payload)
        || cfg.supported_models.is_empty()
        || !cfg
            .supported_models
            .iter()
            .all(|model| allowed_model(model) || RETIRED.contains(&model.as_str()))
    {
        return Err(invalid());
    }
    Ok(())
}

fn valid_payload(payload: &ProviderAccountPayload) -> bool {
    payload.base_url == BASE
        && payload.canonical_adapter() == "openai_compatible"
        && payload.default_model.as_deref().is_some_and(zero_model)
        && payload.chat_completions_path.as_deref() == Some("/chat/completions")
        && payload.responses_path.is_none()
        && payload.session_auth.is_none()
        && payload.auth_token.is_none()
        && payload.endpoint_execution_modes.is_none()
        && !payload.request_client_profile.unwrap_or_default().is_cpa()
        && (!monthly_credit(payload) || payload.credential_id.is_some())
        && payload
            .execution_mode
            .is_none_or(|mode| mode == crate::routing::candidate::ProviderExecutionMode::DirectHttp)
}

fn valid_body(body: &serde_json::Map<String, serde_json::Value>) -> bool {
    body.get("model")
        .and_then(serde_json::Value::as_str)
        .is_some_and(allowed_model)
        && body.keys().all(|key| {
            matches!(
                key.as_str(),
                "model"
                    | "messages"
                    | "stream"
                    | "stream_options"
                    | "max_tokens"
                    | "max_completion_tokens"
                    | "temperature"
                    | "top_p"
                    | "stop"
                    | "seed"
                    | "frequency_penalty"
                    | "presence_penalty"
                    | "tools"
                    | "tool_choice"
                    | "parallel_tool_calls"
                    | "response_format"
                    | "n"
                    | "logprobs"
                    | "top_logprobs"
            )
        })
}

pub(crate) fn finalize(
    payload: &ProviderAccountPayload,
    plan: &RequestPlan,
    headers: &mut HeaderMap,
) -> Result<(), GatewayError> {
    if !owns_payload(payload) {
        return Ok(());
    }
    let body = plan
        .body
        .as_ref()
        .and_then(serde_json::Value::as_object)
        .ok_or_else(invalid)?;
    if !valid_payload(payload)
        || plan.method != rquest::Method::POST
        || plan.url != CHAT
        || !plan.query.is_empty()
        || !valid_body(body)
        || !account_allows(
            payload,
            body.get("model").and_then(serde_json::Value::as_str),
        )
    {
        return Err(invalid());
    }
    let key = payload.api_key.trim();
    if key.is_empty() {
        return Err(invalid());
    }
    let mut authorization =
        HeaderValue::from_str(&format!("Bearer {key}")).map_err(|_| invalid())?;
    authorization.set_sensitive(true);
    headers.clear();
    headers.insert(AUTHORIZATION, authorization);
    headers.insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));
    let accept = if body.get("stream").and_then(serde_json::Value::as_bool) == Some(true) {
        "text/event-stream"
    } else {
        "application/json"
    };
    headers.insert(ACCEPT, HeaderValue::from_static(accept));
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn vercel_free_rejects_other_models_and_provider_overrides() {
        for (body, allowed) in [
            (
                json!({"model": MODEL, "messages": [], "stream": true}),
                true,
            ),
            (json!({"model": "openai/paid", "messages": []}), false),
            (json!({"model": CREDIT[0], "messages": []}), true),
            (json!({"model": ZERO[1], "messages": []}), true),
            (json!({"model": RETIRED[0], "messages": []}), false),
            (
                json!({"model": MODEL, "providerOptions": {"gateway": {"models": ["openai/paid"]}}}),
                false,
            ),
        ] {
            assert_eq!(valid_body(body.as_object().unwrap()), allowed);
        }
    }

    #[test]
    fn vercel_free_compiles_with_an_explicit_account_and_rejects_overrides() {
        let document = json!({"providers": [{
            "id": PRESET, "preset": PRESET, "base_url": BASE,
            "supported_models": [MODEL], "explicit_credentials_only": true,
            "credentials": [{"id": "vercel-test", "api_key": "synthetic-test-key"}]
        }], "model_routes": [{"pattern": MODEL, "provider_ids": [PRESET]}]});
        let validate = |doc| {
            crate::console::document::validate_route_document(serde_json::from_value(doc).unwrap())
        };
        assert!(validate(document.clone()).is_ok());
        let mut bad_host = document.clone();
        bad_host["providers"][0]["base_url"] = json!("https://example.test/v1");
        assert!(validate(bad_host).is_err());
        let mut bad_model = document;
        bad_model["providers"][0]["supported_models"] = json!(["openai/paid"]);
        assert!(validate(bad_model).is_err());
    }
}
