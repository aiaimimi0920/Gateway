use super::*;

fn minimal_policy_view_for_test(provider_supply_metric_kind: &str) -> CredentialStockPolicyView {
    CredentialStockPolicyView {
        id: "policy-1".to_string(),
        stock_class_key: "chatgpt:web_reverse:chat:session_auth:free".to_string(),
        display_name: "ChatGPT Web Reverse Free".to_string(),
        service_provider_key: "chatgpt_platform".to_string(),
        implementation_line_key: "chatgpt_web_reverse".to_string(),
        provider_surface_key: "chatgpt_web_reverse".to_string(),
        credential_material_kind: "session_auth".to_string(),
        provider_account_id: None,
        provider_adapter: None,
        selector: serde_json::json!({}),
        provider_supply_metric_kind: provider_supply_metric_kind.to_string(),
        metric_kind: provider_supply_metric_kind.to_string(),
        token_window_key: None,
        token_window_seconds: None,
        min_credential_count: Some(1),
        target_credential_count: Some(2),
        max_credential_count: None,
        min_average_available_tokens: None,
        target_average_available_tokens: None,
        signal_enabled: true,
        signal_stream: DEFAULT_SIGNAL_STREAM.to_string(),
        signal_cooldown_secs: 300,
        enabled: true,
        last_signal_key: None,
        last_signal_at: None,
        created_at: "2026-05-31T00:00:00Z".to_string(),
        updated_at: "2026-05-31T00:00:00Z".to_string(),
    }
}

#[test]
fn count_policy_reports_target_deficit_and_critical_severity() {
    let status = evaluate_credential_count(
        2,
        CountWatermark {
            min: Some(3),
            target: Some(5),
            max: Some(10),
        },
    );

    assert!(status.needs_replenishment);
    assert_eq!(status.severity, StockSeverity::Critical);
    assert_eq!(status.deficit_to_min, 1);
    assert_eq!(status.deficit_to_target, 3);
    assert_eq!(status.suggested_credential_top_up_count, 3);
    assert_eq!(status.excess_over_max, 0);
}

#[test]
fn count_policy_reports_overstock_without_replenishment() {
    let status = evaluate_credential_count(
        12,
        CountWatermark {
            min: Some(3),
            target: Some(5),
            max: Some(10),
        },
    );

    assert!(!status.needs_replenishment);
    assert_eq!(status.severity, StockSeverity::Overstock);
    assert_eq!(status.deficit_to_min, 0);
    assert_eq!(status.deficit_to_target, 0);
    assert_eq!(status.suggested_credential_top_up_count, 0);
    assert_eq!(status.excess_over_max, 2);
}

#[test]
fn token_policy_reports_average_token_deficit() {
    let status = evaluate_token_window(
        &[Some(800), Some(1200), None],
        TokenWindowWatermark {
            min_average_available_tokens: Some(1_500),
            target_average_available_tokens: Some(2_000),
            target_credential_count: Some(4),
        },
    );

    assert!(status.needs_replenishment);
    assert_eq!(status.severity, StockSeverity::Critical);
    assert_eq!(status.known_token_credential_count, 2);
    assert_eq!(status.unknown_token_credential_count, 1);
    assert_eq!(status.average_available_tokens, Some(1_000));
    assert_eq!(status.deficit_to_min_average_tokens, 500);
    assert_eq!(status.deficit_to_target_average_tokens, 1_000);
    assert_eq!(status.suggested_credential_top_up_count, 2);
}

#[test]
fn signal_key_is_stable_for_same_deficit_bucket() {
    let left = build_signal_key("chatgpt-web-a", "credential_count", Some(3), None);
    let right = build_signal_key("chatgpt-web-a", "credential_count", Some(3), None);

    assert_eq!(left, right);
    assert_ne!(
        left,
        build_signal_key("chatgpt-web-a", "credential_count", Some(4), None)
    );
}

#[test]
fn credential_count_metric_does_not_need_quota_snapshot_reads() {
    assert!(!metric_requires_quota_snapshot("credential_count"));
    assert!(metric_requires_quota_snapshot("token_window"));
}

#[test]
fn normalize_required_key_rejects_values_that_normalize_to_empty() {
    let error = normalize_required_key(" !!! ", "stockClassKey", 160)
        .expect_err("punctuation-only values must not normalize to empty keys");

    assert_eq!(error.http_status, Some(400));
}

#[test]
fn credential_material_kind_allows_wildcard_selector() {
    assert_eq!(normalize_credential_material_kind("*").unwrap(), "*");
    assert_eq!(normalize_credential_material_kind("ANY").unwrap(), "any");
}

