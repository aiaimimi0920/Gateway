use super::*;

fn row(provider: &str, credential: Option<&str>, status: &str) -> GatewayRequestAuditView {
    serde_json::from_value(serde_json::json!({
        "id": "test-audit", "projectId": "test", "providerAccountId": provider,
        "protocolFamily": "openai", "endpointKind": "chat_completions",
        "resolvedModel": "test-model", "stream": false, "status": status,
        "clientHasCacheControl": false, "autoCacheApplied": false,
        "responseId": "test-response", "createdAt": "2026-09-27T01:00:00Z",
        "updatedAt": "2026-09-27T01:00:00Z",
        "routeTrace": { "realCredentialRef": credential }
    }))
    .unwrap()
}

#[test]
fn credential_summary_separates_accounts_and_preserves_unattributed_history() {
    let summary = summarize_request_audit_rows(&[
        row("nvidia", Some("one"), "completed"),
        row("nvidia", Some("one"), "failed"),
        row("nvidia", Some("two"), "completed"),
        row("nvidia", None, "completed"),
        row("nvidia", Some("  "), "failed"),
        row("another-provider", Some("one"), "completed"),
    ]);
    assert_eq!(summary.total_requests, 6);
    assert_eq!(summary.credentials.len(), 3);
    let one = summary
        .credentials
        .iter()
        .find(|entry| entry.credential_ref == "one" && entry.stats.provider_account_id == "nvidia")
        .unwrap();
    assert_eq!(one.stats.total_requests, 2);
    assert_eq!(one.stats.completed_count, 1);
    assert_eq!(one.stats.failed_count, 1);
    assert_eq!(one.stats.windows[0].total_requests, 2);
    assert_eq!(one.stats.models[0].total_requests, 2);
    let wire = serde_json::to_value(one).unwrap();
    assert_eq!(wire["credentialRef"], "one");
    assert_eq!(wire["providerAccountId"], "nvidia");
    assert_eq!(wire["totalRequests"], 2);
    assert_eq!(
        summary
            .provider_accounts
            .iter()
            .find(|entry| entry.provider_account_id == "nvidia")
            .unwrap()
            .total_requests,
        5
    );
}

#[test]
fn availability_uses_real_half_hour_boundaries_for_providers_credentials_and_models() {
    let rows: Vec<_> = [
        ("2026-10-25T23:00:00Z", "completed"),
        ("2026-10-25T23:29:59.999Z", "failed"),
        ("2026-10-25T23:30:00Z", "completed"),
        ("2026-10-25T23:59:59Z", "running"),
        ("2026-10-26T00:00:00Z", "cancelled"),
        ("2026-10-26T08:15:00+08:00", "completed"),
    ]
    .into_iter()
    .map(|(timestamp, status)| {
        let mut audit = row("test-provider", Some("credential"), status);
        audit.created_at = timestamp.to_string();
        audit
    })
    .collect();
    let summary = summarize_request_audit_rows(&rows);
    let provider = &summary.provider_accounts[0];
    assert_eq!(provider.total_requests, 6);
    assert_eq!(provider.windows.len(), 3);
    for windows in [
        &provider.windows,
        &provider.models[0].windows,
        &summary.credentials[0].stats.windows,
        &summary.credentials[0].stats.models[0].windows,
    ] {
        let wire = serde_json::to_value(windows).unwrap();
        assert_eq!(
            wire[0],
            serde_json::json!({
                "label": "23:00", "bucketStart": "2026-10-25T23:00:00Z",
                "totalRequests": 2, "successCount": 1, "failureCount": 1,
            })
        );
        assert_eq!(
            wire[1],
            serde_json::json!({
                "label": "23:30", "bucketStart": "2026-10-25T23:30:00Z",
                "totalRequests": 2, "successCount": 1, "failureCount": 0,
            })
        );
        assert_eq!(
            wire[2],
            serde_json::json!({
                "label": "00:00", "bucketStart": "2026-10-26T00:00:00Z",
                "totalRequests": 2, "successCount": 1, "failureCount": 0,
            })
        );
    }
}

#[test]
fn invalid_timestamps_do_not_create_fake_availability_windows_or_drop_totals() {
    for timestamp in [
        "invalid",
        "2026-99-99T23:15:00Z",
        "2026-10-26T25:00:00Z",
        "",
    ] {
        let mut audit = row("test-provider", Some("credential"), "completed");
        audit.created_at = timestamp.to_string();
        let summary = summarize_request_audit_rows(&[audit]);
        assert_eq!(summary.provider_accounts[0].total_requests, 1);
        assert!(summary.provider_accounts[0].windows.is_empty());
    }
}
