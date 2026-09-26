use super::*;

fn provider(value: serde_json::Value) -> ProviderConfigYaml {
    serde_json::from_value(value).unwrap()
}

#[test]
fn demand_combines_provider_and_identity_category_deficits() {
    let provider = provider(serde_json::json!({
        "id": "codex",
        "base_url": "https://example.invalid",
        "pool_target_size": 2,
        "credentials": [
            {"id": "plus-1", "credential_identity_category_id": "plus"},
            {"id": "free-1", "credential_identity_category_id": "free"}
        ],
        "credential_identity_categories": [
            {"id": "plus", "pool_target_size": 4, "auto_refill_enabled": true},
            {"id": "free", "pool_target_size": 9, "auto_refill_enabled": false}
        ]
    }));

    assert_eq!(active_credential_count(&provider), 2);
    assert_eq!(identity_category_deficit(&provider), 3);
}

#[test]
fn task_view_never_serializes_the_claim_token_or_idempotency_key() {
    let record = CredentialRefillTaskRecord {
        id: "task-1".to_string(),
        provider_id: "provider-a".to_string(),
        provider_label: "Provider A".to_string(),
        trigger: CredentialRefillTrigger::Inquiry,
        state: CredentialRefillTaskState::Claimed,
        requested_count: 1,
        target_size: 2,
        active_credential_count: 1,
        route_revision: "r1".to_string(),
        created_at: "now".to_string(),
        updated_at: "now".to_string(),
        worker_id: Some("worker-a".to_string()),
        claim_token: Some("claim-secret".to_string()),
        lease_until: Some("later".to_string()),
        attempt: 1,
        delivery_mode: None,
        created_count: 0,
        message: None,
        revision_id: None,
        idempotency_key: Some("caller-secret".to_string()),
    };

    let serialized = serde_json::to_string(&CredentialRefillTaskView::from(&record)).unwrap();
    assert!(!serialized.contains("claim-secret"));
    assert!(!serialized.contains("caller-secret"));
    assert!(!serialized.contains("claimToken"));
    assert!(!serialized.contains("idempotencyKey"));
}

#[test]
fn direct_delivery_is_idempotent_by_credential_id() {
    let mut provider = provider(serde_json::json!({
        "id": "provider-a",
        "base_url": "https://example.invalid",
        "credentials": [{"id": "account-a", "api_key": "existing"}]
    }));
    let incoming: Vec<ProviderCredentialYaml> = serde_json::from_value(serde_json::json!([
        {"id": "account-a", "api_key": "retry"},
        {"id": "account-b", "api_key": "new"}
    ]))
    .unwrap();

    assert_eq!(
        append_refill_credentials(&mut provider, incoming).unwrap(),
        1
    );
    assert_eq!(provider.credentials.len(), 2);
}

#[test]
fn folder_delivery_rejects_parent_directory_escape() {
    let error = validate_folder_paths(&["../outside.json".to_string()]).unwrap_err();
    assert_eq!(error.http_status, Some(400));
}

#[test]
fn caller_idempotency_keys_are_scoped_to_the_provider() {
    let provider_a = hash_idempotency_key("provider-a", "request-1");
    let provider_b = hash_idempotency_key("provider-b", "request-1");

    assert_ne!(provider_a, provider_b);
    assert_eq!(provider_a, hash_idempotency_key("provider-a", "request-1"));
}

#[test]
fn provider_specific_api_path_encodes_the_provider_as_one_segment() {
    assert_eq!(
        provider_path_segment("longcat:longcat-live"),
        "longcat%3Alongcat-live"
    );
    assert_eq!(provider_path_segment("provider/child"), "provider%2Fchild");
}

#[test]
fn delivery_inputs_follow_the_documented_camel_case_contract() {
    let folder: CredentialRefillDeliveryInput = serde_json::from_value(serde_json::json!({
        "mode": "folder_sync",
        "relativePaths": ["provider/account.json"]
    }))
    .unwrap();
    assert!(matches!(
        folder,
        CredentialRefillDeliveryInput::FolderSync { relative_paths }
            if relative_paths == ["provider/account.json"]
    ));

    let pull: CredentialRefillDeliveryInput = serde_json::from_value(serde_json::json!({
        "mode": "gateway_pull",
        "artifactReference": "artifact-1"
    }))
    .unwrap();
    assert!(matches!(
        pull,
        CredentialRefillDeliveryInput::GatewayPull { artifact_reference }
            if artifact_reference == "artifact-1"
    ));

    let direct: CredentialRefillDeliveryInput = serde_json::from_value(serde_json::json!({
        "mode": "direct_callback",
        "credentials": [{"id": "account-1", "api_key": "secret"}]
    }))
    .unwrap();
    assert!(matches!(
        direct,
        CredentialRefillDeliveryInput::DirectCallback { credentials }
            if credentials.len() == 1
    ));
}
