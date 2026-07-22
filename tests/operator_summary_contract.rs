use std::sync::Arc;

use axum::body::Body;
use axum::http::{Request, StatusCode};
use http_body_util::BodyExt;
use neuro_gateway::config::{Config, GatewayRuntimeRole};
use neuro_gateway::http::router::build_router;
use neuro_gateway::http::routes::internal_runtime::{
    bounded_readiness_probe, optional_postgresql_readiness,
};
use neuro_gateway::routing::config::RouteConfigStore;
use neuro_gateway::state::AppState;
use serde_json::Value;
use std::time::{Duration, Instant};
use tower::ServiceExt;

mod support;
use support::build_test_app_state;

const MANAGEMENT_TOKEN: &str = "operator-summary-contract-token";

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
        default_project_id: "operator-summary-project".to_string(),
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

fn test_state() -> Arc<AppState> {
    test_state_with_redis_url("redis://127.0.0.1:1/15")
}

fn test_state_with_redis_url(redis_url: &str) -> Arc<AppState> {
    let mut config = test_config();
    config.redis_url = redis_url.to_string();
    build_test_app_state(config, RouteConfigStore::new(), None)
}

fn test_app() -> axum::Router {
    build_router(test_state())
}

async fn parse_json(response: axum::response::Response) -> Value {
    let bytes = response
        .into_body()
        .collect()
        .await
        .expect("collect response body")
        .to_bytes();
    serde_json::from_slice(&bytes).expect("operator response must be JSON")
}

#[test]
fn unconfigured_postgresql_is_optional_and_ready() {
    let dependency = optional_postgresql_readiness(false, false);
    assert!(!dependency.configured);
    assert!(!dependency.required);
    assert!(dependency.ready);
    assert!(!dependency.timed_out);
}

#[test]
fn configured_unavailable_postgresql_is_required_and_not_ready() {
    let dependency = optional_postgresql_readiness(true, false);
    assert!(dependency.configured);
    assert!(dependency.required);
    assert!(!dependency.ready);
    assert!(!dependency.timed_out);
}

#[tokio::test]
async fn bounded_dependency_probe_returns_stable_timeout_state() {
    let started = Instant::now();
    let outcome = bounded_readiness_probe(Duration::from_millis(20), async {
        std::future::pending::<bool>().await
    })
    .await;

    assert!(!outcome.ready);
    assert!(outcome.timed_out);
    assert!(started.elapsed() < Duration::from_secs(1));
}