#[test]
fn token_window_policy_preserves_count_replenishment_need() {
    let count = evaluate_credential_count(
        1,
        CountWatermark {
            min: Some(2),
            target: Some(2),
            max: None,
        },
    );
    let token = evaluate_token_window(
        &[Some(10_000)],
        TokenWindowWatermark {
            min_average_available_tokens: Some(1),
            target_average_available_tokens: Some(1),
            target_credential_count: None,
        },
    );

    assert!(!token.needs_replenishment);
    assert!(combined_needs_replenishment(&count, Some(&token)));
    assert_eq!(
        combined_stock_severity(&count, Some(&token)),
        StockSeverity::Critical
    );
}

#[test]
fn normalize_optional_key_rejects_invalid_nonempty_values() {
    let error = normalize_optional_key(Some(" !!! "), "providerAccountId", 160)
        .expect_err("invalid nonempty optional keys must not silently become None");

    assert_eq!(error.http_status, Some(400));
}

#[test]
fn normalize_optional_key_keeps_empty_values_absent() {
    assert_eq!(
        normalize_optional_key(Some("   "), "providerAccountId", 160).unwrap(),
        None
    );
    assert_eq!(
        normalize_optional_key(None, "providerAccountId", 160).unwrap(),
        None
    );
}

#[test]
fn upsert_input_accepts_provider_supply_metric_kind_as_canonical_field() {
    let input: UpsertCredentialStockPolicyInput = serde_json::from_value(serde_json::json!({
        "stockClassKey": "chatgpt:web_reverse:chat:session_auth:free",
        "displayName": "ChatGPT Web Reverse Free",
        "serviceProviderKey": "chatgpt_platform",
        "implementationLineKey": "chatgpt_web_reverse",
        "providerSurfaceKey": "chatgpt_web_reverse",
        "credentialMaterialKind": "session_auth",
        "providerSupplyMetricKind": "credential_count"
    }))
    .expect("providerSupplyMetricKind is the canonical provider stock metric field");

    assert_eq!(
        resolve_provider_supply_metric_kind(
            input.provider_supply_metric_kind.as_deref(),
            input.legacy_metric_kind.as_deref(),
        )
        .unwrap(),
        "credential_count"
    );
}

#[test]
fn upsert_input_keeps_legacy_metric_kind_alias_for_compatibility() {
    let input: UpsertCredentialStockPolicyInput = serde_json::from_value(serde_json::json!({
        "stockClassKey": "chatgpt:web_reverse:chat:session_auth:free",
        "displayName": "ChatGPT Web Reverse Free",
        "serviceProviderKey": "chatgpt_platform",
        "implementationLineKey": "chatgpt_web_reverse",
        "providerSurfaceKey": "chatgpt_web_reverse",
        "credentialMaterialKind": "session_auth",
        "metricKind": "token_window"
    }))
    .expect("legacy metricKind remains accepted as a compatibility alias");

    assert_eq!(
        resolve_provider_supply_metric_kind(
            input.provider_supply_metric_kind.as_deref(),
            input.legacy_metric_kind.as_deref(),
        )
        .unwrap(),
        "token_window"
    );
}

#[test]
fn upsert_input_allows_round_tripped_canonical_and_legacy_alias_when_equal() {
    let input: UpsertCredentialStockPolicyInput = serde_json::from_value(serde_json::json!({
        "stockClassKey": "chatgpt:web_reverse:chat:session_auth:free",
        "displayName": "ChatGPT Web Reverse Free",
        "serviceProviderKey": "chatgpt_platform",
        "implementationLineKey": "chatgpt_web_reverse",
        "providerSurfaceKey": "chatgpt_web_reverse",
        "credentialMaterialKind": "session_auth",
        "providerSupplyMetricKind": "credential_count",
        "metricKind": "credential_count"
    }))
    .expect("round-tripped output may contain both canonical field and legacy alias");

    assert_eq!(
        resolve_provider_supply_metric_kind(
            input.provider_supply_metric_kind.as_deref(),
            input.legacy_metric_kind.as_deref(),
        )
        .unwrap(),
        "credential_count"
    );
}

#[test]
fn provider_supply_metric_kind_rejects_conflicting_legacy_alias() {
    let error = resolve_provider_supply_metric_kind(Some("credential_count"), Some("token_window"))
        .expect_err("conflicting canonical and legacy stock metrics must be rejected");

    assert_eq!(error.http_status, Some(400));
}

#[test]
fn policy_view_serializes_provider_supply_metric_kind_with_legacy_alias() {
    let policy = minimal_policy_view_for_test("credential_count");
    let value = serde_json::to_value(policy).expect("serialize policy view");

    assert_eq!(
        value["providerSupplyMetricKind"].as_str(),
        Some("credential_count")
    );
    assert_eq!(value["metricKind"].as_str(), Some("credential_count"));
}
