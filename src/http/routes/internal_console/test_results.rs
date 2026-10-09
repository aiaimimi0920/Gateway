//! Reading the last completed assessment never starts a call or claims a schedule slot.
use super::{
    console_request_context, json_with_no_store, required_console_management_token,
    ProviderProbeResponse,
};
use crate::{
    error::GatewayError, http::extractors::OptionalBearerToken,
    provider_runtime::ProviderPayloadProbeStatus, state::AppState,
};
use axum::{
    extract::{ConnectInfo, Path, State},
    http::HeaderMap,
    response::Response,
};
use std::{net::SocketAddr, sync::Arc};

pub async fn read_provider_test_results(
    State(state): State<Arc<AppState>>,
    Path(provider_id): Path<String>,
    connect_info: Option<ConnectInfo<SocketAddr>>,
    OptionalBearerToken(token): OptionalBearerToken,
    headers: HeaderMap,
) -> Result<Response, GatewayError> {
    read_results(state, provider_id, connect_info, token, headers, &[]).await
}

// Same-origin browser GETs may omit Origin. POST keeps the exact grant identity
// used by confirmation without relaxing authentication or starting model calls.
pub async fn read_provider_test_results_post(
    State(state): State<Arc<AppState>>,
    Path(provider_id): Path<String>,
    connect_info: Option<ConnectInfo<SocketAddr>>,
    OptionalBearerToken(token): OptionalBearerToken,
    headers: HeaderMap,
    body: axum::body::Bytes,
) -> Result<Response, GatewayError> {
    read_results(state, provider_id, connect_info, token, headers, &body).await
}

#[derive(Default, serde::Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ReadRequest {
    scope: Option<crate::provider_runtime::ConsoleTestScope>,
    plan_id: Option<String>,
    #[serde(default)]
    include_plans: bool,
}

async fn read_results(
    state: Arc<AppState>,
    provider_id: String,
    connect_info: Option<ConnectInfo<SocketAddr>>,
    token: Option<String>,
    headers: HeaderMap,
    body: &[u8],
) -> Result<Response, GatewayError> {
    let request = console_request_context(&headers, connect_info.as_ref());
    let actor = state.console_auth.authenticate_management_token(
        &request,
        required_console_management_token(token.as_deref(), &headers)?,
    )?;
    state.console_auth.verify_secret_grant(
        &request,
        &actor,
        headers
            .get("x-secret-grant")
            .and_then(|v| v.to_str().ok())
            .unwrap_or_default(),
    )?;
    let invalid = || {
        GatewayError::bad_request("Invalid test result scope.")
            .with_code("console_probe_input_invalid")
    };
    if body.len() > 1024 {
        return Err(invalid());
    }
    let options = if body.is_empty() {
        ReadRequest::default()
    } else {
        serde_json::from_slice::<ReadRequest>(body).map_err(|_| invalid())?
    };
    let scope = options.scope;
    if options
        .plan_id
        .as_ref()
        .is_some_and(|id| !crate::credential_test_policy::exact_model(id))
        || (options.plan_id.is_some() && options.include_plans)
    {
        return Err(invalid());
    }
    if scope.as_ref().is_some_and(|scope| match scope {
        crate::provider_runtime::ConsoleTestScope::Pool => false,
        crate::provider_runtime::ConsoleTestScope::Subpool { id }
        | crate::provider_runtime::ConsoleTestScope::Account { id } => {
            !crate::credential_test_policy::exact_model(id)
        }
    }) {
        return Err(invalid());
    }
    let snapshot = state.route_config.snapshot();
    let mut targets = snapshot
        .credential_probe_targets_for_provider(&provider_id)
        .ok_or_else(|| {
            GatewayError::not_found("Provider not found").with_code("console_provider_not_found")
        })?;
    if let Some(scope) = scope {
        targets.retain(|target| match &scope {
            crate::provider_runtime::ConsoleTestScope::Pool => true,
            crate::provider_runtime::ConsoleTestScope::Account { id } => {
                target.credential_id == *id
            }
            crate::provider_runtime::ConsoleTestScope::Subpool { id } => {
                snapshot.credential_subpool_id(target) == Some(id.as_str())
            }
        });
        if targets.is_empty() {
            return Err(GatewayError::bad_request(
                "The result scope has no accounts in this pool.",
            )
            .with_code("console_probe_scope_mismatch"));
        }
    }
    let plans: Vec<_> = if let Some(id) = options.plan_id.as_deref() {
        vec![Some(
            snapshot.named_test_plan(&provider_id, id).ok_or_else(|| {
                GatewayError::bad_request("Test plan is not present in this provider pool.")
                    .with_code("console_probe_plan_mismatch")
            })?,
        )]
    } else if options.include_plans {
        std::iter::once(None)
            .chain(
                snapshot
                    .document()
                    .providers
                    .iter()
                    .find(|provider| provider.id == provider_id)
                    .into_iter()
                    .flat_map(|provider| provider.test_plans.iter().take(32).map(Some)),
            )
            .collect()
    } else {
        vec![None]
    };
    let mut results = vec![];
    let mut truncated = false;
    'plans: for plan in plans {
        let matching: Vec<_> = targets
            .iter()
            .filter(|target| plan.is_none_or(|plan| snapshot.test_plan_matches(target, plan)))
            .take(128)
            .map(|target| target.credential_id.as_str())
            .collect();
        for result in crate::provider_runtime::test_store::read_many(
            &state,
            &provider_id,
            &matching,
            plan.map(|plan| plan.id.as_str()),
        )
        .await?
        {
            if results.len() == 128 {
                truncated = true;
                break 'plans;
            }
            results.push(result);
        }
        truncated |= targets
            .iter()
            .filter(|target| plan.is_none_or(|plan| snapshot.test_plan_matches(target, plan)))
            .count()
            > 128;
    }
    let passed_count = results
        .iter()
        .filter(|r| r.status == ProviderPayloadProbeStatus::Passed)
        .count();
    let failed_count = results
        .iter()
        .filter(|r| r.status == ProviderPayloadProbeStatus::Failed)
        .count();
    let unsupported_count = results.len() - passed_count - failed_count;
    let checked_at = results
        .iter()
        .map(|r| r.checked_at.as_str())
        .max()
        .map(str::to_string)
        .unwrap_or_else(crate::provider_runtime::test_results::checked_at);
    let mut response = serde_json::to_value(ProviderProbeResponse {
        provider_id,
        status: if failed_count > 0 {
            ProviderPayloadProbeStatus::Failed
        } else if passed_count > 0 {
            ProviderPayloadProbeStatus::Passed
        } else {
            ProviderPayloadProbeStatus::Unsupported
        },
        message: "Latest completed test rounds; reading results does not start calls.".into(),
        checked_at,
        total_count: results.len(),
        passed_count,
        failed_count,
        unsupported_count,
        results,
    })
    .expect("test result response");
    response["truncated"] = serde_json::json!(truncated);
    Ok(json_with_no_store(serde_json::json!({"result":response})))
}
