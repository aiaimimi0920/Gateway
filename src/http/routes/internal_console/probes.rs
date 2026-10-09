//! Credential and provider probes retain the exact secret grant, ordering and result attribution.

use super::{
    console_request_context, json_with_no_store, required_console_management_token,
    CredentialProbeResponse, ProviderProbeResponse,
};
use crate::error::GatewayError;
use crate::http::extractors::OptionalBearerToken;
use crate::provider_runtime::ProviderPayloadProbeStatus;
use crate::state::AppState;
use axum::extract::{ConnectInfo, Path, State};
use axum::http::HeaderMap;
use axum::response::Response;
use std::net::SocketAddr;
use std::sync::Arc;

pub async fn probe_console_credential(
    State(state): State<Arc<AppState>>,
    Path(credential_id): Path<String>,
    connect_info: Option<ConnectInfo<SocketAddr>>,
    OptionalBearerToken(token): OptionalBearerToken,
    headers: HeaderMap,
) -> Result<Response, GatewayError> {
    let request = console_request_context(&headers, connect_info.as_ref());
    let actor = state.console_auth.authenticate_management_token(
        &request,
        required_console_management_token(token.as_deref(), &headers)?,
    )?;
    let secret_grant = headers
        .get("x-secret-grant")
        .and_then(|value| value.to_str().ok())
        .unwrap_or_default();
    state
        .console_auth
        .verify_secret_grant(&request, &actor, secret_grant)?;

    let snapshot = state.route_config.snapshot();
    let target = snapshot
        .select_credential_probe_target(&credential_id)
        .ok_or_else(|| {
            GatewayError::not_found(format!(
                "Gateway console credential '{}' was not found",
                credential_id.trim()
            ))
            .with_code("console_credential_not_found")
        })?;
    if let Some((plan, source)) = snapshot.credential_test_policy(&target) {
        let result = crate::provider_runtime::test_execution::run(
            &state,
            &target,
            &plan,
            &source,
            "manual",
            &mut crate::provider_runtime::test_execution::RoundBudget::new(
                snapshot.revision().id(),
            ),
        )
        .await;
        crate::provider_runtime::test_store::save(&state, &result).await?;
        return Ok(json_with_no_store(serde_json::json!({"result":result})));
    }
    let (probe_point, report) =
        crate::provider_runtime::probe_console_target(&state, &target).await;
    crate::provider_runtime::record_provider_credential_probe_report(
        state.as_ref(),
        &target.provider_id,
        &target.credential_id,
        &report,
    )
    .await;

    let result = serde_json::to_value(CredentialProbeResponse {
        credential_id: target.credential_id,
        provider_id: target.provider_id,
        probe_point,
        status: report.status,
        message: report.message,
        checked_at: credential_probe_checked_at(),
        assessment: None,
    })
    .expect("credential probe response JSON");
    Ok(json_with_no_store(serde_json::json!({ "result": result })))
}

