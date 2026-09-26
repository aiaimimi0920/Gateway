use super::*;

pub(super) fn build_policy_request_audit_filters(
    policy: &GatewayAnalysisAnomalyPolicyView,
) -> RequestAuditFilters {
    let mut filters = RequestAuditFilters {
        project_id: policy.project_id.clone(),
        route_policy_id: policy.route_policy_id.clone(),
        provider_account_id: None,
        session_id: None,
        api_key_id: None,
        user_credential_id: None,
        access_key_id: None,
        source_access_key_id: None,
        response_id: None,
        protocol_family: None,
        status: None,
        endpoint_kind: None,
        stream: None,
        error_code: None,
        fallback_eligible: None,
        artifact_available: None,
        created_from: None,
        created_to: None,
        limit: Some(200),
    };
    if let Some(tag) = trimmed_owned_ref_opt(policy.tag.as_deref()) {
        let (_, parts) = split_policy_tag_parts(tag);
        for (key, value) in parts {
            match key.as_str() {
                "provider" => filters.provider_account_id = Some(value),
                "protocol" => filters.protocol_family = Some(value),
                "endpoint" => filters.endpoint_kind = Some(value),
                "api-key" => filters.api_key_id = Some(value),
                "session" => filters.session_id = Some(value),
                "response" => filters.response_id = Some(value),
                "status" => filters.status = Some(value),
                _ => {}
            }
        }
    }
    filters
}

pub(super) fn build_policy_analysis_export_filters(
    policy: &GatewayAnalysisAnomalyPolicyView,
) -> GatewayPersistedAnalysisExportFilters {
    GatewayPersistedAnalysisExportFilters {
        label: None,
        tag: trimmed_owned(policy.tag.as_deref()),
        project_id: trimmed_owned(policy.project_id.as_deref()),
        status: Some("active".to_string()),
        text_mode: trimmed_owned(policy.text_mode.as_deref()),
        created_from: None,
        created_to: None,
        limit: None,
        ..GatewayPersistedAnalysisExportFilters::default()
    }
}

pub(super) fn build_ad_hoc_analysis_export_filters(
    policy: Option<&GatewayAnalysisAnomalyPolicyView>,
    input: &GatewayAnalysisAnomalyIncidentSyncInput,
    effective_tag: Option<&str>,
) -> GatewayPersistedAnalysisExportFilters {
    GatewayPersistedAnalysisExportFilters {
        label: trimmed_owned(input.label.as_deref()),
        tag: trimmed_owned(input.tag.as_deref())
            .or_else(|| {
                effective_tag
                    .and_then(trimmed_owned_ref)
                    .map(str::to_string)
            })
            .or_else(|| policy.and_then(|item| trimmed_owned(item.tag.as_deref()))),
        project_id: trimmed_owned(input.project_id.as_deref())
            .or_else(|| policy.and_then(|item| trimmed_owned(item.project_id.as_deref()))),
        status: trimmed_owned(input.status.as_deref()).or_else(|| Some("active".to_string())),
        text_mode: trimmed_owned(input.text_mode.as_deref())
            .or_else(|| policy.and_then(|item| trimmed_owned(item.text_mode.as_deref()))),
        created_from: trimmed_owned(input.created_from.as_deref()),
        created_to: trimmed_owned(input.created_to.as_deref()),
        limit: input.limit,
        ..GatewayPersistedAnalysisExportFilters::default()
    }
}

pub(super) fn build_analysis_export_auto_escalation_config(
    policy: Option<&GatewayAnalysisAnomalyPolicyView>,
) -> GatewayAnalysisExportAutoEscalationConfig {
    let Some(policy) = policy else {
        return GatewayAnalysisExportAutoEscalationConfig::default();
    };
    GatewayAnalysisExportAutoEscalationConfig {
        enabled: policy.auto_escalate_enabled,
        severity_threshold: trimmed_owned(policy.escalate_severity_threshold.as_deref()),
        after_sync_count: policy.escalate_after_sync_count,
        owner_user_id: trimmed_owned(policy.auto_escalate_owner_user_id.as_deref()),
        follow_up_status: trimmed_owned(policy.auto_escalate_follow_up_status.as_deref()),
    }
}

