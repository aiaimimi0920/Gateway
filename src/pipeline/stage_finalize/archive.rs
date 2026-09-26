//! Buffered and streaming conversation-archive persistence inputs.

use super::*;

pub(super) async fn persist_conversation_archive_success(
    ctx: &PipelineContext,
    body: &Value,
    usage: Option<TokenUsage>,
    state: &Arc<AppState>,
) {
    if !is_conversation_archive_endpoint(ctx.canonical_req.endpoint_kind) {
        return;
    }
    let input = PersistConversationArchiveInput {
        request_audit_id: ctx.request_audit_id.clone(),
        request_id: ctx.req_id.to_string(),
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
        provider_account_id: ctx.selected_provider_id.clone(),
        provider_credential_ref: ctx
            .selected_provider_credential_id
            .clone()
            .or_else(|| ctx.selected_platform_access_id.clone()),
        protocol_family: protocol_family_name(ctx),
        protocol_profile: ctx.selected_protocol_profile.clone(),
        endpoint_kind: endpoint_kind_name(ctx),
        requested_model: ctx.canonical_req.requested_model.clone(),
        resolved_model: ctx.resolved_model.clone(),
        status: "completed".to_string(),
        upstream_status: Some(200),
        failure_class: None,
        failure_scope: None,
        request_payload: build_archive_request_payload_from_ctx(ctx),
        response_payload: Some(json!({
            "result": body,
            "usage": usage,
            "responseId": extract_response_id(body),
        })),
        request_already_truncated: false,
        response_already_truncated: false,
    };
    if let Err(error) = persist_conversation_archive(state, input).await {
        warn!(
            req_id = %ctx.req_id,
            error = %error,
            "failed to persist conversation archive"
        );
    }
}

pub(super) async fn persist_conversation_archive_failure(
    ctx: &PipelineContext,
    error: &crate::error::GatewayError,
    upstream_status: Option<u16>,
    state: &Arc<AppState>,
) {
    if !is_conversation_archive_endpoint(ctx.canonical_req.endpoint_kind) {
        return;
    }
    let classification = classify_provider_failure(
        upstream_status,
        error.code.as_deref(),
        Some(error.message.as_str()),
    );
    let input = PersistConversationArchiveInput {
        request_audit_id: ctx.request_audit_id.clone(),
        request_id: ctx.req_id.to_string(),
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
        provider_account_id: ctx.selected_provider_id.clone(),
        provider_credential_ref: ctx
            .selected_provider_credential_id
            .clone()
            .or_else(|| ctx.selected_platform_access_id.clone()),
        protocol_family: protocol_family_name(ctx),
        protocol_profile: ctx.selected_protocol_profile.clone(),
        endpoint_kind: endpoint_kind_name(ctx),
        requested_model: ctx.canonical_req.requested_model.clone(),
        resolved_model: ctx.resolved_model.clone(),
        status: "failed".to_string(),
        upstream_status,
        failure_class: Some(classification.class_name().to_string()),
        failure_scope: Some(classification.scope_name().to_string()),
        request_payload: build_archive_request_payload_from_ctx(ctx),
        response_payload: Some(json!({
            "error": {
                "message": error.message.clone(),
                "code": error.code.clone(),
                "kind": format!("{:?}", error.kind),
                "httpStatus": error.http_status,
            }
        })),
        request_already_truncated: false,
        response_already_truncated: false,
    };
    if let Err(error) = persist_conversation_archive(state, input).await {
        warn!(
            req_id = %ctx.req_id,
            error = %error,
            "failed to persist failed conversation archive"
        );
    }
}

