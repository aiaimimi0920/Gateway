use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use axum::body::Body;
use axum::extract::State as AxumState;
use axum::http::{HeaderMap, Method, Request, StatusCode};
use axum::routing::get;
use axum::Router;
use http_body_util::BodyExt;
use neuro_gateway::concurrency::aimd::AimdConfig;
use neuro_gateway::concurrency::registry::ConcurrencyRegistry;
use neuro_gateway::config::{Config, GatewayRuntimeRole};
use neuro_gateway::console::secrets::redact_route_document;
use neuro_gateway::console::{
    ConsoleAuthRuntime, RouteConfigRedisActivationOutcome, RouteConfigRedisBackend,
    RouteConfigRedisRevision, RouteConfigRedisStoreError, RouteConfigRuntime,
};
use neuro_gateway::credential_store::CredentialMemoryCache;
use neuro_gateway::http::router::build_router;
use neuro_gateway::routing::config::{RouteConfigStore, RouteConfigYaml};
use neuro_gateway::state::{
    AppState, GatewayLifecycleState, GatewayShutdownHandle, ProviderCredentialFolderSyncRuntime,
};
use neuro_gateway::upstream::client::UpstreamClient;
use tower::ServiceExt;

const MANAGEMENT_TOKEN: &str = "console-management-contract-token";

#[derive(Clone)]
struct ProbeServerState {
    accepted_authorizations: Vec<String>,
    status: StatusCode,
    body: String,
}

#[derive(Clone, Copy)]
enum SensitiveCommitKind {
    Raw,
    Replace,
    Clear,
}

fn document(provider_id: &str, model: &str, api_key: &str) -> RouteConfigYaml {
    serde_yaml::from_str(&format!(
        r#"
providers:
  - id: {provider_id}
    base_url: https://example.com/v1
    api_key: {api_key}
    supported_models: [{model}]
model_routes:
  - pattern: {model}
    provider_ids: [{provider_id}]
aliases:
  answer: {model}
"#
    ))
    .unwrap()
}

fn test_config() -> Config {
    Config {
        console: Default::default(),
        runtime_role: GatewayRuntimeRole::Standalone,
        port: 0,
        redis_url: "redis://127.0.0.1:1/15".to_string(),
        database_url: None,
        upstream_timeout_secs: 1,
        max_request_body_bytes: 1024 * 1024,
        max_body_chat_completions_bytes: 1024 * 1024,
        max_body_completions_bytes: 1024 * 1024,
        max_body_messages_bytes: 1024 * 1024,
        max_body_responses_bytes: 1024 * 1024,
        max_body_embeddings_bytes: 1024 * 1024,
        max_body_audio_transcriptions_bytes: 8 * 1024 * 1024,
        max_body_audio_speech_bytes: 1024 * 1024,
        max_body_search_bytes: 512 * 1024,
        max_body_fetch_bytes: 512 * 1024,
        max_body_research_bytes: 512 * 1024,
        max_body_images_generations_bytes: 1024 * 1024,
        max_body_images_edits_bytes: 1024 * 1024,
        max_body_music_bytes: 1024 * 1024,
        max_body_videos_bytes: 1024 * 1024,
        response_cache_ttl_secs: 300,
        response_cache_max_size_bytes: 512 * 1024,
        quota_pre_deduct_estimate_ratio: 1.2,
        usage_report_batch_size: 100,
        provider_probe_interval_secs: 30,
        log_level: "info".to_string(),
        gateway_api_key: Some("public-api-contract-secret".to_string()),
        gateway_api_key_secret: Some("hmac-contract-secret".to_string()),
        gateway_management_token: Some(MANAGEMENT_TOKEN.to_string()),
        gateway_keepalive_bearer_token: Some("keepalive-contract-secret".to_string()),
        default_project_id: "console-contract-project".to_string(),
        gateway_inbound_api_key_header_aliases: Vec::new(),
        provider_credential_folder_sync_enabled: false,
        provider_credential_folder_sync_root_dir: None,
        provider_credential_folder_sync_interval_secs: 30,
        provider_credential_folder_sync_watch_enabled: false,
        provider_credential_folder_sync_watch_debounce_millis: 1500,
        provider_credential_folder_sync_import_enabled: false,
        provider_credential_folder_sync_export_enabled: false,
        provider_credential_folder_sync_delete_missing: false,
        provider_credential_refresh_enabled: false,
        provider_credential_refresh_interval_secs: 3600,
        provider_credential_refresh_before_secs: 24 * 60 * 60,
        provider_credential_refresh_batch_limit: 100,
        provider_credential_refresh_lock_ttl_secs: 300,
        credential_stock_monitor_enabled: false,
        credential_stock_monitor_interval_secs: 60,
        splitter_worker_executable_path: None,
        splitter_initial_worker_port: 1,
        splitter_ready_timeout_secs: 1,
        splitter_ready_poll_interval_millis: 10,
        splitter_reload_shutdown_timeout_secs: 1,
    }
}