pub(super) fn has_request_audit_specific_sync_hints(
    input: &GatewayAnalysisAnomalyIncidentSyncInput,
) -> bool {
    input.provider_account_id.is_some()
        || input.session_id.is_some()
        || input.api_key_id.is_some()
        || input.user_credential_id.is_some()
        || input.response_id.is_some()
        || input.protocol_family.is_some()
        || input.endpoint_kind.is_some()
        || input.stream.is_some()
        || input.error_code.is_some()
        || input.fallback_eligible.is_some()
        || input.artifact_available.is_some()
}

pub(super) fn build_ad_hoc_request_audit_filters(
    policy: Option<&GatewayAnalysisAnomalyPolicyView>,
    input: &GatewayAnalysisAnomalyIncidentSyncInput,
    effective_tag: Option<&str>,
) -> RequestAuditFilters {
    let mut filters = policy
        .map(build_policy_request_audit_filters)
        .unwrap_or_else(|| RequestAuditFilters {
            project_id: None,
            route_policy_id: None,
            provider_account_id: None,
            session_id: None,
            api_key_id: None,
            user_credential_id: None,
            access_key_id: None,
            source_access_key_id: None,
            response_id: None,
            protocol_family: None,
            status: None,
            endpoint_kind: None,
            stream: None,
            error_code: None,
            fallback_eligible: None,
            artifact_available: None,
            created_from: None,
            created_to: None,
            limit: Some(200),
        });

    if policy.is_none() {
        if let Some(tag) = trimmed_owned_ref_opt(effective_tag) {
            let (_, parts) = split_policy_tag_parts(tag);
            for (key, value) in parts {
                match key.as_str() {
                    "provider" => filters.provider_account_id = Some(value),
                    "protocol" => filters.protocol_family = Some(value),
                    "endpoint" => filters.endpoint_kind = Some(value),
                    "api-key" => filters.api_key_id = Some(value),
                    "session" => filters.session_id = Some(value),
                    "response" => filters.response_id = Some(value),
                    "status" => filters.status = Some(value),
                    _ => {}
                }
            }
        }
    }

    if input.project_id.is_some() {
        filters.project_id = trimmed_owned(input.project_id.as_deref());
    }
    if input.route_policy_id.is_some() {
        filters.route_policy_id = trimmed_owned(input.route_policy_id.as_deref());
    }
    if input.provider_account_id.is_some() {
        filters.provider_account_id = trimmed_owned(input.provider_account_id.as_deref());
    }
    if input.session_id.is_some() {
        filters.session_id = trimmed_owned(input.session_id.as_deref());
    }
    if input.api_key_id.is_some() {
        filters.api_key_id = trimmed_owned(input.api_key_id.as_deref());
    }
    if input.user_credential_id.is_some() {
        filters.user_credential_id = trimmed_owned(input.user_credential_id.as_deref());
    }
    if input.response_id.is_some() {
        filters.response_id = trimmed_owned(input.response_id.as_deref());
    }
    if input.protocol_family.is_some() {
        filters.protocol_family = trimmed_owned(input.protocol_family.as_deref());
    }
    if input.status.is_some() {
        filters.status = trimmed_owned(input.status.as_deref());
    }
    if input.endpoint_kind.is_some() {
        filters.endpoint_kind = trimmed_owned(input.endpoint_kind.as_deref());
    }
    if input.stream.is_some() {
        filters.stream = input.stream;
    }
    if input.error_code.is_some() {
        filters.error_code = trimmed_owned(input.error_code.as_deref());
    }
    if input.fallback_eligible.is_some() {
        filters.fallback_eligible = input.fallback_eligible;
    }
    if input.artifact_available.is_some() {
        filters.artifact_available = input.artifact_available;
    }
    if input.created_from.is_some() {
        filters.created_from = trimmed_owned(input.created_from.as_deref());
    }
    if input.created_to.is_some() {
        filters.created_to = trimmed_owned(input.created_to.as_deref());
    }
    if input.limit.is_some() {
        filters.limit = input.limit;
    }

    filters
}

pub(super) fn split_policy_tag_parts(tag: &str) -> (Option<String>, Vec<(String, String)>) {
    let segments = tag
        .split(':')
        .map(str::trim)
        .filter(|segment| !segment.is_empty())
        .map(|segment| segment.to_string())
        .collect::<Vec<_>>();
    if segments.len() < 2 {
        return (None, Vec::new());
    }
    let profile_key = Some(segments[1].clone());
    let mut pairs = Vec::new();
    let mut index = 2usize;
    while index + 1 < segments.len() {
        pairs.push((segments[index].clone(), segments[index + 1].clone()));
        index += 2;
    }
    (profile_key, pairs)
}