pub(super) async fn persist_stream_conversation_archive_success(
    snapshot: &RequestAuditFinalizeSnapshot,
    usage: Option<TokenUsage>,
    raw_response_text: Option<String>,
    response_already_truncated: bool,
    state: &Arc<AppState>,
) {
    if !is_conversation_archive_endpoint_name(&snapshot.endpoint_kind) {
        return;
    }
    let input = PersistConversationArchiveInput {
        request_audit_id: snapshot.request_audit_id.clone(),
        request_id: snapshot.request_id.clone(),
        project_id: snapshot.project_id.clone(),
        user_id: snapshot.user_id.clone(),
        session_id: snapshot.session_id.clone(),
        provider_account_id: snapshot.provider_account_id.clone(),
        provider_credential_ref: snapshot.provider_credential_ref.clone(),
        protocol_family: snapshot.protocol_family.clone(),
        protocol_profile: snapshot.selected_protocol_profile.clone(),
        endpoint_kind: snapshot.endpoint_kind.clone(),
        requested_model: snapshot.requested_model.clone(),
        resolved_model: snapshot.resolved_model.clone(),
        status: "completed".to_string(),
        upstream_status: Some(200),
        failure_class: None,
        failure_scope: None,
        request_payload: snapshot.request_payload.clone(),
        response_payload: Some(json!({
            "rawSseText": raw_response_text,
            "usage": usage,
            "responseId": snapshot.response_id,
        })),
        request_already_truncated: false,
        response_already_truncated,
    };
    if let Err(error) = persist_conversation_archive(state, input).await {
        warn!(
            request_id = %snapshot.request_id,
            error = %error,
            "failed to persist stream conversation archive"
        );
    }
}

pub(super) async fn persist_stream_conversation_archive_failure(
    snapshot: &RequestAuditFinalizeSnapshot,
    failure_class: Option<String>,
    failure_scope: Option<String>,
    error_summary: Option<&str>,
    state: &Arc<AppState>,
) {
    if !is_conversation_archive_endpoint_name(&snapshot.endpoint_kind) {
        return;
    }
    let input = PersistConversationArchiveInput {
        request_audit_id: snapshot.request_audit_id.clone(),
        request_id: snapshot.request_id.clone(),
        project_id: snapshot.project_id.clone(),
        user_id: snapshot.user_id.clone(),
        session_id: snapshot.session_id.clone(),
        provider_account_id: snapshot.provider_account_id.clone(),
        provider_credential_ref: snapshot.provider_credential_ref.clone(),
        protocol_family: snapshot.protocol_family.clone(),
        protocol_profile: snapshot.selected_protocol_profile.clone(),
        endpoint_kind: snapshot.endpoint_kind.clone(),
        requested_model: snapshot.requested_model.clone(),
        resolved_model: snapshot.resolved_model.clone(),
        status: "failed".to_string(),
        upstream_status: None,
        failure_class,
        failure_scope,
        request_payload: snapshot.request_payload.clone(),
        response_payload: Some(json!({
            "error": {
                "message": error_summary,
                "code": "stream_failed",
            }
        })),
        request_already_truncated: false,
        response_already_truncated: false,
    };
    if let Err(error) = persist_conversation_archive(state, input).await {
        warn!(
            request_id = %snapshot.request_id,
            error = %error,
            "failed to persist failed stream conversation archive"
        );
    }
}

pub(super) fn build_archive_request_payload_from_ctx(ctx: &PipelineContext) -> Value {
    json!({
        "canonicalRequest": serde_json::to_value(&ctx.canonical_req).unwrap_or(Value::Null),
        "selected": {
            "providerAccountId": ctx.selected_provider_id.clone(),
            "providerCredentialRef": ctx.selected_provider_credential_id.clone(),
            "protocolProfile": ctx.selected_protocol_profile.clone(),
            "executionMode": ctx.selected_execution_mode.clone(),
            "resolvedModel": ctx.resolved_model.clone(),
        },
    })
}

fn is_conversation_archive_endpoint_name(endpoint_kind: &str) -> bool {
    matches!(
        endpoint_kind,
        "chat_completions" | "completions" | "messages" | "responses"
    )
}
