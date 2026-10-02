use super::*;
use axum::routing::post;
use axum::{Json, Router};

#[test]
fn registry_document_accepts_the_documented_tagged_transport_and_rejects_unknown_fields() {
    let documented: DriverRegistryDocument = serde_json::from_value(serde_json::json!({
        "drivers": [
            {
                "id": "provider-a-driver",
                "provider_ids": ["provider-a"],
                "timeout_secs": 30,
                "mode": "script",
                "script": "provider-a.mjs"
            }
        ]
    }))
    .unwrap();
    assert!(matches!(
        documented.drivers[0].transport,
        CredentialAutomationDriverTransport::Script { .. }
    ));

    let unknown = serde_json::from_value::<DriverRegistryDocument>(serde_json::json!({
        "drivers": [
            {
                "id": "provider-a-driver",
                "provider_ids": ["provider-a"],
                "mode": "script",
                "script": "provider-a.mjs",
                "command": "not-allowlisted"
            }
        ]
    }));
    assert!(unknown.is_err());
}

#[test]
fn registry_rejects_non_https_non_loopback_http() {
    let config = CredentialPoolAutomationConfig::default();
    let driver = CredentialAutomationDriver {
        id: "unsafe-http".to_string(),
        provider_ids: vec!["provider-a".to_string()],
        timeout_secs: None,
        transport: CredentialAutomationDriverTransport::Http {
            endpoint: "http://example.com/refill".to_string(),
            secret_env: None,
        },
    };

    assert!(validate_driver(&config, &driver).is_err());
}

#[test]
fn registry_rejects_plaintext_localhost_domains() {
    let config = CredentialPoolAutomationConfig::default();
    let driver = CredentialAutomationDriver {
        id: "unsafe-localhost-domain".to_string(),
        provider_ids: vec!["provider-a".to_string()],
        timeout_secs: None,
        transport: CredentialAutomationDriverTransport::Http {
            endpoint: "http://localhost:9911/refill".to_string(),
            secret_env: None,
        },
    };

    assert!(validate_driver(&config, &driver).is_err());
}

#[test]
fn registry_resolves_only_allowlisted_provider() {
    let driver = CredentialAutomationDriver {
        id: "provider-a-driver".to_string(),
        provider_ids: vec!["provider-a".to_string()],
        timeout_secs: None,
        transport: CredentialAutomationDriverTransport::Http {
            endpoint: "http://127.0.0.1:9911/refill".to_string(),
            secret_env: None,
        },
    };
    let registry = DriverRegistry {
        drivers: BTreeMap::from([(driver.id.clone(), driver)]),
    };
    let mut provider: ProviderConfigYaml = serde_json::from_value(serde_json::json!({
        "id": "provider-a",
        "label": "Provider A",
        "base_url": "https://api.example.com",
        "credentials": []
    }))
    .unwrap();

    assert_eq!(
        registry.resolve(&provider).unwrap().unwrap().id,
        "provider-a-driver"
    );
    provider.id = "provider-b".to_string();
    assert!(registry.resolve(&provider).unwrap().is_none());
}

#[test]
fn driver_response_accepts_only_permanent_prune_classifications() {
    let quota_response = br#"{
        "credentials": [],
        "prune": [{"credential_id":"account-a","classification":"quota_exhausted"}]
    }"#;
    let permanent_response = br#"{
        "credentials": [],
        "prune": [{"credential_id":"account-a","classification":"permanent_auth_failure"}]
    }"#;

    assert!(serde_json::from_slice::<DriverResponse>(quota_response).is_err());
    assert!(serde_json::from_slice::<DriverResponse>(permanent_response).is_ok());
}

#[test]
fn prune_removes_legacy_credential_by_its_synthetic_id() {
    let mut provider: ProviderConfigYaml = serde_json::from_value(serde_json::json!({
        "id": "provider-a",
        "label": "Provider A",
        "base_url": "https://api.example.com",
        "credentials": [{"account_name": "Legacy account"}]
    }))
    .unwrap();
    let prune_ids = HashSet::from(["provider-a-cred-0".to_string()]);

    assert_eq!(prune_credentials(&mut provider, &prune_ids), 1);
    assert!(provider.credentials.is_empty());
}

