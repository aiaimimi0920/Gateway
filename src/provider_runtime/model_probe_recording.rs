//! Model tests use cancellation-safe local audits and the normal credential-health stores.
use std::time::Duration;
pub(super) mod sap;

use serde_json::json;

use super::console_model_probe::ModelProbeResult;
use super::{
    model_probe_audit::{InternalProbeProduct, ProbeAudit},
    ProviderPayloadProbeReport, ProviderPayloadProbeStatus,
};
use crate::{
    config::GatewayStorageMode, db, error::GatewayError, routing::config::CredentialProbeTarget,
    state::AppState,
};

pub(super) struct Recording {
    audit: Option<ProbeAudit>,
    health: super::model_probe_health::ProbeHealth,
    trace: serde_json::Value,
    response_id: Option<String>,
    sap: Option<crate::protocol::sap_aicore::probe_evidence::ProbeEvidence>,
}

impl Recording {
    pub(super) fn luma_generation_audit(
        &mut self,
        state: &AppState,
        target: &CredentialProbeTarget,
    ) -> Option<crate::protocol::luma_agents::generation_audit::GenerationAudit> {
        self.audit
            .as_mut()
            .map(|audit| audit.luma_generation_audit(state, target))
    }
    pub(super) fn media_failure(
        mut self,
        adapter: &str,
        error: &crate::error::GatewayError,
    ) -> Self {
        if adapter == crate::protocol::luma_agents::ADAPTER {
            self.response_id =
                crate::protocol::luma_agents::record_failure_audit(error, &mut self.trace)
                    .or_else(|| self.audit.as_ref()?.record_luma_generation(&mut self.trace));
        }
        self
    }

    pub(super) fn lobster_selection(
        mut self,
        selection: Option<&crate::protocol::lobsterai::virtual_model::Selection>,
    ) -> Self {
        if let Some(selection) = selection {
            self.trace["upstreamModelResolution"] = selection.trace();
            self.trace["selectedUpstreamTargetProtocolFamily"] =
                if selection.model.api_format.as_deref() == Some("anthropic") {
                    "anthropic_messages".into()
                } else {
                    "openai_chat".into()
                };
        }
        self
    }
    pub(super) fn media_result(
        mut self,
        adapter: &str,
        model: &str,
        body: Option<&serde_json::Value>,
    ) -> Self {
        if adapter == "lumalabs_compatible" {
            if body.is_some() {
                // Validated bytes are not a supplier model receipt; never retain signed URLs.
                self.trace["lumaWebImage"] = json!({"verified":true,"count":1,
                    "modelVerified":false,"supplierCharge":null});
            }
            return self;
        }
        if adapter == "udio_compatible" {
            if let Some(body) = body {
                // Only admitted task facts; no song IDs, playback URLs, lyrics or false audio proof.
                self.trace["udioMusicTask"] = json!({"taskCompleted":true,
                    "count":body["data"].as_array().expect("completed Udio task metadata").len(),
                    "audioVerified":false,"supplierCharge":null});
            }
            return self;
        }
        if adapter == "suno_compatible" {
            if let Some(body) = body {
                self.trace["sunoMusic"] = super::suno_media_probe::audit(body, model);
            }
            return self;
        }
        if adapter == "chataibot_compatible" && body.is_some() {
            // The ordinary sender has validated one image. Never copy URL, base64, prompt,
            // arbitrary upstream metadata or legacy estimated costs into this closed audit.
            self.trace["chataibotImage"] = json!({"verified":true,"count":1,"supplierCharge":null});
            return self;
        }
        self.response_id = body.and_then(|body| {
            crate::protocol::luma_agents::record_success_audit(Some(adapter), body, &mut self.trace)
        });
        self
    }
}

pub(super) async fn begin(
    state: &AppState,
    target: &CredentialProbeTarget,
    model: &str,
    responses: bool,
) -> Recording {
    begin_endpoint(
        state,
        target,
        model,
        (
            "openai",
            if responses {
                "responses"
            } else {
                "chat_completions"
            },
            responses,
        ),
    )
    .await
}