#[tokio::test]
async fn route_config_management_route_requires_management_authentication() {
    let fixture = ConsoleStateFixture::new(document("managed", "old-model", "live-secret"), true);
    let response = build_router(Arc::clone(&fixture.state))
        .oneshot(
            Request::get("/v1/internal/gateway/route-config")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn route_config_management_route_returns_redacted_active_document() {
    let active = document("managed", "old-model", "live-secret");
    let fixture = ConsoleStateFixture::new(active.clone(), true);
    let response = build_router(Arc::clone(&fixture.state))
        .oneshot(
            Request::get("/v1/internal/gateway/route-config")
                .header("x-management-token", MANAGEMENT_TOKEN)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body = parse_json(response).await;
    assert_eq!(
        body["routeConfig"]["revision"]["id"].as_str(),
        Some(fixture.state.route_config.snapshot().revision().id())
    );
    assert_eq!(body["routeConfig"]["source"].as_str(), Some("database"));
    assert_eq!(body["routeConfig"]["mutationSupported"], true);
    assert_ne!(
        body["routeConfig"]["document"]["providers"][0]["api_key"].as_str(),
        Some("live-secret")
    );
    assert_eq!(
        redact_route_document(&active).unwrap().secrets.len(),
        body["routeConfig"]["secrets"].as_array().unwrap().len()
    );
}

#[tokio::test]
async fn route_config_console_alias_returns_no_store_and_etag() {
    let fixture = ConsoleStateFixture::new(document("managed", "old-model", "live-secret"), true);
    let revision_id = fixture
        .state
        .route_config
        .snapshot()
        .revision()
        .id()
        .to_string();

    let response = build_router(Arc::clone(&fixture.state))
        .oneshot(
            Request::get("/v1/internal/gateway/console/route-config")
                .header("x-management-token", MANAGEMENT_TOKEN)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(
        response
            .headers()
            .get("cache-control")
            .and_then(|value| value.to_str().ok()),
        Some("no-store")
    );
    assert_eq!(
        response
            .headers()
            .get("etag")
            .and_then(|value| value.to_str().ok()),
        Some(format!("\"{revision_id}\"").as_str())
    );
}

#[tokio::test]
async fn route_config_management_commit_applies_secret_patches_and_updates_runtime() {
    let active = document("managed", "old-model", "live-secret");
    let fixture = ConsoleStateFixture::new(active.clone(), true);
    let current_revision = fixture
        .state
        .route_config
        .snapshot()
        .revision()
        .id()
        .to_string();
    let mut draft = redact_route_document(&active).unwrap().document;
    draft.providers[0].supported_models = vec!["new-model".to_string()];
    draft.model_routes[0].pattern = "new-model".to_string();
    draft
        .aliases
        .insert("answer".to_string(), "new-model".to_string());

    let response = build_router(Arc::clone(&fixture.state))
        .oneshot(
            Request::post("/v1/internal/gateway/route-config")
                .header("x-management-token", MANAGEMENT_TOKEN)
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::json!({
                        "expectedRevision": current_revision,
                        "document": draft,
                        "secretPatches": [
                            { "path": "/providers/0/api_key", "operation": "keep" }
                        ],
                        "message": "update route config"
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body = parse_json(response).await;
    assert_eq!(
        body["routeConfig"]["document"]["aliases"]["answer"].as_str(),
        Some("new-model")
    );
    assert_eq!(
        fixture
            .state
            .route_config
            .snapshot()
            .resolve_alias(Some("answer")),
        Some("new-model".to_string())
    );
    assert_eq!(
        fixture.state.route_config.snapshot().source(),
        neuro_gateway::routing::config::ActiveConfigSource::Redis
    );
}

#[tokio::test]
async fn route_config_console_validate_returns_redacted_candidate_document() {
    let active = document("managed", "old-model", "live-secret");
    let fixture = ConsoleStateFixture::new(active.clone(), true);
    let mut draft = redact_route_document(&active).unwrap().document;
    draft.providers[0].supported_models = vec!["candidate-model".to_string()];
    draft.model_routes[0].pattern = "candidate-model".to_string();
    draft
        .aliases
        .insert("answer".to_string(), "candidate-model".to_string());

    let response = build_router(Arc::clone(&fixture.state))
        .oneshot(
            Request::post("/v1/internal/gateway/console/route-config/validate")
                .header("x-management-token", MANAGEMENT_TOKEN)
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::json!({
                        "document": draft,
                        "secretPatches": [
                            { "path": "/providers/0/api_key", "operation": "keep" }
                        ]
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body = parse_json(response).await;
    assert_eq!(
        body["validation"]["document"]["aliases"]["answer"].as_str(),
        Some("candidate-model")
    );
    assert_eq!(body["validation"]["requiresRepair"], false);
    assert_ne!(
        body["validation"]["document"]["providers"][0]["api_key"].as_str(),
        Some("live-secret")
    );
}

#[tokio::test]
async fn route_config_validate_requires_exact_secret_grant_for_sensitive_inputs() {
    let active = document("managed", "old-model", "live-secret");
    let fixture = ConsoleStateFixture::new(active.clone(), true);
    let redacted = redact_route_document(&active).unwrap().document;
    let cases = [
        (
            "raw secret",
            document("managed", "old-model", "raw-request-secret"),
            serde_json::json!([]),
        ),
        (
            "replace patch",
            redacted.clone(),
            serde_json::json!([
                {
                    "path": "/providers/0/api_key",
                    "operation": "replace",
                    "value": "replacement-secret"
                }
            ]),
        ),
        (
            "clear patch",
            redacted,
            serde_json::json!([
                { "path": "/providers/0/api_key", "operation": "clear" }
            ]),
        ),
    ];

    for (label, draft, secret_patches) in cases {
        let body = serde_json::json!({
            "document": draft,
            "secretPatches": secret_patches,
        });
        for supplied_grant in [None, Some("grant-does-not-exist")] {
            let response = send_console_json(
                &fixture.state,
                Method::POST,
                "/v1/internal/gateway/console/route-config/validate",
                body.clone(),
                supplied_grant,
                None,
            )
            .await;
            assert_eq!(
                response.status(),
                StatusCode::FORBIDDEN,
                "{label} accepted a missing or unknown secret grant"
            );
            let response_body = parse_json(response).await;
            assert_eq!(
                response_body["error"]["code"], "console_secret_access_required",
                "{label} returned the wrong authorization error"
            );
        }

        let exact_grant = grant_console_secret_access(&fixture.state).await;
        let wrong_origin = send_console_json(
            &fixture.state,
            Method::POST,
            "/v1/internal/gateway/console/route-config/validate",
            body.clone(),
            Some(&exact_grant),
            Some("https://different-origin.example"),
        )
        .await;
        assert_eq!(
            wrong_origin.status(),
            StatusCode::FORBIDDEN,
            "{label} accepted a grant bound to another origin"
        );

        let exact = send_console_json(
            &fixture.state,
            Method::POST,
            "/v1/internal/gateway/console/route-config/validate",
            body,
            Some(&exact_grant),
            None,
        )
        .await;
        if label == "raw secret" {
            assert_eq!(exact.status(), StatusCode::UNPROCESSABLE_ENTITY);
            let response_body = parse_json(exact).await;
            assert_eq!(
                response_body["error"]["code"],
                "secret_value_must_use_patch"
            );
        } else {
            assert_eq!(
                exact.status(),
                StatusCode::OK,
                "{label} rejected the exact grant"
            );
        }
    }
}

#[tokio::test]
async fn route_config_commit_requires_exact_secret_grant_for_sensitive_inputs() {
    for (label, kind) in [
        ("raw secret", SensitiveCommitKind::Raw),
        ("replace patch", SensitiveCommitKind::Replace),
        ("clear patch", SensitiveCommitKind::Clear),
    ] {
        for supplied_grant in [None, Some("grant-does-not-exist")] {
            let active = document("managed", "old-model", "live-secret");
            let fixture = ConsoleStateFixture::new(active.clone(), true);
            let body = sensitive_commit_body(&fixture, &active, &kind);
            let response = send_console_json(
                &fixture.state,
                Method::PUT,
                "/v1/internal/gateway/console/route-config",
                body,
                supplied_grant,
                None,
            )
            .await;
            assert_eq!(
                response.status(),
                StatusCode::FORBIDDEN,
                "{label} accepted a missing or unknown secret grant"
            );
            let response_body = parse_json(response).await;
            assert_eq!(
                response_body["error"]["code"], "console_secret_access_required",
                "{label} returned the wrong authorization error"
            );
            assert_eq!(
                fixture.state.route_config.get_providers()[0]
                    .payload
                    .api_key,
                "live-secret",
                "{label} mutated the active secret before authorization"
            );
        }

        let active = document("managed", "old-model", "live-secret");
        let fixture = ConsoleStateFixture::new(active.clone(), true);
        let exact_grant = grant_console_secret_access(&fixture.state).await;
        let body = sensitive_commit_body(&fixture, &active, &kind);
        let exact = send_console_json(
            &fixture.state,
            Method::PUT,
            "/v1/internal/gateway/console/route-config",
            body,
            Some(&exact_grant),
            None,
        )
        .await;
        match kind {
            SensitiveCommitKind::Raw => {
                assert_eq!(exact.status(), StatusCode::UNPROCESSABLE_ENTITY);
                let response_body = parse_json(exact).await;
                assert_eq!(
                    response_body["error"]["code"],
                    "secret_value_must_use_patch"
                );
                assert_eq!(
                    fixture.state.route_config.get_providers()[0]
                        .payload
                        .api_key,
                    "live-secret"
                );
            }
            SensitiveCommitKind::Replace => {
                assert_eq!(exact.status(), StatusCode::OK);
                assert_eq!(
                    fixture.state.route_config.get_providers()[0]
                        .payload
                        .api_key,
                    "replacement-secret"
                );
            }
            SensitiveCommitKind::Clear => {
                assert_eq!(exact.status(), StatusCode::OK);
                assert!(fixture.state.route_config.get_providers()[0]
                    .payload
                    .api_key
                    .is_empty());
            }
        }
    }
}

#[tokio::test]
async fn route_config_management_commit_rejects_stale_revision_with_conflict() {
    let active = document("managed", "old-model", "live-secret");
    let fixture = ConsoleStateFixture::new(active.clone(), true);
    let mut draft = redact_route_document(&active).unwrap().document;
    draft
        .aliases
        .insert("answer".to_string(), "different-model".to_string());

    let response = build_router(Arc::clone(&fixture.state))
        .oneshot(
            Request::post("/v1/internal/gateway/route-config")
                .header("x-management-token", MANAGEMENT_TOKEN)
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::json!({
                        "expectedRevision": "r0-deadbeefcafe",
                        "document": draft,
                        "secretPatches": [
                            { "path": "/providers/0/api_key", "operation": "keep" }
                        ]
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::CONFLICT);
    let body = parse_json(response).await;
    assert_eq!(
        body["error"]["code"].as_str(),
        Some("console_revision_conflict")
    );
}

#[tokio::test]
async fn route_config_management_commit_reports_runtime_unavailable_when_not_configured() {
    let active = document("managed", "old-model", "live-secret");
    let fixture = ConsoleStateFixture::new(active.clone(), false);
    let mut draft = redact_route_document(&active).unwrap().document;
    draft.providers[0].supported_models = vec!["new-model".to_string()];
    draft.model_routes[0].pattern = "new-model".to_string();
    let response = build_router(Arc::clone(&fixture.state))
        .oneshot(
            Request::post("/v1/internal/gateway/route-config")
                .header("x-management-token", MANAGEMENT_TOKEN)
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::json!({
                        "expectedRevision": fixture.state.route_config.snapshot().revision().id(),
                        "document": draft,
                        "secretPatches": [
                            { "path": "/providers/0/api_key", "operation": "keep" }
                        ]
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
    let body = parse_json(response).await;
    assert_eq!(
        body["error"]["code"].as_str(),
        Some("console_runtime_unavailable")
    );
}

#[tokio::test]
async fn route_config_console_put_rejects_if_match_mismatch() {
    let active = document("managed", "old-model", "live-secret");
    let fixture = ConsoleStateFixture::new(active.clone(), true);
    let current_revision = fixture
        .state
        .route_config
        .snapshot()
        .revision()
        .id()
        .to_string();
    let mut draft = redact_route_document(&active).unwrap().document;
    draft
        .aliases
        .insert("answer".to_string(), "new-model".to_string());

    let response = build_router(Arc::clone(&fixture.state))
        .oneshot(
            Request::put("/v1/internal/gateway/console/route-config")
                .header("x-management-token", MANAGEMENT_TOKEN)
                .header("if-match", "\"r99-deadbeefcafe\"")
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::json!({
                        "expectedRevision": current_revision,
                        "document": draft,
                        "secretPatches": [
                            { "path": "/providers/0/api_key", "operation": "keep" }
                        ]
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    let body = parse_json(response).await;
    assert_eq!(
        body["error"]["code"].as_str(),
        Some("console_if_match_mismatch")
    );
}

#[tokio::test]
async fn route_config_management_revisions_list_returns_archived_history() {
    let fixture = ConsoleStateFixture::new(document("managed", "old-model", "live-secret"), true);
    let snapshot = fixture
        .commit_route_document(
            document("managed", "new-model", "live-secret"),
            Some("archive history"),
        )
        .await;

    let response = build_router(Arc::clone(&fixture.state))
        .oneshot(
            Request::get("/v1/internal/gateway/route-config/revisions")
                .header("x-management-token", MANAGEMENT_TOKEN)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body = parse_json(response).await;
    let revisions = body["revisions"].as_array().expect("revisions array");
    assert_eq!(revisions.len(), 1);
    assert_eq!(
        revisions[0]["revision"]["id"].as_str(),
        Some(snapshot.revision().id())
    );
    assert_eq!(revisions[0]["active"], true);
    assert_eq!(revisions[0]["hasArchive"], true);
    assert_eq!(
        revisions[0]["revision"]["message"].as_str(),
        Some("archive history")
    );
}

#[tokio::test]
async fn route_config_management_revision_detail_returns_redacted_current_snapshot() {
    let active = document("managed", "old-model", "live-secret");
    let fixture = ConsoleStateFixture::new(active.clone(), true);
    let revision_id = fixture
        .state
        .route_config
        .snapshot()
        .revision()
        .id()
        .to_string();

    let response = build_router(Arc::clone(&fixture.state))
        .oneshot(
            Request::get(format!(
                "/v1/internal/gateway/route-config/revisions/{revision_id}"
            ))
            .header("x-management-token", MANAGEMENT_TOKEN)
            .body(Body::empty())
            .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body = parse_json(response).await;
    assert_eq!(
        body["routeConfig"]["revision"]["id"].as_str(),
        Some(revision_id.as_str())
    );
    assert_eq!(body["active"], true);
    assert_eq!(body["hasArchive"], false);
    assert_ne!(
        body["routeConfig"]["document"]["providers"][0]["api_key"].as_str(),
        Some("live-secret")
    );
    assert_eq!(
        redact_route_document(&active).unwrap().secrets.len(),
        body["routeConfig"]["secrets"].as_array().unwrap().len()
    );
}

#[tokio::test]
async fn route_config_management_revision_detail_returns_not_found_for_unknown_revision() {
    let fixture = ConsoleStateFixture::new(document("managed", "old-model", "live-secret"), true);

    let response = build_router(Arc::clone(&fixture.state))
        .oneshot(
            Request::get("/v1/internal/gateway/route-config/revisions/r99-deadbeefcafe")
                .header("x-management-token", MANAGEMENT_TOKEN)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::NOT_FOUND);
    let body = parse_json(response).await;
    assert_eq!(
        body["error"]["code"].as_str(),
        Some("console_revision_not_found")
    );
}

#[tokio::test]
async fn credential_probe_requires_management_authentication_and_secret_access() {
    let fixture = ConsoleStateFixture::new(
        serde_yaml::from_str(
            r#"
providers:
  - id: probe-provider
    adapter: openai_compatible
    base_url: http://127.0.0.1:9/v1
    api_key: probe-secret
model_routes: []
aliases: {}
"#,
        )
        .unwrap(),
        false,
    );
    let endpoint = "/v1/internal/gateway/console/credentials/probe-provider::default/probe";

    let unauthenticated = build_router(Arc::clone(&fixture.state))
        .oneshot(Request::post(endpoint).body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(unauthenticated.status(), StatusCode::UNAUTHORIZED);

    let without_secret_grant = build_router(Arc::clone(&fixture.state))
        .oneshot(
            Request::post(endpoint)
                .header("x-management-token", MANAGEMENT_TOKEN)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(without_secret_grant.status(), StatusCode::FORBIDDEN);
    let body = parse_json(without_secret_grant).await;
    assert_eq!(body["error"]["code"], "console_secret_access_required");

    let secret_grant = grant_console_secret_access(&fixture.state).await;

    let without_grant_header = build_router(Arc::clone(&fixture.state))
        .oneshot(
            Request::post(endpoint)
                .header("x-management-token", MANAGEMENT_TOKEN)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(without_grant_header.status(), StatusCode::FORBIDDEN);
    let body = parse_json(without_grant_header).await;
    assert_eq!(body["error"]["code"], "console_secret_access_required");

    let wrong_grant_header = build_router(Arc::clone(&fixture.state))
        .oneshot(
            Request::post(endpoint)
                .header("x-management-token", MANAGEMENT_TOKEN)
                .header("x-secret-grant", "grant-does-not-exist")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(wrong_grant_header.status(), StatusCode::FORBIDDEN);
    let body = parse_json(wrong_grant_header).await;
    assert_eq!(body["error"]["code"], "console_secret_access_required");

    let with_exact_grant =
        probe_credential(&fixture.state, &secret_grant, "probe-provider::default").await;
    assert_eq!(with_exact_grant.status(), StatusCode::OK);
}

#[tokio::test]
async fn credential_probe_selects_explicit_and_provider_default_targets() {
    let (base_url, server) = spawn_probe_server(ProbeServerState {
        accepted_authorizations: vec![
            "Bearer explicit-secret".to_string(),
            "Bearer default-secret".to_string(),
        ],
        status: StatusCode::OK,
        body: r#"{"data":[]}"#.to_string(),
    })
    .await;
    let document: RouteConfigYaml = serde_yaml::from_str(&format!(
        r#"
providers:
  - id: pooled-provider
    adapter: openai_compatible
    base_url: "{base_url}"
    api_key: provider-unused
    credentials:
      - id: explicit-credential
        api_key: explicit-secret
  - id: default-provider
    adapter: openai_compatible
    base_url: "{base_url}"
    api_key: default-secret
model_routes: []
aliases: {{}}
"#
    ))
    .unwrap();
    let fixture = ConsoleStateFixture::new(document, false);
    let secret_grant = grant_console_secret_access(&fixture.state).await;

    let explicit = probe_credential(&fixture.state, &secret_grant, "explicit-credential").await;
    assert_eq!(explicit.status(), StatusCode::OK);
    let explicit_body = parse_json(explicit).await;
    assert_eq!(
        explicit_body["result"]["credentialId"],
        "explicit-credential"
    );
    assert_eq!(explicit_body["result"]["providerId"], "pooled-provider");
    assert_eq!(explicit_body["result"]["status"], "passed");
    assert!(explicit_body["result"]["message"].as_str().is_some());
    assert!(explicit_body["result"]["checkedAt"].as_str().is_some());

    let default =
        probe_credential(&fixture.state, &secret_grant, "default-provider::default").await;
    assert_eq!(default.status(), StatusCode::OK);
    let default_body = parse_json(default).await;
    assert_eq!(
        default_body["result"]["credentialId"],
        "default-provider::default"
    );
    assert_eq!(default_body["result"]["providerId"], "default-provider");
    assert_eq!(default_body["result"]["status"], "passed");

    let serialized = serde_json::to_string(&(explicit_body, default_body)).unwrap();
    assert!(!serialized.contains("explicit-secret"));
    assert!(!serialized.contains("default-secret"));
    server.abort();
}

#[tokio::test]
async fn credential_probe_reports_disabled_unknown_and_fixed_targets_as_unsupported() {
    let document: RouteConfigYaml = serde_yaml::from_str(
        r#"
providers:
  - id: disabled-provider
    adapter: openai_compatible
    base_url: http://127.0.0.1:9/v1
    api_key: provider-unused
    credentials:
      - id: disabled-credential
        api_key: disabled-secret
        enabled: false
  - id: unknown-provider
    adapter: gemini_web_compatible
    base_url: http://127.0.0.1:9/v1
    api_key: unknown-secret
  - id: browser-provider
    adapter: openai_compatible
    base_url: http://127.0.0.1:9/v1
    api_key: browser-secret
    execution_mode: browser_backed
  - id: fixed-provider
    preset: codex
    base_url: https://chatgpt.com/backend-api/codex
    api_key: fixed-secret
model_routes: []
aliases: {}
"#,
    )
    .unwrap();
    let fixture = ConsoleStateFixture::new(document, false);
    let secret_grant = grant_console_secret_access(&fixture.state).await;

    for (credential_id, expected_fragment) in [
        ("disabled-credential", "disabled"),
        ("unknown-provider::default", "gemini_web_compatible"),
        ("browser-provider::default", "browser-backed"),
        ("fixed-provider::default", "fixed-model"),
    ] {
        let response = probe_credential(&fixture.state, &secret_grant, credential_id).await;
        assert_eq!(response.status(), StatusCode::OK);
        let body = parse_json(response).await;
        assert_eq!(body["result"]["credentialId"], credential_id);
        assert_eq!(body["result"]["status"], "unsupported");
        assert!(body["result"]["message"]
            .as_str()
            .is_some_and(|message| message.contains(expected_fragment)));
    }
}

#[tokio::test]
async fn credential_probe_sanitizes_failed_probe_response() {
    let api_key = "sk-probe-api-secret";
    let cookie_secret = "cookie-probe-secret";
    let body_secret = "body-probe-secret";
    let (base_url, server) = spawn_probe_server(ProbeServerState {
        accepted_authorizations: vec![format!("Bearer {api_key}")],
        status: StatusCode::INTERNAL_SERVER_ERROR,
        body: format!(
            "Authorization: Bearer {api_key}; Cookie: session={cookie_secret}; body={body_secret}"
        ),
    })
    .await;
    let document: RouteConfigYaml = serde_yaml::from_str(&format!(
        r#"
providers:
  - id: failing-provider
    adapter: openai_compatible
    base_url: "{base_url}"
    api_key: "{api_key}"
    headers:
      Cookie: "session={cookie_secret}"
model_routes: []
aliases: {{}}
"#
    ))
    .unwrap();
    let fixture = ConsoleStateFixture::new(document, false);
    let secret_grant = grant_console_secret_access(&fixture.state).await;

    let response =
        probe_credential(&fixture.state, &secret_grant, "failing-provider::default").await;
    assert_eq!(response.status(), StatusCode::OK);
    let body = parse_json(response).await;
    assert_eq!(body["result"]["status"], "failed");
    assert!(body["result"]["checkedAt"].as_str().is_some());
    let serialized = serde_json::to_string(&body).unwrap();
    for secret in [api_key, cookie_secret, body_secret] {
        assert!(
            !serialized.contains(secret),
            "probe response leaked {secret}"
        );
    }
    server.abort();
}

#[tokio::test]
async fn credential_probe_returns_not_found_for_unknown_global_id() {
    let fixture = ConsoleStateFixture::new(document("managed", "gpt-5.4", "live-secret"), false);
    let secret_grant = grant_console_secret_access(&fixture.state).await;

    let response = probe_credential(&fixture.state, &secret_grant, "missing-credential").await;
    assert_eq!(response.status(), StatusCode::NOT_FOUND);
    let body = parse_json(response).await;
    assert_eq!(body["error"]["code"], "console_credential_not_found");
}

async fn grant_console_secret_access(state: &Arc<AppState>) -> String {
    let response = build_router(Arc::clone(state))
        .oneshot(
            Request::post("/v1/internal/gateway/console/session/confirm-secret-access")
                .header("x-management-token", MANAGEMENT_TOKEN)
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::json!({ "token": MANAGEMENT_TOKEN }).to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let body = parse_json(response).await;
    body["grant"]
        .as_str()
        .expect("secret access response must include a grant")
        .to_string()
}

fn sensitive_commit_body(
    fixture: &ConsoleStateFixture,
    active: &RouteConfigYaml,
    kind: &SensitiveCommitKind,
) -> serde_json::Value {
    let (draft, secret_patches) = match kind {
        SensitiveCommitKind::Raw => (
            document("managed", "old-model", "raw-request-secret"),
            serde_json::json!([]),
        ),
        SensitiveCommitKind::Replace => (
            redact_route_document(active).unwrap().document,
            serde_json::json!([
                {
                    "path": "/providers/0/api_key",
                    "operation": "replace",
                    "value": "replacement-secret"
                }
            ]),
        ),
        SensitiveCommitKind::Clear => (
            redact_route_document(active).unwrap().document,
            serde_json::json!([
                { "path": "/providers/0/api_key", "operation": "clear" }
            ]),
        ),
    };
    serde_json::json!({
        "expectedRevision": fixture.state.route_config.snapshot().revision().id(),
        "document": draft,
        "secretPatches": secret_patches,
    })
}

async fn send_console_json(
    state: &Arc<AppState>,
    method: Method,
    endpoint: &str,
    body: serde_json::Value,
    secret_grant: Option<&str>,
    origin: Option<&str>,
) -> axum::response::Response {
    let mut request = Request::builder()
        .method(method)
        .uri(endpoint)
        .header("x-management-token", MANAGEMENT_TOKEN)
        .header("content-type", "application/json");
    if let Some(secret_grant) = secret_grant {
        request = request.header("x-secret-grant", secret_grant);
    }
    if let Some(origin) = origin {
        request = request.header("origin", origin);
    }
    build_router(Arc::clone(state))
        .oneshot(request.body(Body::from(body.to_string())).unwrap())
        .await
        .unwrap()
}

async fn probe_credential(
    state: &Arc<AppState>,
    secret_grant: &str,
    credential_id: &str,
) -> axum::response::Response {
    build_router(Arc::clone(state))
        .oneshot(
            Request::post(format!(
                "/v1/internal/gateway/console/credentials/{credential_id}/probe"
            ))
            .header("x-management-token", MANAGEMENT_TOKEN)
            .header("x-secret-grant", secret_grant)
            .body(Body::empty())
            .unwrap(),
        )
        .await
        .unwrap()
}

async fn spawn_probe_server(state: ProbeServerState) -> (String, tokio::task::JoinHandle<()>) {
    async fn models(
        AxumState(state): AxumState<ProbeServerState>,
        headers: HeaderMap,
    ) -> (StatusCode, String) {
        let authorization = headers
            .get("authorization")
            .and_then(|value| value.to_str().ok())
            .unwrap_or_default();
        if !state
            .accepted_authorizations
            .iter()
            .any(|expected| expected == authorization)
        {
            return (
                StatusCode::UNAUTHORIZED,
                "unexpected authorization".to_string(),
            );
        }
        (state.status, state.body)
    }

    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind credential probe fixture");
    let address = listener
        .local_addr()
        .expect("credential probe fixture address");
    let server = tokio::spawn(async move {
        axum::serve(
            listener,
            Router::new()
                .route("/v1/models", get(models))
                .with_state(state),
        )
        .await
        .expect("serve credential probe fixture");
    });
    (format!("http://{address}/v1"), server)
}

async fn parse_json(response: axum::response::Response) -> serde_json::Value {
    let bytes = response
        .into_body()
        .collect()
        .await
        .expect("collect response body")
        .to_bytes();
    serde_json::from_slice(&bytes).expect("response must be JSON")
}

struct ConsoleStateFixture {
    _temp: TestDirectory,
    state: Arc<AppState>,
}

impl ConsoleStateFixture {
    fn new(active: RouteConfigYaml, with_runtime: bool) -> Self {
        let temp = TestDirectory::new("console-management");
        let routes = temp.path().join("routes.yaml");
        fs::write(
            &routes,
            neuro_gateway::console::document::canonicalize_route_document(&active)
                .unwrap()
                .canonical_yaml(),
        )
        .unwrap();
        let store = Arc::new(RouteConfigStore::from_document(active).unwrap());
        let runtime = with_runtime.then(|| {
            let persistence =
                neuro_gateway::console::RouteConfigPersistence::for_test(temp.path(), &routes)
                    .unwrap();
            Arc::new(RouteConfigRuntime::with_backend(
                Arc::clone(&store),
                persistence,
                Arc::new(FakeRedisBackend::default()),
                true,
            ))
        });
        Self {
            state: build_state(Arc::clone(&store), runtime),
            _temp: temp,
        }
    }

    async fn commit_route_document(
        &self,
        document: RouteConfigYaml,
        message: Option<&str>,
    ) -> Arc<neuro_gateway::routing::config::RouteConfigSnapshot> {
        let runtime = self
            .state
            .route_config_runtime
            .as_ref()
            .expect("runtime must be configured");
        let current_revision = self
            .state
            .route_config
            .snapshot()
            .revision()
            .id()
            .to_string();
        runtime
            .commit_document(&current_revision, document, message.map(str::to_string))
            .await
            .expect("commit route document")
    }
}

fn build_state(
    route_config: Arc<RouteConfigStore>,
    route_config_runtime: Option<Arc<RouteConfigRuntime>>,
) -> Arc<AppState> {
    let config = test_config();
    let redis_pool = deadpool_redis::Config::from_url(config.redis_url.clone())
        .create_pool(Some(deadpool_redis::Runtime::Tokio1))
        .expect("create lazy Redis test pool");
    let upstream_timeout_secs = config.upstream_timeout_secs;
    let provider_credential_folder_sync_enabled = config.provider_credential_folder_sync_enabled;
    let console_auth = test_console_auth_runtime(config.gateway_management_token.clone());

    Arc::new(AppState {
        config,
        redis_pool,
        pg_pool: None,
        upstream_client: UpstreamClient::new(upstream_timeout_secs),
        concurrency_registry: ConcurrencyRegistry::new(AimdConfig::default()),
        auth_adapters: Vec::new(),
        filter_config: None,
        route_config,
        route_config_runtime,
        console_auth,
        credential_cache: CredentialMemoryCache::new(30),
        lifecycle: GatewayLifecycleState::default(),
        shutdown: GatewayShutdownHandle::default(),
        provider_credential_folder_sync: ProviderCredentialFolderSyncRuntime::new(
            provider_credential_folder_sync_enabled,
        ),
    })
}

fn test_console_auth_runtime(env_management_token: Option<String>) -> Arc<ConsoleAuthRuntime> {
    let temp = std::env::temp_dir().join(format!(
        "gateway-console-auth-management-{}",
        uuid::Uuid::new_v4()
    ));
    let console = neuro_gateway::console::ConsoleConfig::from_values(
        neuro_gateway::console::ConsoleConfigValues {
            state_dir: Some(temp.clone()),
            routes_file: Some(temp.join("routes.yaml")),
            ..Default::default()
        },
    )
    .unwrap();
    Arc::new(ConsoleAuthRuntime::new(&console, env_management_token).unwrap())
}

#[derive(Clone, Debug, Default)]
struct FakeRedisBackend {
    state: Arc<Mutex<FakeRedisState>>,
}

#[async_trait]
impl RouteConfigRedisBackend for FakeRedisBackend {
    async fn store_revision(
        &self,
        revision: &RouteConfigRedisRevision,
    ) -> Result<(), RouteConfigRedisStoreError> {
        self.state
            .lock()
            .unwrap()
            .revisions
            .insert(revision.metadata().id().to_string(), revision.clone());
        Ok(())
    }

    async fn store_prepared_transaction(
        &self,
        _record: &neuro_gateway::console::TransactionRecord,
    ) -> Result<(), RouteConfigRedisStoreError> {
        Ok(())
    }

    async fn activate_revision(
        &self,
        _expected_active_revision: Option<&str>,
        revision: &RouteConfigRedisRevision,
        _prepared_record: &neuro_gateway::console::TransactionRecord,
        _activated_record: &neuro_gateway::console::TransactionRecord,
    ) -> Result<RouteConfigRedisActivationOutcome, RouteConfigRedisStoreError> {
        self.state.lock().unwrap().active_revision = Some(revision.clone());
        Ok(RouteConfigRedisActivationOutcome::Activated)
    }

    async fn load_active_revision(
        &self,
    ) -> Result<Option<RouteConfigRedisRevision>, RouteConfigRedisStoreError> {
        Ok(self.state.lock().unwrap().active_revision.clone())
    }
}

#[derive(Clone, Debug, Default)]
struct FakeRedisState {
    revisions: BTreeMap<String, RouteConfigRedisRevision>,
    active_revision: Option<RouteConfigRedisRevision>,
}

struct TestDirectory {
    path: PathBuf,
}

impl TestDirectory {
    fn new(label: &str) -> Self {
        let path = std::env::temp_dir().join(format!("gateway-{label}-{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&path).unwrap();
        Self { path }
    }

    fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for TestDirectory {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
    }
}
