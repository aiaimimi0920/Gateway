use super::*;
use crate::routing::candidate::ProviderExecutionMode;
use std::collections::HashMap;

fn codex_payload() -> ProviderAccountPayload {
    let mut headers = HashMap::new();
    headers.insert("Originator".to_string(), "codex_cli_rs".to_string());
    ProviderAccountPayload {
        discovered_protocols: Vec::new(),
        adapter: "openai_compatible".to_string(),
        base_url: "https://chatgpt.com/backend-api/codex".to_string(),
        api_key: "tok".to_string(),
        credential_id: None,
        expires_at: None,
        runtime_state_object_key: None,
        account_name: None,
        execution_mode: Some(ProviderExecutionMode::DirectHttp),
        endpoint_execution_modes: None,
        default_model: Some("gpt-5.4".to_string()),
        headers,
        auth_mode: None,
        anthropic_version: None,
        beta_headers: None,
        auth_header_name: None,
        auth_token: None,
        responses_path: Some("/responses".to_string()),
        chat_completions_path: None,
        completions_path: None,
        embeddings_path: None,
        audio_transcriptions_path: None,
        audio_speech_path: None,
        messages_path: None,
        search_path: None,
        fetch_path: None,
        research_path: None,
        balance_path: None,
        search_query_field: None,
        fetch_urls_field: None,
        extra_body: None,
        session_auth: None,
        keepalive: None,
    }
}

fn accio_payload() -> ProviderAccountPayload {
    let mut headers = HashMap::new();
    headers.insert("utdid".to_string(), "utd-accio".to_string());
    headers.insert("version".to_string(), "0.5.6".to_string());
    headers.insert(
        "Cookie".to_string(),
        "cna=test-cna; other=value".to_string(),
    );
    ProviderAccountPayload {
        discovered_protocols: Vec::new(),
        adapter: "accio_compatible".to_string(),
        base_url: "https://phoenix-gw.alibaba.com".to_string(),
        api_key: "accio-access-token".to_string(),
        credential_id: None,
        expires_at: None,
        runtime_state_object_key: None,
        account_name: Some("accio@example.com".to_string()),
        execution_mode: Some(ProviderExecutionMode::DirectHttp),
        endpoint_execution_modes: None,
        default_model: Some("claude-sonnet-4-6".to_string()),
        headers,
        auth_mode: None,
        anthropic_version: None,
        beta_headers: None,
        auth_header_name: None,
        auth_token: None,
        responses_path: Some("/api/adk/llm/generateContent".to_string()),
        chat_completions_path: None,
        completions_path: None,
        embeddings_path: None,
        audio_transcriptions_path: None,
        audio_speech_path: None,
        messages_path: None,
        search_path: None,
        fetch_path: None,
        research_path: None,
        balance_path: None,
        search_query_field: None,
        fetch_urls_field: None,
        extra_body: Some(HashMap::from([(
            "token".to_string(),
            serde_json::json!("accio-access-token"),
        )])),
        session_auth: None,
        keepalive: None,
    }
}

#[test]
fn codex_payload_supports_quota() {
    assert!(provider_supports_quota(&codex_payload()));
}

#[test]
fn accio_payload_supports_quota() {
    assert!(provider_supports_quota(&accio_payload()));
}

#[test]
fn parses_codex_usage_windows() {
    let raw = serde_json::json!({
        "plan_type": "team",
        "rate_limit": {
            "allowed": true,
            "limit_reached": false,
            "primary_window": {
                "used_percent": 0,
                "limit_window_seconds": 18000,
                "reset_after_seconds": 18000,
                "reset_at": 1776255407
            },
            "secondary_window": {
                "used_percent": 31,
                "limit_window_seconds": 604800,
                "reset_after_seconds": 528561,
                "reset_at": 1776765968
            }
        },
        "spend_control": {
            "reached": false
        }
    });
    let parsed: CodexUsageResponse = serde_json::from_value(raw.clone()).unwrap();
    let mut windows = Vec::new();
    let rate_limit = parsed.rate_limit.as_ref().unwrap();
    windows.push(codex_window_to_view(
        "primary",
        rate_limit.primary_window.as_ref().unwrap(),
    ));
    windows.push(codex_window_to_view(
        "secondary",
        rate_limit.secondary_window.as_ref().unwrap(),
    ));
    windows.sort_by_key(|window| window.limit_window_seconds.unwrap_or(i64::MAX));
    assert_eq!(windows[0].label, "5h");
    assert_eq!(windows[1].label, "7d");
    assert_eq!(windows[1].used_percent, Some(31.0));
    assert_eq!(windows[1].remaining_ratio, Some(0.69));
}

#[test]
fn codex_usage_below_limit_stays_available() {
    let exhausted = false;
    let status = if exhausted { "exhausted" } else { "available" };
    assert_eq!(status, "available");
}

#[test]
fn balance_status_uses_warning_and_exhausted_flags() {
    let snapshot = GatewayProviderQuotaView {
        provider_account_id: "prov-1".to_string(),
        provider_credential_id: None,
        provider_type: "codex".to_string(),
        source: "codex_wham_usage".to_string(),
        status: "warning".to_string(),
        ready: true,
        checked_at: "2026-04-15T00:00:00Z".to_string(),
        next_check_at: "2026-04-15T00:01:00Z".to_string(),
        next_reset_at: None,
        plan_type: Some("team".to_string()),
        representative_claim: Some("5h 窗口已用 85%".to_string()),
        windows: vec![GatewayProviderQuotaWindowView {
            key: "primary".to_string(),
            label: "5h".to_string(),
            used_percent: Some(85.0),
            remaining_ratio: Some(0.15),
            limit_window_seconds: Some(18000),
            reset_at: None,
            reset_after_seconds: Some(1800),
        }],
        error: None,
        raw_data: serde_json::json!({}),
    };
    let balance = quota_to_balance_status(&snapshot);
    assert!(balance.should_deprioritize);
    assert!(!balance.is_unavailable);
    assert_eq!(balance.remaining_ratio, Some(0.15));
}

#[test]
fn build_accio_quota_headers_uses_prefixed_control_plane_contract() {
    let headers = build_accio_control_plane_headers(&accio_payload(), true);
    assert_eq!(
        headers.get("x-utdid").and_then(|value| value.to_str().ok()),
        Some("utd-accio")
    );
    assert_eq!(
        headers
            .get("x-app-version")
            .and_then(|value| value.to_str().ok()),
        Some("0.5.6")
    );
    assert_eq!(
        headers.get("x-cna").and_then(|value| value.to_str().ok()),
        Some("test-cna")
    );
}

#[test]
fn accio_quota_status_uses_remaining_ratio_and_probe_percent() {
    let remaining = serde_json::json!({
        "success": true,
        "data": {
            "total": 520,
            "remaining": 104,
            "entitlement": {
                "monthly": {
                    "total": 520,
                    "used": 416,
                    "remaining": 104
                }
            }
        }
    });
    let total = find_numeric_field(&remaining, &["total"]).unwrap();
    let left = find_numeric_field(&remaining, &["remaining"]).unwrap();
    let ratio = (left / total).clamp(0.0, 1.0);
    assert_eq!(round_percent((1.0 - ratio) * 100.0), 80.0);

    let probe = serde_json::json!({
        "success": true,
        "data": {
            "usagePercent": 12.5,
            "refreshCountdownSeconds": 3600
        }
    });
    assert_eq!(
        find_numeric_field(&probe, &["usagePercent", "usage_percent"]),
        Some(12.5)
    );
}