pub(super) enum InternalAuditFailure {
    Admission,
    Finalization,
}

impl InternalAuditFailure {
    pub(super) fn report(self, point: String) -> (String, ProviderPayloadProbeReport) {
        let message = match self {
            Self::Admission => "Internal model probe audit unavailable; no request was sent.",
            Self::Finalization => {
                "Internal model probe audit could not be finalized; request was not replayed."
            }
        };
        (
            point,
            ProviderPayloadProbeReport {
                health_recovery: Some(db::provider_credential_model_states::recovery::ModelHealthRecovery::StorageFailed),
                status: ProviderPayloadProbeStatus::Failed,
                message: message.into(),
                answer: None,
            },
        )
    }
}

pub(super) async fn begin_internal(
    state: &AppState,
    product: InternalProbeProduct,
    target: &CredentialProbeTarget,
    model: &str,
) -> Result<Recording, InternalAuditFailure> {
    if !requires_server_audit(state) {
        return Ok(begin(state, target, model, false).await);
    }
    let health = super::model_probe_health::begin(state, target, model).await;
    let trace = json!({"realCredentialRef": target.credential_id, "consoleModelProbe": true});
    let input = audit_input(
        target,
        model,
        model,
        ("openai", "chat_completions", false),
        &trace,
    );
    // A Server probe has no public caller. Persist its internal admission before sending.
    let audit = ProbeAudit::begin_server_internal(state, product, &target.credential_id, input)
        .await
        .map_err(|_| InternalAuditFailure::Admission)?;
    Ok(Recording {
        audit: Some(audit),
        health,
        trace,
        response_id: None,
        sap: None,
    })
}

fn requires_server_audit(state: &AppState) -> bool {
    state.local_runtime.is_none() && state.config.storage_mode == GatewayStorageMode::Server
}

pub(super) async fn begin_search(
    state: &AppState,
    target: &CredentialProbeTarget,
    model: &str,
) -> Recording {
    begin_endpoint(state, target, model, ("search", "search", false)).await
}

pub(super) async fn begin_bedrock(
    state: &AppState,
    target: &CredentialProbeTarget,
    model: &str,
) -> Recording {
    begin_endpoint(
        state,
        target,
        model,
        ("bedrock_converse", "converse_stream", true),
    )
    .await
}

pub(super) async fn begin_fetch(
    state: &AppState,
    target: &CredentialProbeTarget,
    model: &str,
) -> Recording {
    begin_endpoint(state, target, model, ("search", "fetch", false)).await
}

async fn begin_endpoint(
    state: &AppState,
    target: &CredentialProbeTarget,
    model: &str,
    endpoint: (&str, &str, bool),
) -> Recording {
    let health = super::model_probe_health::begin(state, target, model).await;
    let trace = json!({"realCredentialRef": target.credential_id, "consoleModelProbe": true});
    Recording {
        audit: begin_audit(state, target, model, model, endpoint, &trace).await,
        health,
        trace,
        response_id: None,
        sap: None,
    }
}

pub(super) async fn begin_media(
    state: &AppState,
    target: &CredentialProbeTarget,
    model: &str,
    requested: &str,
    operation: super::media_probe_request::MediaProbeOperation,
    family: &str,
) -> Recording {
    let health = super::model_probe_health::begin(state, target, model).await;
    let trace = json!({"realCredentialRef": target.credential_id, "consoleModelProbe": true,
        "consoleMediaProbe": {"operation": operation, "chargeConfirmed": true}});
    Recording {
        audit: begin_audit(
            state,
            target,
            model,
            requested,
            (family, operation.endpoint(), false),
            &trace,
        )
        .await,
        health,
        trace,
        response_id: None,
        sap: None,
    }
}