#[test]
fn archive_records_pruned_credentials_and_purge_removes_only_archive_json() {
    let root = std::env::temp_dir().join(format!(
        "gateway-credential-archive-test-{}",
        Uuid::new_v4()
    ));
    let directory = root.join("provider-a");
    let provider: ProviderConfigYaml = serde_json::from_value(serde_json::json!({
        "id": "provider-a",
        "label": "Provider A",
        "base_url": "https://api.example.com",
        "credentials": [
            {"id": "account-a", "account_name": "Account A", "api_key": "secret-a"},
            {"id": "account-b", "account_name": "Account B", "api_key": "secret-b"}
        ]
    }))
    .unwrap();
    let prune_ids = HashSet::from(["account-a".to_string()]);

    assert_eq!(
        archive_pruned_credentials_in_directory(&provider, &prune_ids, &directory).unwrap(),
        1
    );
    assert_eq!(
        archived_credential_count_in_directory(&directory, "provider-a"),
        1
    );
    let archive_path = fs::read_dir(&directory)
        .unwrap()
        .next()
        .unwrap()
        .unwrap()
        .path();
    let record: serde_json::Value =
        serde_json::from_slice(&fs::read(&archive_path).unwrap()).unwrap();
    assert_eq!(record["schemaVersion"], 1);
    assert_eq!(record["providerId"], "provider-a");
    assert_eq!(record["credentialId"], "account-a");
    assert_eq!(record["reason"], "permanent_driver_rejection");
    assert_eq!(record["credential"]["account_name"], "Account A");

    assert_eq!(
        purge_credential_archive_directory(&directory, "provider-a").unwrap(),
        1
    );
    assert!(directory.exists());
    let _ = fs::remove_dir_all(root);
}

#[test]
fn permanent_delete_flag_defaults_off_and_accepts_explicit_opt_in() {
    let default_provider: ProviderConfigYaml = serde_json::from_value(serde_json::json!({
        "id": "provider-a",
        "base_url": "https://api.example.com"
    }))
    .unwrap();
    let permanent_provider: ProviderConfigYaml = serde_json::from_value(serde_json::json!({
        "id": "provider-b",
        "base_url": "https://api.example.com",
        "credential_permanent_delete_enabled": true
    }))
    .unwrap();

    assert!(!default_provider.credential_permanent_delete_enabled);
    assert!(permanent_provider.credential_permanent_delete_enabled);
    assert_eq!(safe_archive_path_segment("../provider/a"), ".._provider_a");
}

#[test]
fn identity_categories_cannot_override_the_provider_maximum() {
    let provider: ProviderConfigYaml = serde_json::from_value(serde_json::json!({
        "id": "codex",
        "label": "Codex",
        "base_url": "https://chatgpt.com/backend-api",
        "auto_refill_enabled": true,
        "credential_identity_categories": [
            {
                "id": "plus",
                "label": "Plus",
                "pool_target_size": 3,
                "auto_refill_enabled": true
            },
            {
                "id": "free",
                "label": "Free",
                "pool_target_size": 10,
                "auto_refill_enabled": false
            }
        ],
        "credentials": [
            {
                "id": "codex-plus-1",
                "credential_identity_category_id": "plus"
            },
            {
                "id": "codex-free-1",
                "credential_identity_category_id": "free"
            }
        ]
    }))
    .unwrap();

    let mut provider = provider;
    provider.pool_target_size = Some(2);
    assert_eq!(capacity::remaining_capacity(&provider), 0);
}

#[tokio::test]
async fn http_driver_receives_fixed_context_and_returns_credential_drafts() {
    let app = Router::new().route(
        "/reconcile",
        post(|Json(request): Json<serde_json::Value>| async move {
            assert_eq!(request["action"], "reconcile");
            assert_eq!(request["provider"]["id"], "provider-a");
            assert_eq!(request["provider"]["requestedCount"], 1);
            Json(serde_json::json!({
                "credentials": [{
                    "id": "provider-a-account-2",
                    "account_name": "Account 2",
                    "api_key": "driver-secret"
                }],
                "prune": [],
                "message": "created one account"
            }))
        }),
    );
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let server = tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });
    let driver = CredentialAutomationDriver {
        id: "provider-a-driver".to_string(),
        provider_ids: vec!["provider-a".to_string()],
        timeout_secs: Some(5),
        transport: CredentialAutomationDriverTransport::Http {
            endpoint: format!("http://{address}/reconcile"),
            secret_env: None,
        },
    };
    let request = DriverRequest {
        run_id: "test-run".to_string(),
        action: "reconcile",
        provider: DriverProviderContext {
            id: "provider-a".to_string(),
            label: "Provider A".to_string(),
            target_size: 2,
            credential_count: 1,
            active_credential_count: 1,
            requested_count: 1,
            auto_refill_enabled: true,
            auto_prune_enabled: false,
            identity_categories: vec![],
            credentials: vec![DriverCredentialContext {
                id: "provider-a-account-1".to_string(),
                account_name: Some("Account 1".to_string()),
                enabled: true,
                identity_category_id: None,
            }],
        },
        refill: None,
    };

    let response = execute_driver(
        &CredentialPoolAutomationConfig::default(),
        &driver,
        &request,
    )
    .await
    .unwrap();

    assert_eq!(response.credentials.len(), 1);
    assert_eq!(
        response.credentials[0].id.as_deref(),
        Some("provider-a-account-2")
    );
    server.abort();
}
