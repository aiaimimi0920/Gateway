use neuro_gateway::conversation_dataset::{
    build_clean_dataset_row, can_publish_dataset, clean_training_json_value,
    deterministic_sample_indices,
};
use neuro_gateway::db::GatewayConversationArchiveView;
use neuro_gateway::provider_failure::{classify_provider_failure, ProviderFailureClass};
use neuro_gateway::provider_health_state::{
    transition_for_failure, transition_for_success, CredentialModelHealthStatus,
};
use neuro_gateway::redis::usage_tracking::UsageReport;
use neuro_gateway::usage_aggregation::aggregate_usage_reports_by_user_credential_model;
use serde_json::json;

#[test]
fn credential_model_health_blocks_permanent_credential_failures() {
    let classification =
        classify_provider_failure(Some(401), Some("invalid_api_key"), Some("invalid API key"));

    let transition = transition_for_failure(&classification);

    assert_eq!(
        classification.class,
        ProviderFailureClass::CredentialInvalid
    );
    assert_eq!(transition.status, CredentialModelHealthStatus::Blocked);
    assert!(transition.permanent);
    assert!(transition.penalizes_candidate);
    assert_eq!(transition.cooldown_seconds, None);
}

#[test]
fn credential_model_health_cools_rate_limits_but_does_not_permanently_block() {
    let classification =
        classify_provider_failure(Some(429), Some("rate_limit"), Some("too many requests"));

    let transition = transition_for_failure(&classification);

    assert_eq!(transition.status, CredentialModelHealthStatus::Cooling);
    assert!(!transition.permanent);
    assert!(transition.penalizes_candidate);
    assert!(transition.cooldown_seconds.unwrap_or_default() >= 60);
}

#[test]
fn credential_model_health_ignores_client_request_failures() {
    let classification = classify_provider_failure(
        Some(400),
        Some("bad_request"),
        Some("missing required field"),
    );

    let transition = transition_for_failure(&classification);

    assert_eq!(transition.status, CredentialModelHealthStatus::Active);
    assert!(!transition.penalizes_candidate);
    assert_eq!(transition.cooldown_seconds, None);
}

#[test]
fn credential_model_health_success_resets_to_active() {
    let transition = transition_for_success();

    assert_eq!(transition.status, CredentialModelHealthStatus::Active);
    assert!(!transition.permanent);
    assert!(!transition.penalizes_candidate);
    assert_eq!(transition.cooldown_seconds, None);
}

#[test]
fn usage_aggregation_groups_by_user_credential_and_model() {
    let reports = vec![
        usage_report("req-1", "user-a", "cred-a", "gpt-4o", 10, 3, true),
        usage_report("req-2", "user-a", "cred-a", "gpt-4o", 20, 7, false),
        usage_report("req-3", "user-a", "cred-b", "gpt-4o", 5, 1, true),
    ];

    let mut buckets = aggregate_usage_reports_by_user_credential_model(&reports, 3600);
    buckets.sort_by(|a, b| a.provider_credential_ref.cmp(&b.provider_credential_ref));

    assert_eq!(buckets.len(), 2);
    assert_eq!(buckets[0].user_id, "user-a");
    assert_eq!(buckets[0].provider_credential_ref, "cred-a");
    assert_eq!(buckets[0].model, "gpt-4o");
    assert_eq!(buckets[0].request_count, 2);
    assert_eq!(buckets[0].failure_count, 1);
    assert_eq!(buckets[0].prompt_tokens, 30);
    assert_eq!(buckets[0].completion_tokens, 10);
    assert_eq!(buckets[0].total_tokens, 40);

    assert_eq!(buckets[1].provider_credential_ref, "cred-b");
    assert_eq!(buckets[1].request_count, 1);
}

