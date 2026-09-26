use super::*;

pub(super) fn resolve_provider_routing_auto_escalation(
    severity: &str,
    sync_hit_count: i32,
) -> ProviderRoutingEscalationDecision {
    let normalized_severity = severity.trim().to_lowercase();
    let required_hit_count = if normalized_severity == "critical" {
        1
    } else {
        3
    };
    if sync_hit_count < required_hit_count {
        return ProviderRoutingEscalationDecision {
            should_escalate: false,
            reason: None,
            owner_user_id: None,
            follow_up_status: None,
        };
    }
    let reason = if normalized_severity == "critical" {
        format!(
            "Provider routing auto escalated immediately at severity critical after {sync_hit_count} sync hit(s)."
        )
    } else {
        format!("Provider routing auto escalated after {sync_hit_count} warning sync hit(s).")
    };
    ProviderRoutingEscalationDecision {
        should_escalate: true,
        reason: Some(reason),
        owner_user_id: None,
        follow_up_status: Some(
            if normalized_severity == "critical" {
                "investigating"
            } else {
                "monitoring"
            }
            .to_string(),
        ),
    }
}

pub(super) fn resolve_rate_limit_hotspot_auto_escalation(
    severity: &str,
    sync_hit_count: i32,
) -> ProviderRoutingEscalationDecision {
    let normalized_severity = severity.trim().to_lowercase();
    let required_hit_count = if normalized_severity == "critical" {
        1
    } else {
        3
    };
    if sync_hit_count < required_hit_count {
        return ProviderRoutingEscalationDecision {
            should_escalate: false,
            reason: None,
            owner_user_id: None,
            follow_up_status: None,
        };
    }
    let reason = if normalized_severity == "critical" {
        format!(
            "Hotspot auto escalated immediately at severity critical after {sync_hit_count} sync hit(s)."
        )
    } else {
        format!("Hotspot auto escalated after {sync_hit_count} warning sync hit(s).")
    };
    ProviderRoutingEscalationDecision {
        should_escalate: true,
        reason: Some(reason),
        owner_user_id: None,
        follow_up_status: Some(
            if normalized_severity == "critical" {
                "investigating"
            } else {
                "monitoring"
            }
            .to_string(),
        ),
    }
}

fn severity_rank(value: &str) -> i32 {
    match value.trim().to_lowercase().as_str() {
        "critical" => 2,
        "warning" => 1,
        _ => 0,
    }
}

pub(super) fn resolve_analysis_export_auto_escalation(
    config: &GatewayAnalysisExportAutoEscalationConfig,
    severity: &str,
    sync_hit_count: i32,
) -> ProviderRoutingEscalationDecision {
    if !config.enabled {
        return ProviderRoutingEscalationDecision {
            should_escalate: false,
            reason: None,
            owner_user_id: None,
            follow_up_status: None,
        };
    }

    let normalized_severity = severity.trim().to_lowercase();
    let required_severity = config
        .severity_threshold
        .as_deref()
        .and_then(trimmed_owned_ref)
        .unwrap_or("critical");
    let required_hit_count = config.after_sync_count.unwrap_or(3).max(1);
    if severity_rank(&normalized_severity) < severity_rank(required_severity)
        || sync_hit_count < required_hit_count
    {
        return ProviderRoutingEscalationDecision {
            should_escalate: false,
            reason: None,
            owner_user_id: None,
            follow_up_status: None,
        };
    }

    ProviderRoutingEscalationDecision {
        should_escalate: true,
        reason: Some(format!(
            "Auto escalated after {sync_hit_count} sync hit(s) at severity {normalized_severity}."
        )),
        owner_user_id: trimmed_owned(config.owner_user_id.as_deref()),
        follow_up_status: trimmed_owned(config.follow_up_status.as_deref()),
    }
}