pub async fn probe_console_provider(
    State(state): State<Arc<AppState>>,
    Path(provider_id): Path<String>,
    connect_info: Option<ConnectInfo<SocketAddr>>,
    OptionalBearerToken(token): OptionalBearerToken,
    headers: HeaderMap,
    body: axum::body::Bytes,
) -> Result<Response, GatewayError> {
    let request = console_request_context(&headers, connect_info.as_ref());
    let actor = state.console_auth.authenticate_management_token(
        &request,
        required_console_management_token(token.as_deref(), &headers)?,
    )?;
    let secret_grant = headers
        .get("x-secret-grant")
        .and_then(|value| value.to_str().ok())
        .unwrap_or_default();
    state
        .console_auth
        .verify_secret_grant(&request, &actor, secret_grant)?;

    let options = crate::provider_runtime::ConsoleProbeRequest::parse(&body)?;
    let provider_id = provider_id.trim().to_string();
    let snapshot = state.route_config.snapshot();
    let mut targets = snapshot
        .credential_probe_targets_for_provider(&provider_id)
        .ok_or_else(|| {
            GatewayError::not_found(format!(
                "Gateway console provider '{provider_id}' was not found"
            ))
            .with_code("console_provider_not_found")
        })?;
    let named_plan = options
        .as_ref()
        .and_then(|request| request.plan_id.as_deref())
        .map(|id| {
            snapshot
                .named_test_plan(&provider_id, id)
                .cloned()
                .ok_or_else(|| {
                    GatewayError::bad_request("Test plan is not present in this provider pool.")
                        .with_code("console_probe_plan_mismatch")
                })
        })
        .transpose()?;
    if let Some(plan) = &named_plan {
        targets.retain(|target| snapshot.test_plan_matches(target, plan));
        if targets.is_empty() {
            return Err(
                GatewayError::bad_request("Test plan has no accounts in this pool.")
                    .with_code("console_probe_scope_mismatch"),
            );
        }
    }
    if let Some(ids) = options
        .as_ref()
        .and_then(|options| options.credential_ids.as_ref())
    {
        if ids
            .iter()
            .any(|id| !targets.iter().any(|target| &target.credential_id == id))
        {
            return Err(GatewayError::bad_request(
                "A selected credential is not in this provider pool.",
            )
            .with_code("console_probe_credential_mismatch"));
        }
        targets.retain(|target| ids.contains(&target.credential_id));
    }
    if let Some(scope) = options.as_ref().and_then(|request| request.scope.as_ref()) {
        targets.retain(|target| match scope {
            crate::provider_runtime::ConsoleTestScope::Pool => true,
            crate::provider_runtime::ConsoleTestScope::Subpool { id } => {
                snapshot.credential_subpool_id(target) == Some(id.as_str())
            }
            crate::provider_runtime::ConsoleTestScope::Account { id } => {
                target.credential_id == *id
            }
        });
        if targets.is_empty() {
            return Err(GatewayError::bad_request(
                "The selected test scope has no accounts in this pool.",
            )
            .with_code("console_probe_scope_mismatch"));
        }
    }
    if targets.len() > 128 {
        return Err(
            GatewayError::bad_request("Select at most 128 accounts for a manual test.")
                .with_code("console_probe_account_limit"),
        );
    }
    let checked_at = credential_probe_checked_at();
    let mut results = Vec::with_capacity(targets.len());
    let mut passed_count = 0usize;
    let mut failed_count = 0usize;
    let mut unsupported_count = 0usize;
    let mut budget =
        crate::provider_runtime::test_execution::RoundBudget::new(snapshot.revision().id());

    for target in targets {
        // Stop admitting manual calls after five minutes; finish and audit any in-flight call.
        if named_plan.is_none() && (!budget.available() || !budget.matches(&state)) {
            unsupported_count += 1;
            results.push(CredentialProbeResponse {
                credential_id: target.credential_id,
                provider_id: target.provider_id,
                probe_point: "Not executed".to_string(),
                status: ProviderPayloadProbeStatus::Unsupported,
                message: "Manual batch budget reached or route revision changed; this account was not called."
                    .to_string(),
                checked_at: checked_at.clone(),
                assessment: None,
            });
            continue;
        }
        let plan = named_plan
            .as_ref()
            .map(|plan| (plan.policy.clone(), format!("plan:{}", plan.id)))
            .or_else(|| match options.as_ref() {
                Some(request) => crate::provider_runtime::test_execution::plan_for_request(
                    &state, &target, request,
                ),
                None => snapshot.credential_test_policy(&target),
            });
        if let Some((plan, source)) = plan {
            let mut result = crate::provider_runtime::test_execution::run(
                &state,
                &target,
                &plan,
                &source,
                "manual",
                &mut budget,
            )
            .await;
            if let Some(assessment) = result.assessment.as_mut() {
                assessment.plan_id = named_plan.as_ref().map(|plan| plan.id.clone());
            }
            crate::provider_runtime::test_store::save(&state, &result).await?;
            match result.status {
                ProviderPayloadProbeStatus::Passed => passed_count += 1,
                ProviderPayloadProbeStatus::Failed => failed_count += 1,
                ProviderPayloadProbeStatus::Unsupported => unsupported_count += 1,
            }
            results.push(result);
            continue;
        }
        if !budget.admit() {
            continue;
        }
        let (probe_point, report) = crate::provider_runtime::probe_console_target_with_request(
            &state,
            &target,
            options.as_ref(),
        )
        .await;
        crate::provider_runtime::record_provider_credential_probe_report(
            state.as_ref(),
            &target.provider_id,
            &target.credential_id,
            &report,
        )
        .await;
        match report.status {
            ProviderPayloadProbeStatus::Passed => passed_count += 1,
            ProviderPayloadProbeStatus::Failed => failed_count += 1,
            ProviderPayloadProbeStatus::Unsupported => unsupported_count += 1,
        }
        results.push(CredentialProbeResponse {
            credential_id: target.credential_id,
            provider_id: target.provider_id,
            probe_point,
            status: report.status,
            message: report.message,
            checked_at: checked_at.clone(),
            assessment: None,
        });
    }

    let status = if failed_count > 0 {
        ProviderPayloadProbeStatus::Failed
    } else if passed_count > 0 {
        ProviderPayloadProbeStatus::Passed
    } else {
        ProviderPayloadProbeStatus::Unsupported
    };
    let total_count = results.len();
    let message = format!(
        "Provider test completed: {passed_count} passed, {failed_count} failed, {unsupported_count} unsupported."
    );
    let result = serde_json::to_value(ProviderProbeResponse {
        provider_id,
        status,
        message,
        checked_at,
        total_count,
        passed_count,
        failed_count,
        unsupported_count,
        results,
    })
    .expect("provider probe response JSON");
    Ok(json_with_no_store(serde_json::json!({ "result": result })))
}

fn credential_probe_checked_at() -> String {
    time::OffsetDateTime::now_utc()
        .format(&time::format_description::well_known::Rfc3339)
        .unwrap_or_else(|_| "unknown".to_string())
}
