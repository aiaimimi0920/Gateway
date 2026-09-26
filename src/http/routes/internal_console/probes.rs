//! Credential and provider probes retain the exact secret grant, ordering and result attribution.

use super::{
    console_request_context, json_with_no_store, required_console_management_token,
    CredentialProbeResponse, ProviderProbeResponse,
};
use crate::error::GatewayError;
use crate::http::extractors::OptionalBearerToken;
use crate::provider_runtime::{ProviderPayloadProbeReport, ProviderPayloadProbeStatus};
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
    let probe_point = crate::provider_runtime::provider_payload_probe_point(&target.payload);
    let report = if target.enabled {
        crate::provider_runtime::probe_provider_payload_for_console(
            state.upstream_client.client(),
            &target.payload,
        )
        .await
    } else {
        ProviderPayloadProbeReport {
            status: ProviderPayloadProbeStatus::Unsupported,
            message: "Credential is disabled.".to_string(),
        }
    };
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

    let provider_id = provider_id.trim().to_string();
    let targets = state
        .route_config
        .snapshot()
        .credential_probe_targets_for_provider(&provider_id)
        .ok_or_else(|| {
            GatewayError::not_found(format!(
                "Gateway console provider '{provider_id}' was not found"
            ))
            .with_code("console_provider_not_found")
        })?;
    let checked_at = credential_probe_checked_at();
    let mut results = Vec::with_capacity(targets.len());
    let mut passed_count = 0usize;
    let mut failed_count = 0usize;
    let mut unsupported_count = 0usize;

    for target in targets {
        let probe_point = crate::provider_runtime::provider_payload_probe_point(&target.payload);
        let report = if target.enabled {
            crate::provider_runtime::probe_provider_payload_for_console(
                state.upstream_client.client(),
                &target.payload,
            )
            .await
        } else {
            ProviderPayloadProbeReport {
                status: ProviderPayloadProbeStatus::Unsupported,
                message: "Credential is disabled.".to_string(),
            }
        };
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
