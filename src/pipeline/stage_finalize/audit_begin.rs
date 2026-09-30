//! Request audit creation and owned snapshots for stream completion.

use super::*;

pub async fn begin_request_audit(ctx: &mut PipelineContext, state: &Arc<AppState>) {
    if ctx.request_audit_id.is_some() {
        return;
    }

    if state.pg_pool.is_none() && state.local_runtime.is_none() {
        return;
    }
    let Some(session) = ctx.session.as_ref() else {
        return;
    };
    // `dev-mode` and `gateway-key` are synthetic identifiers for the auth modes
    // that have no operator-issued key behind them. A standalone deployment
    // bootstraps a matching `gateway_api_keys` identity row (which carries no
    // secret material) so the audit satisfies both
    // `gateway_request_audits_identity_ck` and
    // `gateway_request_audits_api_key_id_fkey` and keeps the attribution.
    let api_key_id = session.api_key_id.clone();
    let user_credential_id = session.user_credential_id.clone();
    let access_key_id = session.access_key_id.clone();
    if api_key_id.is_none() && user_credential_id.is_none() && access_key_id.is_none() {
        return;
    }

    let input = db::CreateRequestAuditInput {
        project_id: session.project_id.clone(),
        api_key_id,
        user_credential_id,
        access_key_id,
        source_access_key_id: ctx.source_access_key_id.clone(),
        session_id: None,
        route_policy_id: ctx.route_policy_id.clone(),
        provider_account_id: None,
        protocol_family: protocol_family_name(ctx),
        endpoint_kind: endpoint_kind_name(ctx),
        requested_model: ctx.canonical_req.requested_model.clone(),
        resolved_model: None,
        model_alias: None,
        stream: ctx.stream,
        route_attempt_count: 1,
        response_id: format!("gw-audit-{}", ctx.req_id),
        previous_response_id: ctx.canonical_req.previous_response_id.clone(),
        route_trace: None,
    };

    match crate::local_runtime::audits::create(state, input).await {
        Ok(request_audit_id) => {
            ctx.request_audit_id = Some(request_audit_id);
        }
        Err(error) => {
            warn!(
                req_id = %ctx.req_id,
                error = %error,
                "failed to create request audit"
            );
        }
    }
}

pub fn snapshot_request_audit(ctx: &PipelineContext) -> RequestAuditFinalizeSnapshot {
    RequestAuditFinalizeSnapshot {
        request_id: ctx.req_id.to_string(),
        request_audit_id: ctx.request_audit_id.clone(),
        project_id: ctx
            .session
            .as_ref()
            .map(|session| session.project_id.clone()),
        user_id: archive_user_id(
            ctx.neuro_user_id.as_deref(),
            ctx.session
                .as_ref()
                .and_then(|session| session.user_id.as_deref()),
        ),
        session_id: ctx.canonical_req.explicit_session_key.clone(),
        credential_id: ctx
            .quota_credential_id
            .clone()
            .or_else(|| ctx.credential_ref.clone()),
        provider_credential_ref: ctx
            .selected_provider_credential_id
            .clone()
            .or_else(|| ctx.selected_platform_access_id.clone()),
        access_key_id: ctx.requesting_access_key_id.clone(),
        source_access_key_id: ctx.source_access_key_id.clone(),
        platform_access_id: ctx.selected_platform_access_id.clone(),
        real_credential_ref: ctx.selected_real_credential_ref.clone(),
        route_policy_id: ctx.route_policy_id.clone(),
        provider_account_id: ctx.audited_provider_id(),
        resolved_model: ctx.resolved_model.clone(),
        model_alias: ctx.selected_model_alias.clone(),
        route_selection_strategy: ctx.route_selection_strategy.clone(),
        selected_provider_label: ctx.selected_provider_label.clone(),
        selected_adapter: ctx.selected_adapter.clone(),
        selected_protocol_profile: ctx.selected_protocol_profile.clone(),
        selected_execution_mode: ctx.selected_execution_mode.clone(),
        selected_upstream_target_protocol_family: ctx
            .selected_upstream_target_protocol_family
            .clone(),
        selected_upstream_target_conversation_family: ctx
            .selected_upstream_target_conversation_family
            .clone(),
        canonical_conversation_semantics: ctx.canonical_conversation_semantics.clone(),
        selected_upstream_target_tool_family: ctx.selected_upstream_target_tool_family.clone(),
        tool_strategy: ctx.tool_strategy.clone(),
        canonical_tool_choice_semantics: ctx.canonical_tool_choice_semantics.clone(),
        canonical_completion_semantics: ctx.canonical_completion_semantics.clone(),
        selected_routing_score: ctx.selected_routing_score,
        selected_health_weight: ctx.selected_health_weight,
        selected_capacity_weight: ctx.selected_capacity_weight,
        selected_degraded: ctx.selected_degraded,
        selected_breaker_open: ctx.selected_breaker_open,
        selected_degradation_reasons: ctx.selected_degradation_reasons.clone(),
        route_attempt_count: ctx.route_attempt_count(),
        attempted_provider_ids: ctx.attempted_provider_ids(),
        protocol_family: protocol_family_name(ctx),
        endpoint_kind: endpoint_kind_name(ctx),
        requested_model: ctx.canonical_req.requested_model.clone(),
        request_payload: build_archive_request_payload_from_ctx(ctx),
        response_id: None,
        client_has_cache_control: ctx.client_has_cache_control,
        auto_cache_applied: ctx.auto_cache_applied,
        pre_deducted_tokens: ctx.quota_pre_deducted_tokens,
        started_at: ctx.started_at,
    }
}
