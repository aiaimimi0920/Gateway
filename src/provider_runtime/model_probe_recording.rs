//! Model tests use the normal audit and credential-health stores in both storage modes.
use std::time::Duration;

use serde_json::json;

use super::console_model_probe::ModelProbeResult;
use crate::{db, local_runtime, routing::config::CredentialProbeTarget, state::AppState};

pub(super) async fn begin(
    state: &AppState,
    target: &CredentialProbeTarget,
    model: &str,
    responses: bool,
) -> Option<String> {
    if state.local_runtime.is_none() && state.pg_pool.is_none() {
        return None;
    }
    let input = db::CreateRequestAuditInput {
        project_id: "console-model-probe".into(),
        api_key_id: None,
        user_credential_id: None,
        access_key_id: None,
        source_access_key_id: None,
        session_id: None,
        route_policy_id: None,
        provider_account_id: Some(target.provider_id.clone()),
        protocol_family: "openai".into(),
        endpoint_kind: if responses {
            "responses"
        } else {
            "chat_completions"
        }
        .into(),
        requested_model: Some(model.into()),
        resolved_model: Some(model.into()),
        model_alias: None,
        stream: responses,
        route_attempt_count: 1,
        response_id: format!("probe-{}", uuid::Uuid::new_v4()),
        previous_response_id: None,
        route_trace: Some(
            json!({"realCredentialRef": target.credential_id, "consoleModelProbe": true}),
        ),
    };
    match local_runtime::audits::create(state, input).await {
        Ok(id) => Some(id),
        Err(error) => {
            tracing::warn!(error = %error, "cannot persist model probe audit");
            None
        }
    }
}

pub(super) async fn finish(
    state: &AppState,
    target: &CredentialProbeTarget,
    model: &str,
    audit: Option<&str>,
    elapsed: Duration,
    result: &ModelProbeResult,
) {
    let passed = result.reply.is_ok();
    if let Some(id) = audit {
        let input = db::FinalizeRequestAuditInput {
            status: if passed { "completed" } else { "failed" }.into(),
            upstream_status: result.status,
            duration_ms: elapsed.as_millis().min(u64::MAX as u128) as u64,
            prompt_tokens: result
                .usage
                .get("prompt_tokens")
                .or_else(|| result.usage.get("input_tokens"))
                .and_then(|value| value.as_u64()),
            completion_tokens: result
                .usage
                .get("completion_tokens")
                .or_else(|| result.usage.get("output_tokens"))
                .and_then(|value| value.as_u64()),
            total_tokens: result
                .usage
                .get("total_tokens")
                .and_then(|value| value.as_u64()),
            cache_creation_input_tokens: None,
            cache_read_input_tokens: None,
            client_has_cache_control: false,
            auto_cache_applied: false,
            error_summary: result.reply.as_ref().err().cloned(),
            access_key_id: None,
            source_access_key_id: None,
            session_id: None,
            route_policy_id: None,
            provider_account_id: Some(target.provider_id.clone()),
            resolved_model: Some(model.into()),
            model_alias: None,
            route_attempt_count: 1,
            response_id: None,
            route_trace: Some(
                json!({"realCredentialRef": target.credential_id, "consoleModelProbe": true}),
            ),
        };
        if let Err(error) = local_runtime::audits::finalize(state, id, input).await {
            tracing::warn!(error = %error, "cannot finalize model probe audit");
        }
    }
    let health = if passed {
        local_runtime::model_states::success(
            state,
            db::RecordCredentialModelSuccessInput {
                provider_account_id: target.provider_id.clone(),
                provider_credential_id: None,
                provider_credential_ref: Some(target.credential_id.clone()),
                protocol_profile: None,
                model: model.into(),
            },
        )
        .await
    } else {
        let error_message = result.reply.as_ref().err().cloned();
        local_runtime::model_states::failure(
            state,
            db::RecordCredentialModelFailureInput {
                provider_account_id: target.provider_id.clone(),
                provider_credential_id: None,
                provider_credential_ref: Some(target.credential_id.clone()),
                protocol_profile: None,
                model: model.into(),
                upstream_status: result.status,
                classification: crate::provider_failure::classify_provider_failure(
                    result.status,
                    None,
                    error_message.as_deref(),
                ),
                error_message,
            },
        )
        .await
    };
    if let Err(error) = health {
        tracing::warn!(error = %error, "cannot persist model probe health");
    }
}