async fn begin_audit(
    state: &AppState,
    target: &CredentialProbeTarget,
    model: &str,
    requested: &str,
    (family, endpoint, stream): (&str, &str, bool),
    trace: &serde_json::Value,
) -> Option<ProbeAudit> {
    if state.local_runtime.is_none() && state.pg_pool.is_none() {
        return None;
    }
    ProbeAudit::begin(
        state,
        audit_input(target, model, requested, (family, endpoint, stream), trace),
    )
    .await
}

fn audit_input(
    target: &CredentialProbeTarget,
    model: &str,
    requested: &str,
    (family, endpoint, stream): (&str, &str, bool),
    trace: &serde_json::Value,
) -> db::CreateRequestAuditInput {
    db::CreateRequestAuditInput {
        project_id: "console-model-probe".into(),
        api_key_id: None,
        user_credential_id: None,
        access_key_id: None,
        source_access_key_id: None,
        session_id: None,
        route_policy_id: None,
        provider_account_id: Some(target.provider_id.clone()),
        protocol_family: family.into(),
        endpoint_kind: endpoint.into(),
        requested_model: Some(requested.into()),
        resolved_model: Some(model.into()),
        model_alias: (requested != model).then(|| requested.into()),
        stream,
        route_attempt_count: 1,
        response_id: format!("probe-{}", uuid::Uuid::new_v4()),
        previous_response_id: None,
        route_trace: Some(trace.clone()),
    }
}

pub(super) async fn finish(
    state: &AppState,
    target: &CredentialProbeTarget,
    model: &str,
    recording: Recording,
    elapsed: Duration,
    result: &ModelProbeResult,
) -> db::provider_credential_model_states::recovery::ModelHealthRecovery {
    let (health, _) = finish_audit(target, model, recording, elapsed, result).await;
    // This remains an owned future: cancelling the probe does not detach a health update.
    Box::pin(super::model_probe_health::finish(
        state, target, model, health, result,
    ))
    .await
}

pub(super) async fn finish_internal(
    state: &AppState,
    target: &CredentialProbeTarget,
    model: &str,
    recording: Recording,
    elapsed: Duration,
    result: &ModelProbeResult,
) -> Result<db::provider_credential_model_states::recovery::ModelHealthRecovery, InternalAuditFailure>
{
    let (health, stored) = finish_audit(target, model, recording, elapsed, result).await;
    if requires_server_audit(state) {
        stored.map_err(|_| InternalAuditFailure::Finalization)?;
    }
    Ok(Box::pin(super::model_probe_health::finish(
        state, target, model, health, result,
    ))
    .await)
}

/// Audited task completion need not prove model identity or audio availability.
pub(super) async fn finish_without_health(
    target: &CredentialProbeTarget,
    model: &str,
    recording: Recording,
    elapsed: Duration,
    result: &ModelProbeResult,
) {
    let _ = finish_audit(target, model, recording, elapsed, result).await;
}

async fn finish_audit(
    target: &CredentialProbeTarget,
    model: &str,
    recording: Recording,
    elapsed: Duration,
    result: &ModelProbeResult,
) -> (
    super::model_probe_health::ProbeHealth,
    Result<(), GatewayError>,
) {
    let passed = result.reply.is_ok();
    let stored = if let Some(audit) = recording.audit {
        let mut input = db::FinalizeRequestAuditInput {
            client_disconnected: false,
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
            cache_creation_input_tokens: result
                .usage
                .get("cache_creation_input_tokens")
                .and_then(|value| value.as_u64()),
            cache_read_input_tokens: result
                .usage
                .get("cache_read_input_tokens")
                .and_then(|value| value.as_u64()),
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
            response_id: recording.response_id,
            route_trace: Some(recording.trace),
        };
        if let Some(evidence) = &recording.sap {
            evidence.project(&mut input);
        }
        audit.finish(input).await
    } else {
        Ok(())
    };
    (recording.health, stored)
}
