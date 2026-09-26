use super::*;

#[test]
fn supported_sync_kind_recognizes_provider_routing_and_hotspot_tags() {
    assert!(matches!(
        determine_supported_sync_kind_from_tag(Some("provider-routing:balanced")),
        super::SupportedPolicySyncKind::ProviderRouting
    ));
    assert!(matches!(
        determine_supported_sync_kind_from_tag(Some("rate-limit-hotspot:balanced")),
        super::SupportedPolicySyncKind::RateLimitHotspot
    ));
    assert!(matches!(
        determine_supported_sync_kind_from_tag(Some("analysis-export:balanced")),
        super::SupportedPolicySyncKind::AnalysisExport
    ));
}

#[test]
fn ad_hoc_request_filters_parse_tag_parts_and_explicit_overrides() {
    let filters = build_ad_hoc_request_audit_filters(
        None,
        &GatewayAnalysisAnomalyIncidentSyncInput {
            project_id: Some("project-1".to_string()),
            status: Some("failed".to_string()),
            limit: Some(25),
            ..GatewayAnalysisAnomalyIncidentSyncInput::default()
        },
        Some("provider-routing:balanced:provider:provider-a:endpoint:search"),
    );
    assert_eq!(filters.project_id.as_deref(), Some("project-1"));
    assert_eq!(filters.provider_account_id.as_deref(), Some("provider-a"));
    assert_eq!(filters.endpoint_kind.as_deref(), Some("search"));
    assert_eq!(filters.status.as_deref(), Some("failed"));
    assert_eq!(filters.limit, Some(25));
}

#[test]
fn ad_hoc_analysis_export_defaults_to_active_status() {
    let filters = build_ad_hoc_analysis_export_filters(
        None,
        &GatewayAnalysisAnomalyIncidentSyncInput {
            label: Some("Nightly".to_string()),
            project_id: Some("project-1".to_string()),
            text_mode: Some("chat".to_string()),
            ..GatewayAnalysisAnomalyIncidentSyncInput::default()
        },
        Some("custom-export-tag"),
    );
    assert_eq!(filters.label.as_deref(), Some("Nightly"));
    assert_eq!(filters.project_id.as_deref(), Some("project-1"));
    assert_eq!(filters.tag.as_deref(), Some("custom-export-tag"));
    assert_eq!(filters.text_mode.as_deref(), Some("chat"));
    assert_eq!(filters.status.as_deref(), Some("active"));
}

#[test]
fn ad_hoc_sync_kind_defaults_to_analysis_export_without_request_audit_hints() {
    assert!(matches!(
        determine_supported_ad_hoc_sync_kind(
            Some("custom-export-tag"),
            &GatewayAnalysisAnomalyIncidentSyncInput {
                project_id: Some("project-1".to_string()),
                ..GatewayAnalysisAnomalyIncidentSyncInput::default()
            }
        ),
        super::SupportedPolicySyncKind::AnalysisExport
    ));
    assert!(matches!(
        determine_supported_ad_hoc_sync_kind(
            Some("custom-export-tag"),
            &GatewayAnalysisAnomalyIncidentSyncInput {
                provider_account_id: Some("provider-1".to_string()),
                ..GatewayAnalysisAnomalyIncidentSyncInput::default()
            }
        ),
        super::SupportedPolicySyncKind::Unsupported(_)
    ));
}

#[test]
fn analysis_anomaly_thresholds_match_balanced_defaults() {
    let thresholds = build_analysis_anomaly_threshold_config("balanced", None);
    assert_eq!(
        thresholds
            .get("failureRateWarningThreshold")
            .and_then(Value::as_f64),
        Some(0.15)
    );
    assert_eq!(
        thresholds
            .get("tokensPerSampleCriticalAbsoluteThreshold")
            .and_then(Value::as_f64),
        Some(2000.0)
    );
}
