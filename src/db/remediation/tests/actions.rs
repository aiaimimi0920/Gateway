use super::*;

#[test]
fn provider_routing_plan_contains_expected_actions() {
    let plan = build_incident_remediation_plan(
        OffsetDateTime::now_utc(),
        base_incident("provider_routing_score_drop"),
        None,
        Some(base_route_policy()),
        IncidentSyncContext::default(),
    );
    assert!(plan
        .actions
        .iter()
        .any(|item| item.action_key == "provider-routing-review"));
    assert!(plan
        .actions
        .iter()
        .any(|item| item.action_key == "disable-prestream-fallback"));
    assert!(plan
        .actions
        .iter()
        .any(|item| item.action_key == "reduce-provider-concurrency"));
    assert!(plan
        .actions
        .iter()
        .any(|item| item.action_key == "provider-isolation"));
    assert!(plan
        .actions
        .iter()
        .any(|item| item.action_key == "owner-followup"));
}

#[test]
fn hotspot_config_fallback_maps_api_key_hotspot_action() {
    let config = resolve_rate_limit_hotspot_auto_remediation_config(
        &base_incident("rate_limit_api_key_hotspot"),
        Some(&base_route_policy()),
    );
    assert!(config.auto_remediation_enabled);
    assert_eq!(
        config.auto_remediation_action_keys,
        Some(vec!["tighten-api-key-rate-limit".to_string()])
    );
    assert!(config.auto_remediation_dry_run_first);
    assert!(config.auto_remediation_require_alert_before_apply);
    assert_eq!(config.auto_remediation_max_apply_runs_per_incident, Some(1));
}

#[test]
fn route_policy_patch_tightens_project_rate_limit() {
    let route_policy = base_route_policy();
    let patch = build_route_policy_patch(
        "tighten-project-rate-limit",
        &route_policy,
        Some(
            GatewayAnalysisAnomalyIncidentRemediationRoutePolicyPatchInput {
                project_rate_limit: Some(GatewayRateLimitDefinition {
                    window_seconds: 60,
                    max_requests: 24,
                }),
                ..Default::default()
            },
        ),
    )
    .expect("patch should build");
    assert_eq!(
        patch.next.project_rate_limit,
        Some(GatewayRateLimitDefinition {
            window_seconds: 60,
            max_requests: 24,
        })
    );
    assert!(patch
        .changed_fields
        .iter()
        .any(|item| item == "rateLimitWindowSeconds"));
    assert!(patch
        .changed_fields
        .iter()
        .any(|item| item == "rateLimitMaxRequests"));
}

#[test]
fn route_policy_patch_normalizes_endpoint_rate_limit_key() {
    let route_policy = base_route_policy();
    let patch = build_route_policy_patch(
        "tighten-endpoint-rate-limit",
        &route_policy,
        Some(
            GatewayAnalysisAnomalyIncidentRemediationRoutePolicyPatchInput {
                endpoint_rate_limit_key: Some("POST /V1/CHAT/COMPLETIONS".to_string()),
                endpoint_rate_limit: Some(GatewayRateLimitDefinition {
                    window_seconds: 60,
                    max_requests: 20,
                }),
                ..Default::default()
            },
        ),
    )
    .expect("patch should build");
    assert_eq!(
        patch.next.endpoint_rate_limit_key.as_deref(),
        Some("post /v1/chat/completions")
    );
}