#[test]
fn dataset_cleaning_redacts_secret_like_values_recursively() {
    let raw = json!({
        "authorization": "Bearer sk-secret",
        "nested": {
            "apiKey": "sk-another",
            "messages": [
                {"role": "user", "content": "hello"},
                {"cookie": "session=secret"}
            ]
        }
    });

    let cleaned = clean_training_json_value(&raw);

    assert_eq!(cleaned["authorization"], "[redacted]");
    assert_eq!(cleaned["nested"]["apiKey"], "[redacted]");
    assert_eq!(cleaned["nested"]["messages"][0]["content"], "hello");
    assert_eq!(cleaned["nested"]["messages"][1]["cookie"], "[redacted]");
}

#[test]
fn dataset_sampling_is_stable_and_bounded() {
    let indices = deterministic_sample_indices(10, 4);

    assert_eq!(indices, vec![0, 3, 6, 9]);
    assert_eq!(deterministic_sample_indices(3, 10), vec![0, 1, 2]);
    assert!(deterministic_sample_indices(0, 10).is_empty());
}

#[test]
fn dataset_publish_requires_explicit_approval() {
    assert!(!can_publish_dataset("draft"));
    assert!(!can_publish_dataset("review_pending"));
    assert!(!can_publish_dataset("rejected"));
    assert!(can_publish_dataset("approved"));
}

#[test]
fn clean_dataset_row_keeps_provider_metadata_and_redacts_artifacts() {
    let archive = archive_view();
    let row = build_clean_dataset_row(
        &archive,
        Some(
            json!({"headers":{"authorization":"Bearer token"},"messages":[{"role":"user","content":"法国的首都在哪里"}]}),
        ),
        Some(json!({"result":{"choices":[{"message":{"content":"巴黎"}}]},"set-cookie":"secret"})),
    );

    assert_eq!(row["archiveId"], "archive-1");
    assert_eq!(row["providerAccountId"], "provider-a");
    assert_eq!(row["resolvedModel"], "gpt-4o");
    assert_eq!(row["request"]["headers"]["authorization"], "[redacted]");
    assert_eq!(row["response"]["set-cookie"], "[redacted]");
}

fn usage_report(
    request_id: &str,
    user_id: &str,
    credential_id: &str,
    model: &str,
    prompt_tokens: u64,
    completion_tokens: u64,
    success: bool,
) -> UsageReport {
    UsageReport {
        request_id: request_id.to_string(),
        credential_id: credential_id.to_string(),
        project_id: "project-a".to_string(),
        user_id: user_id.to_string(),
        model: model.to_string(),
        provider: "openai".to_string(),
        prompt_tokens,
        completion_tokens,
        total_tokens: prompt_tokens + completion_tokens,
        cache_creation_input_tokens: None,
        cache_read_input_tokens: None,
        request_started_at: "2026-05-30T00:00:00Z".to_string(),
        request_completed_at: "2026-05-30T00:00:01Z".to_string(),
        latency_ms: 100,
        success,
        error_code: (!success).then(|| "rate_limit".to_string()),
    }
}

fn archive_view() -> GatewayConversationArchiveView {
    GatewayConversationArchiveView {
        id: "archive-1".to_string(),
        request_audit_id: Some("audit-1".to_string()),
        request_id: "request-1".to_string(),
        project_id: Some("project-a".to_string()),
        user_id: Some("user-a".to_string()),
        session_id: Some("session-a".to_string()),
        provider_account_id: Some("provider-a".to_string()),
        provider_credential_ref: Some("cred-a".to_string()),
        protocol_family: "openai_chat".to_string(),
        protocol_profile: Some("chatgpt_official_api".to_string()),
        endpoint_kind: "chat_completions".to_string(),
        requested_model: Some("gpt-4o".to_string()),
        resolved_model: Some("gpt-4o".to_string()),
        status: "completed".to_string(),
        upstream_status: Some(200),
        failure_class: None,
        failure_scope: None,
        request_object_key: Some("request.json".to_string()),
        response_object_key: Some("response.json".to_string()),
        redaction_version: "conversation-archive-redaction-v1".to_string(),
        truncated_request: false,
        truncated_response: false,
        archive_error: None,
        retention_expires_at: None,
        created_at: "2026-05-30T00:00:00Z".to_string(),
        updated_at: "2026-05-30T00:00:00Z".to_string(),
    }
}