#[tokio::test]
async fn operator_summary_route_requires_management_authentication() {
    let response = test_app()
        .oneshot(
            Request::get("/v1/internal/gateway/operations/summary")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn draining_runtime_keeps_authenticated_operator_summary_available() {
    let state = test_state();
    assert!(state.lifecycle.begin_drain("contract-drain"));
    let response = build_router(state)
        .oneshot(
            Request::get("/v1/internal/gateway/operations/summary")
                .header("x-management-token", MANAGEMENT_TOKEN)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body = parse_json(response).await;
    assert_eq!(body["summary"]["lifecycle"]["draining"], true);
    assert_eq!(
        body["summary"]["lifecycle"]["drainReason"],
        "contract-drain"
    );
}

#[tokio::test]
async fn draining_runtime_still_rejects_unauthenticated_operator_summary() {
    let state = test_state();
    assert!(state.lifecycle.begin_drain("contract-drain"));
    let response = build_router(state)
        .oneshot(
            Request::get("/v1/internal/gateway/operations/summary")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn internal_readiness_reports_unconfigured_postgresql_as_optional() {
    let response = test_app()
        .oneshot(
            Request::get("/v1/internal/gateway/readiness")
                .header("x-management-token", MANAGEMENT_TOKEN)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body = parse_json(response).await;
    let readiness = &body["readiness"];
    assert_eq!(readiness["checks"]["databaseConfigured"], false);
    assert_eq!(readiness["checks"]["database"], true);
    assert_eq!(readiness["dependencies"]["postgresql"]["configured"], false);
    assert_eq!(readiness["dependencies"]["postgresql"]["required"], false);
    assert_eq!(readiness["dependencies"]["postgresql"]["ready"], true);
}

#[tokio::test]
async fn operator_summary_reports_runtime_state_without_secrets() {
    let response = test_app()
        .oneshot(
            Request::get("/v1/internal/gateway/operations/summary")
                .header("x-management-token", MANAGEMENT_TOKEN)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body = parse_json(response).await;
    let summary = &body["summary"];
    assert_eq!(summary["schemaVersion"], 1);
    assert_eq!(summary["build"]["name"], "neuro-gateway");
    assert_eq!(summary["runtime"]["role"], "standalone");
    assert_eq!(summary["lifecycle"]["draining"], false);
    assert!(summary["lifecycle"]["activeRequests"].is_number());
    assert_eq!(
        summary["readiness"]["dependencies"]["postgresql"]["configured"],
        false
    );
    assert_eq!(
        summary["readiness"]["dependencies"]["postgresql"]["ready"],
        true
    );
    assert_eq!(
        summary["readiness"]["dependencies"]["postgresql"]["timedOut"],
        false
    );
    assert_eq!(summary["routing"]["providerCount"], 0);
    assert_eq!(summary["routing"]["routeCount"], 0);
    assert_eq!(summary["credentialCache"]["entryCount"], 0);
    assert!(summary["requestMetrics"]["requestsTotal"].is_number());
    assert!(summary["providerStats"]["activeProviders"].is_number());

    let serialized = serde_json::to_string(&body).unwrap();
    for secret in [
        MANAGEMENT_TOKEN,
        "public-api-contract-secret",
        "hmac-contract-secret",
        "keepalive-contract-secret",
        "redis://127.0.0.1:1/15",
    ] {
        assert!(!serialized.contains(secret), "summary leaked {secret}");
    }
}

#[tokio::test]
async fn operator_summary_degrades_when_redis_probe_times_out() {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind stalled Redis fixture");
    let address = listener.local_addr().expect("read stalled Redis address");
    let stalled_server = tokio::spawn(async move {
        let (_socket, _) = listener.accept().await.expect("accept Redis probe");
        std::future::pending::<()>().await;
    });
    let state = test_state_with_redis_url(&format!("redis://{address}/15"));

    let started = Instant::now();
    let response = build_router(state)
        .oneshot(
            Request::get("/v1/internal/gateway/operations/summary")
                .header("x-management-token", MANAGEMENT_TOKEN)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let elapsed = started.elapsed();
    stalled_server.abort();

    assert_eq!(response.status(), StatusCode::OK);
    assert!(elapsed < Duration::from_secs(5));
    let body = parse_json(response).await;
    let redis = &body["summary"]["readiness"]["dependencies"]["redis"];
    assert_eq!(redis["ready"], false);
    assert_eq!(redis["timedOut"], true);
    assert_eq!(body["summary"]["readiness"]["ok"], false);
}

#[test]
fn operations_alerts_are_parseable_and_probe_the_summary_endpoint() {
    let alerts: serde_yaml::Value =
        serde_yaml::from_str(include_str!("../docs/operations-alerts.yaml"))
            .expect("operations alerts must be valid YAML");
    let groups = alerts["groups"]
        .as_sequence()
        .expect("Prometheus rule groups array");
    assert!(!groups.is_empty());
    let entries = groups[0]["rules"]
        .as_sequence()
        .expect("Prometheus rules array");
    assert!(entries.iter().all(|entry| {
        entry["alert"].is_string()
            && entry["expr"].is_string()
            && entry["for"].is_string()
            && entry["labels"]["severity"].is_string()
            && entry["annotations"]["runbook_url"] == "operations-manual.md"
    }));
    let summary_alert = entries
        .iter()
        .find(|entry| entry["alert"] == "GatewayOperatorSummaryUnavailable")
        .expect("summary endpoint alert");
    assert_eq!(
        summary_alert["expr"],
        r#"probe_success{job="gateway-operator-summary"} == 0"#
    );
    assert_eq!(summary_alert["for"], "5m");
}
