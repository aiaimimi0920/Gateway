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
