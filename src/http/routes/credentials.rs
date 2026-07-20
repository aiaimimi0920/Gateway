// ---------------------------------------------------------------------------
// http/routes/credentials.rs — Credential checkout API
//
// POST /v1/credentials/checkout
//
// "Unlimited refill" mode: returns a credential for the user to use
// locally. The platform does NOT relay the call — the user calls the
// upstream provider directly with the returned credential.
// ---------------------------------------------------------------------------

use std::collections::HashMap;
use std::sync::Arc;

use axum::extract::{Query, State};
use axum::http::HeaderMap;
use axum::Json;
use serde::{Deserialize, Serialize};

use crate::error::GatewayError;
use crate::http::extractors::OptionalBearerToken;
use crate::http::request_headers::build_public_auth_request;
use crate::redis::credential_cache;
use crate::state::AppState;

// ---------------------------------------------------------------------------
// Request / Response types
// ---------------------------------------------------------------------------

#[derive(Deserialize)]
pub struct CheckoutQuery {
    pub provider: Option<String>,
    pub model: Option<String>,
}

#[derive(Serialize)]
pub struct CheckoutResponse {
    pub credential: CheckoutCredential,
    pub usage_policy: String,
    pub valid_until: Option<String>,
}

#[derive(Serialize)]
pub struct CheckoutCredential {
    pub provider: String,
    pub base_url: Option<String>,
    pub api_key: Option<String>,
    pub headers: Option<HashMap<String, String>>,
    pub account_payload: Option<serde_json::Value>,
    pub supported_models: Vec<String>,
}

// ---------------------------------------------------------------------------
// Handler
// ---------------------------------------------------------------------------

/// POST /v1/credentials/checkout
pub async fn checkout(
    State(state): State<Arc<AppState>>,
    OptionalBearerToken(token): OptionalBearerToken,
    headers: HeaderMap,
    Query(query): Query<CheckoutQuery>,
) -> Result<Json<CheckoutResponse>, GatewayError> {
    let session = crate::auth::authenticate_request(
        state.as_ref(),
        &build_public_auth_request(
            &state.config,
            token.as_deref(),
            &headers,
            "/v1/credentials/checkout",
            "POST",
        ),
        headers
            .get("x-credential-ref")
            .and_then(|value| value.to_str().ok())
            .map(str::to_string),
    )
    .await?;

    let user_id = session.user_id.as_deref().ok_or_else(|| {
        GatewayError::bad_request("Credential checkout requires a user-bound session")
    })?;

    let credential = credential_cache::checkout_credential(
        &state.redis_pool,
        &session.project_id,
        user_id,
        query.provider.as_deref(),
        query.model.as_deref(),
    )
    .await
    .map_err(|e| GatewayError::server_error(format!("Credential checkout failed: {}", e)))?
    .ok_or_else(|| {
        GatewayError::bad_request("No credentials available for the requested provider/model")
    })?;

    Ok(Json(CheckoutResponse {
        credential: CheckoutCredential {
            provider: credential.provider.clone(),
            base_url: credential.api_base_url.clone(),
            api_key: credential.api_key.clone(),
            headers: credential.headers.clone(),
            account_payload: credential.account_payload.clone(),
            supported_models: credential.supported_models(),
        },
        usage_policy: "unlimited_refill".to_string(),
        valid_until: credential.expires_at.clone(),
    }))
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn checkout_query_deserializes_empty() {
        let q: CheckoutQuery = serde_json::from_str("{}").unwrap();
        assert!(q.provider.is_none());
        assert!(q.model.is_none());
    }

    #[test]
    fn checkout_query_deserializes_with_fields() {
        let q: CheckoutQuery =
            serde_json::from_str(r#"{"provider":"openai","model":"gpt-4o"}"#).unwrap();
        assert_eq!(q.provider.as_deref(), Some("openai"));
        assert_eq!(q.model.as_deref(), Some("gpt-4o"));
    }

    #[test]
    fn checkout_response_serializes() {
        let resp = CheckoutResponse {
            credential: CheckoutCredential {
                provider: "openai".to_string(),
                base_url: Some("https://api.openai.com".to_string()),
                api_key: Some("sk-test".to_string()),
                headers: None,
                account_payload: None,
                supported_models: vec!["gpt-4o".to_string()],
            },
            usage_policy: "unlimited_refill".to_string(),
            valid_until: None,
        };
        let json = serde_json::to_value(&resp).unwrap();
        assert_eq!(json["usage_policy"], "unlimited_refill");
        assert_eq!(json["credential"]["provider"], "openai");
        assert_eq!(json["credential"]["supported_models"][0], "gpt-4o");
    }
}
