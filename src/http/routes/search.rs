// ---------------------------------------------------------------------------
// http/routes/search.rs — Search-provider endpoints
// ---------------------------------------------------------------------------

use std::collections::HashMap;
use std::sync::Arc;

use axum::{
    extract::{Path, Query, State},
    http::HeaderMap,
    response::{IntoResponse, Response},
    Json,
};
use serde::{Deserialize, Serialize};

use crate::error::GatewayError;
use crate::http::extractors::{BearerToken, JsonBody, OptionalBearerToken};
use crate::http::request_headers::apply_public_request_headers;
use crate::pipeline::{run_pipeline, PipelineContext, PipelineOutput};
use crate::protocol::canonical::EndpointKind;
use crate::protocol::search_api::{
    normalize_credits_balance, normalize_fetch, normalize_research_create, normalize_research_get,
    normalize_research_list, normalize_search, BALANCE_ROUTE_MODEL,
};
use crate::routing::config::CompiledProvider;
use crate::state::AppState;

#[derive(Debug, Deserialize, Default)]
pub struct InternalSearchProviderBalanceQuery {
    pub provider_id: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct SearchProviderBalanceOperatorResponse {
    pub provider_count: usize,
    pub credential_count: usize,
    pub success_count: usize,
    pub failure_count: usize,
    pub total_balance: f64,
    pub providers: Vec<SearchProviderBalanceProviderView>,
}

#[derive(Debug, Serialize)]
pub struct SearchProviderBalanceProviderView {
    pub provider_id: String,
    pub provider_label: String,
    pub adapter: String,
    pub base_url: String,
    pub supported_models: Vec<String>,
    pub credential_count: usize,
    pub success_count: usize,
    pub failure_count: usize,
    pub total_balance: f64,
    pub entries: Vec<SearchProviderBalanceCredentialView>,
}

#[derive(Debug, Serialize)]
pub struct SearchProviderBalanceCredentialView {
    pub credential_id: String,
    pub source: String,
    pub base_url: String,
    pub status: String,
    pub balance: Option<f64>,
    pub error: Option<String>,
    pub raw: Option<serde_json::Value>,
}

pub async fn handle_search(
    State(state): State<Arc<AppState>>,
    OptionalBearerToken(token): OptionalBearerToken,
    Query(query_params): Query<HashMap<String, String>>,
    headers: HeaderMap,
    JsonBody(body): JsonBody,
) -> Result<Response, GatewayError> {
    execute_search_provider(
        &state,
        token,
        headers,
        query_params,
        normalize_search(body)?,
    )
    .await
}

pub async fn handle_fetch(
    State(state): State<Arc<AppState>>,
    OptionalBearerToken(token): OptionalBearerToken,
    Query(query_params): Query<HashMap<String, String>>,
    headers: HeaderMap,
    JsonBody(body): JsonBody,
) -> Result<Response, GatewayError> {
    execute_search_provider(&state, token, headers, query_params, normalize_fetch(body)?).await
}

pub async fn handle_research_create(
    State(state): State<Arc<AppState>>,
    OptionalBearerToken(token): OptionalBearerToken,
    Query(query_params): Query<HashMap<String, String>>,
    headers: HeaderMap,
    JsonBody(body): JsonBody,
) -> Result<Response, GatewayError> {
    execute_search_provider(
        &state,
        token,
        headers,
        query_params,
        normalize_research_create(body)?,
    )
    .await
}

pub async fn handle_research_list(
    State(state): State<Arc<AppState>>,
    OptionalBearerToken(token): OptionalBearerToken,
    Query(query_params): Query<HashMap<String, String>>,
    headers: HeaderMap,
) -> Result<Response, GatewayError> {
    execute_search_provider(
        &state,
        token,
        headers,
        query_params.clone(),
        normalize_research_list(query_params)?,
    )
    .await
}

pub async fn handle_research_get(
    State(state): State<Arc<AppState>>,
    OptionalBearerToken(token): OptionalBearerToken,
    Path(research_id): Path<String>,
    Query(query_params): Query<HashMap<String, String>>,
    headers: HeaderMap,
) -> Result<Response, GatewayError> {
    execute_search_provider(
        &state,
        token,
        headers,
        query_params,
        normalize_research_get(&research_id)?,
    )
    .await
}

pub async fn handle_credits_balance(
    State(state): State<Arc<AppState>>,
    OptionalBearerToken(token): OptionalBearerToken,
    Query(query_params): Query<HashMap<String, String>>,
    headers: HeaderMap,
) -> Result<Response, GatewayError> {
    execute_search_provider(
        &state,
        token,
        headers,
        query_params,
        normalize_credits_balance()?,
    )
    .await
}

pub async fn handle_internal_search_provider_balance(
    State(state): State<Arc<AppState>>,
    BearerToken(token): BearerToken,
    Query(query): Query<InternalSearchProviderBalanceQuery>,
) -> Result<Json<SearchProviderBalanceOperatorResponse>, GatewayError> {
    if let Some(expected) = &state.config.gateway_api_key {
        if token != *expected {
            return Err(GatewayError::unauthorized("Invalid API key"));
        }
    }

    let providers = state
        .route_config
        .get_providers()
        .into_iter()
        .filter(|provider| provider.payload.canonical_adapter() == "search_api_compatible")
        .filter(|provider| {
            provider
                .payload
                .supports_search_endpoint(EndpointKind::CreditsBalance)
        })
        .filter(|provider| {
            query
                .provider_id
                .as_deref()
                .map(|expected| provider.id == expected)
                .unwrap_or(true)
        })
        .collect::<Vec<_>>();

    let mut provider_views = Vec::with_capacity(providers.len());
    let mut credential_count = 0usize;
    let mut success_count = 0usize;
    let mut failure_count = 0usize;
    let mut total_balance = 0.0f64;

    for provider in providers {
        let view = inspect_search_provider(&state, &provider).await;
        credential_count += view.credential_count;
        success_count += view.success_count;
        failure_count += view.failure_count;
        total_balance += view.total_balance;
        provider_views.push(view);
    }

    Ok(Json(SearchProviderBalanceOperatorResponse {
        provider_count: provider_views.len(),
        credential_count,
        success_count,
        failure_count,
        total_balance,
        providers: provider_views,
    }))
}

async fn execute_search_provider(
    state: &Arc<AppState>,
    token: Option<String>,
    headers: HeaderMap,
    query_params: HashMap<String, String>,
    canonical: crate::protocol::canonical::CanonicalRelayRequest,
) -> Result<Response, GatewayError> {
    let mut ctx = PipelineContext::new(canonical, token);

    apply_public_request_headers(&mut ctx, &headers, &state.config, &["x-project-id"]);
    if let Some(v) = headers.get("x-project-id").and_then(|v| v.to_str().ok()) {
        ctx.request_headers
            .insert("x-project-id".to_string(), v.to_string());
    }
    ctx.query_params = query_params;

    match run_pipeline(ctx, state).await? {
        PipelineOutput::Json(v) => Ok(Json(v).into_response()),
        PipelineOutput::Sse(_) => Err(GatewayError::server_error(
            "Search-provider endpoint unexpectedly produced a streaming response",
        )),
        PipelineOutput::Binary(_) => Err(GatewayError::server_error(
            "unexpected binary response for search-style endpoint",
        )),
    }
}

async fn inspect_search_provider(
    state: &Arc<AppState>,
    provider: &CompiledProvider,
) -> SearchProviderBalanceProviderView {
    let mut entries = Vec::new();
    let mut total_balance = 0.0f64;
    let mut success_count = 0usize;
    let mut failure_count = 0usize;

    if provider.credential_pool.is_empty() {
        let entry = inspect_search_provider_payload(
            state,
            &provider.id,
            provider
                .payload
                .credential_id
                .clone()
                .unwrap_or_else(|| provider.id.clone()),
            "provider_payload",
            &provider.payload,
        )
        .await;
        match entry.balance {
            Some(balance) => {
                success_count += 1;
                total_balance += balance;
            }
            None => failure_count += 1,
        }
        entries.push(entry);
    } else {
        for credential in &provider.credential_pool {
            let entry = inspect_search_provider_payload(
                state,
                &provider.id,
                credential.id.clone(),
                "credential_pool",
                &credential.payload,
            )
            .await;
            match entry.balance {
                Some(balance) => {
                    success_count += 1;
                    total_balance += balance;
                }
                None => failure_count += 1,
            }
            entries.push(entry);
        }
    }

    SearchProviderBalanceProviderView {
        provider_id: provider.id.clone(),
        provider_label: provider.label.clone(),
        adapter: provider.payload.canonical_adapter().to_string(),
        base_url: provider.payload.base_url.clone(),
        supported_models: provider.supported_models.clone(),
        credential_count: entries.len(),
        success_count,
        failure_count,
        total_balance,
        entries,
    }
}

async fn inspect_search_provider_payload(
    state: &Arc<AppState>,
    provider_account_id: &str,
    credential_id: String,
    source: &str,
    payload: &crate::routing::candidate::ProviderAccountPayload,
) -> SearchProviderBalanceCredentialView {
    let canonical = match normalize_credits_balance() {
        Ok(request) => request,
        Err(error) => {
            return SearchProviderBalanceCredentialView {
                credential_id,
                source: source.to_string(),
                base_url: payload.base_url.clone(),
                status: "error".to_string(),
                balance: None,
                error: Some(error.message),
                raw: None,
            };
        }
    };

    match state
        .upstream_client
        .execute_json_passthrough(
            provider_account_id,
            payload.resolve_execution_mode(EndpointKind::CreditsBalance),
            payload,
            &canonical,
            BALANCE_ROUTE_MODEL,
            None,
        )
        .await
    {
        Ok(raw) => {
            let balance = extract_search_provider_balance(&raw);
            SearchProviderBalanceCredentialView {
                credential_id,
                source: source.to_string(),
                base_url: payload.base_url.clone(),
                status: if balance.is_some() {
                    "ok".to_string()
                } else {
                    "invalid_response".to_string()
                },
                balance,
                error: if balance.is_some() {
                    None
                } else {
                    Some(
                        "Search provider balance response is missing a numeric `balance` field"
                            .to_string(),
                    )
                },
                raw: Some(raw),
            }
        }
        Err(error) => SearchProviderBalanceCredentialView {
            credential_id,
            source: source.to_string(),
            base_url: payload.base_url.clone(),
            status: "error".to_string(),
            balance: None,
            error: Some(error.message),
            raw: None,
        },
    }
}

fn extract_search_provider_balance(raw: &serde_json::Value) -> Option<f64> {
    match raw.get("balance") {
        Some(serde_json::Value::Number(value)) => value.as_f64(),
        Some(serde_json::Value::String(value)) => value.trim().parse::<f64>().ok(),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::extract_search_provider_balance;
    use serde_json::json;

    #[test]
    fn extract_search_provider_balance_reads_numeric_value() {
        assert_eq!(
            extract_search_provider_balance(&json!({ "balance": 5 })),
            Some(5.0)
        );
    }

    #[test]
    fn extract_search_provider_balance_reads_string_value() {
        assert_eq!(
            extract_search_provider_balance(&json!({ "balance": "7.5" })),
            Some(7.5)
        );
    }

    #[test]
    fn extract_search_provider_balance_rejects_missing_value() {
        assert_eq!(
            extract_search_provider_balance(&json!({ "credits": 5 })),
            None
        );
    }
}
