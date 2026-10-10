//! Creation commits before I/O; only current account intents receive asynchronous results.
use axum::{
    http::{Method, StatusCode},
    routing::{get, post},
    Json, Router,
};
use neuro_gateway::{
    console::secrets::redact_route_document,
    routing::config::{RouteConfigStore, RouteConfigYaml},
};
use serde_json::json;
use std::{sync::Arc, time::Duration};
use tokio::sync::Semaphore;
#[path = "console_contract_support/mod.rs"]
mod support;
use support::*;

async fn server() -> (
    String,
    Arc<Semaphore>,
    Arc<Semaphore>,
    tokio::task::JoinHandle<()>,
) {
    let entered = Arc::new(Semaphore::new(0));
    let release = Arc::new(Semaphore::new(0));
    let (seen, gate) = (entered.clone(), release.clone());
    let app = Router::new()
        .route(
            "/v1/models",
            get(|| async { Json(json!({"data":[{"id":"fixture-model"}]})) }),
        )
        .route(
            "/v1/chat/completions",
            post(move || {
                let (seen, gate) = (seen.clone(), gate.clone());
                async move {
                    seen.add_permits(1);
                    gate.acquire().await.unwrap().forget();
                    Json(json!({"choices":[{"message":{"role":"assistant","content":"OK"}}]}))
                }
            }),
        );
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let root = format!("http://{}", listener.local_addr().unwrap());
    let task = tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    (root, entered, release, task)
}

async fn create(fixture: &ConsoleStateFixture, root: &str) {
    let grant = grant_console_secret_access(&fixture.state).await;
    let revision = fixture
        .state
        .route_config
        .snapshot()
        .revision()
        .id()
        .to_owned();
    let response = tokio::time::timeout(Duration::from_secs(3), send_console_json(
        &fixture.state, Method::PUT, "/v1/internal/gateway/console/route-config",
        json!({"expectedRevision":revision,"document":{"providers":[{
            "id":"pool","adapter":"openai_compatible","base_url":root,
            "credentials":[{"id":"account","discovery_job":{"id":uuid::Uuid::new_v4().to_string(),"status":"pending"}}]
        }],"model_routes":[]},"secretPatches":[{
            "path":"/providers/0/credentials/0/api_key","operation":"replace","value":"fixture-secret"
        }]}), Some(&grant), None,
    )).await.expect("Creation must not wait for upstream generation");
    let status = response.status();
    let body = parse_json(response).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert!(!body.to_string().contains("fixture-secret"));
    assert_eq!(
        body["routeConfig"]["document"]["providers"][0]["credentials"][0]["discovery_job"]
            ["status"],
        "pending"
    );
}

async fn settled(fixture: &ConsoleStateFixture) {
    tokio::time::timeout(Duration::from_secs(10), async {
        loop {
            let snapshot = read_document(fixture).await;
            let status = snapshot.providers[0].credentials[0]
                .discovery_job
                .as_ref()
                .unwrap()
                .status;
            if status != neuro_gateway::provider_discovery::job::DiscoveryJobStatus::Pending {
                return;
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    })
    .await
    .expect("Background job should finish");
}

async fn read_document(fixture: &ConsoleStateFixture) -> RouteConfigYaml {
    let response = send_console_json(
        &fixture.state,
        Method::GET,
        "/v1/internal/gateway/console/route-config",
        json!(null),
        None,
        None,
    )
    .await;
    assert_eq!(response.status(), StatusCode::OK);
    let body = parse_json(response).await;
    let mut document: RouteConfigYaml =
        serde_json::from_value(body["routeConfig"]["document"].clone()).unwrap();
    // Only synthetic fixture keys: inspect the public compiled store for round-trip proof.
    document.providers[0].credentials[0].api_key = Some(
        fixture.state.route_config.get_providers()[0].credential_pool[0]
            .payload
            .api_key
            .clone(),
    );
    document
}

fn empty() -> RouteConfigYaml {
    serde_json::from_value(json!({"providers":[]})).unwrap()
}

#[tokio::test]
async fn create_returns_first_then_merges_into_latest_revision_and_reloads() {
    let (root, entered, release, server) = server().await;
    let fixture = ConsoleStateFixture::new(empty(), true);
    create(&fixture, &root).await;
    tokio::time::timeout(Duration::from_secs(5), entered.acquire())
        .await
        .unwrap()
        .unwrap()
        .forget();
    assert!(fixture
        .state
        .route_config
        .resolve_candidates(Some("fixture-model"))
        .is_empty());
    let mut latest = read_document(&fixture).await;
    let mut redirected = redact_route_document(&latest).unwrap().document;
    redirected.providers[0].base_url = "http://127.0.0.1:1".into();
    let denied = send_console_json(&fixture.state, Method::PUT,
        "/v1/internal/gateway/console/route-config", json!({
            "expectedRevision":fixture.state.route_config.snapshot().revision().id(), "document":redirected,
            "secretPatches":[{"path":"/providers/0/credentials/0/api_key","operation":"keep"}]
        }), None, None).await;
    assert_eq!(
        denied.status(),
        StatusCode::FORBIDDEN,
        "Rebinding a pending intent needs secret access"
    );
    latest.providers[0].label = Some("User edited this while discovering".into());
    fixture.commit_route_document(latest, None).await;
    release.add_permits(1);
    settled(&fixture).await;
    let saved = read_document(&fixture).await;
    assert_eq!(
        saved.providers[0].label.as_deref(),
        Some("User edited this while discovering")
    );
    let credential = &saved.providers[0].credentials[0];
    assert!(credential.discovery.is_some());
    assert_eq!(credential.api_key.as_deref(), Some("fixture-secret"));
    assert_eq!(saved.model_routes[0].pattern, "fixture-model");
    let yaml = serde_yaml::to_string(&saved).unwrap();
    let reloaded = RouteConfigStore::from_document(serde_yaml::from_str(&yaml).unwrap()).unwrap();
    assert_eq!(reloaded.resolve_candidates(Some("fixture-model")).len(), 1);
    fixture.state.shutdown.request("test complete");
    server.abort();
}

#[tokio::test]
async fn late_result_does_not_apply_to_a_replaced_key() {
    let (root, entered, release, server) = server().await;
    let fixture = ConsoleStateFixture::new(empty(), true);
    create(&fixture, &root).await;
    tokio::time::timeout(Duration::from_secs(5), entered.acquire())
        .await
        .unwrap()
        .unwrap()
        .forget();
    let mut latest = read_document(&fixture).await;
    latest.providers[0].credentials[0].api_key = Some("replacement-fixture".into());
    fixture.commit_route_document(latest, None).await;
    release.add_permits(1);
    settled(&fixture).await;
    let saved = read_document(&fixture).await;
    let credential = &saved.providers[0].credentials[0];
    assert!(credential.discovery.is_none());
    assert_eq!(credential.api_key.as_deref(), Some("replacement-fixture"));
    assert_eq!(
        credential.discovery_job.as_ref().unwrap().status,
        neuro_gateway::provider_discovery::job::DiscoveryJobStatus::Unconfirmed
    );
    assert!(saved.model_routes.is_empty());
    fixture.state.shutdown.request("test complete");
    server.abort();
}

#[tokio::test]
async fn failure_keeps_the_saved_account_and_requires_grant_for_new_intent() {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let root = format!("http://{}", listener.local_addr().unwrap());
    let server = tokio::spawn(async move { axum::serve(listener, Router::new()).await.unwrap() });
    let fixture = ConsoleStateFixture::new(empty(), true);
    create(&fixture, &root).await;
    settled(&fixture).await;
    let active = fixture.state.route_config.snapshot();
    let saved = read_document(&fixture).await;
    let credential = &saved.providers[0].credentials[0];
    assert!(credential.discovery.is_none());
    assert_eq!(credential.api_key.as_deref(), Some("fixture-secret"));
    let mut draft = redact_route_document(&saved).unwrap().document;
    draft.providers[0].credentials[0].discovery_job =
        Some(neuro_gateway::provider_discovery::job::DiscoveryJob {
            id: uuid::Uuid::new_v4().to_string(),
            status: neuro_gateway::provider_discovery::job::DiscoveryJobStatus::Pending,
        });
    let denied = send_console_json(&fixture.state, Method::PUT,
        "/v1/internal/gateway/console/route-config", json!({"expectedRevision":active.revision().id(),
            "document":draft,"secretPatches":[{"path":"/providers/0/credentials/0/api_key","operation":"keep"}]}), None, None).await;
    assert_eq!(denied.status(), StatusCode::FORBIDDEN);
    fixture.state.shutdown.request("test complete");
    server.abort();
}

#[tokio::test]
async fn generation_can_outlive_the_old_hundred_second_timeout() {
    assert_eq!(
        neuro_gateway::provider_discovery::REQUEST_TIMEOUT,
        Duration::from_secs(600)
    );
    let (root, entered, release, server) = server().await;
    let task = tokio::spawn(async move {
        neuro_gateway::provider_discovery::discover(&root, "fixture-secret").await
    });
    tokio::time::timeout(Duration::from_secs(5), entered.acquire())
        .await
        .unwrap()
        .unwrap()
        .forget();
    tokio::time::pause();
    tokio::time::advance(Duration::from_secs(101)).await;
    for _ in 0..10 {
        tokio::task::yield_now().await;
    }
    assert!(
        !task.is_finished(),
        "The old per-request timeout must not reject the protocol"
    );
    tokio::time::resume();
    release.add_permits(1);
    let result = tokio::time::timeout(Duration::from_secs(10), task)
        .await
        .unwrap()
        .unwrap()
        .unwrap();
    assert_eq!(
        result.protocol,
        neuro_gateway::provider_discovery::DiscoveredProtocol::ChatCompletions
    );
    server.abort();
}

#[tokio::test]
async fn replacement_job_cannot_receive_the_previous_jobs_result() {
    let (root, entered, release, server) = server().await;
    let fixture = ConsoleStateFixture::new(empty(), true);
    create(&fixture, &root).await;
    tokio::time::timeout(Duration::from_secs(5), entered.acquire())
        .await
        .unwrap()
        .unwrap()
        .forget();
    let mut latest = read_document(&fixture).await;
    let replacement = uuid::Uuid::new_v4().to_string();
    latest.providers[0].credentials[0]
        .discovery_job
        .as_mut()
        .unwrap()
        .id = replacement.clone();
    fixture.commit_route_document(latest, None).await;
    release.add_permits(1);
    // The new intent must execute its own probe, not be marked complete by the old one.
    tokio::time::timeout(Duration::from_secs(5), entered.acquire())
        .await
        .unwrap()
        .unwrap()
        .forget();
    assert!(read_document(&fixture).await.providers[0].credentials[0]
        .discovery
        .is_none());
    release.add_permits(1);
    settled(&fixture).await;
    let saved = read_document(&fixture).await;
    assert_eq!(
        saved.providers[0].credentials[0]
            .discovery_job
            .as_ref()
            .unwrap()
            .id,
        replacement
    );
    assert!(saved.providers[0].credentials[0].discovery.is_some());
    fixture.state.shutdown.request("test complete");
    server.abort();
}

#[tokio::test]
async fn request_deadline_is_ten_minutes_not_an_unbounded_wait() {
    let (root, entered, release, server) = server().await;
    let task = tokio::spawn(async move {
        neuro_gateway::provider_discovery::discover(&root, "fixture-secret").await
    });
    tokio::time::timeout(Duration::from_secs(5), entered.acquire())
        .await
        .unwrap()
        .unwrap()
        .forget();
    tokio::time::pause();
    tokio::time::advance(Duration::from_secs(599)).await;
    for _ in 0..10 {
        tokio::task::yield_now().await;
    }
    assert!(!task.is_finished());
    tokio::time::advance(Duration::from_secs(2)).await;
    for _ in 0..10 {
        tokio::task::yield_now().await;
    }
    tokio::time::resume();
    let result = tokio::time::timeout(Duration::from_secs(10), task)
        .await
        .unwrap()
        .unwrap();
    assert!(result.is_err());
    release.add_permits(1);
    server.abort();
}

#[tokio::test]
async fn retrying_writeback_does_not_repeat_completed_generation() {
    use std::sync::atomic::Ordering;
    let (root, entered, release, server) = server().await;
    let fixture = ConsoleStateFixture::new(empty(), true);
    create(&fixture, &root).await;
    tokio::time::timeout(Duration::from_secs(5), entered.acquire())
        .await
        .unwrap()
        .unwrap()
        .forget();
    fixture.fail_next_activation.store(true, Ordering::SeqCst);
    release.add_permits(1);
    tokio::time::timeout(Duration::from_secs(5), async {
        while !fixture.activation_failure_seen.load(Ordering::SeqCst) {
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .unwrap();
    // The fixture rebuilds the router for reads, waking its existing worker.
    // No second upstream reply is released: a repeated paid probe would hang here.
    settled(&fixture).await;
    assert!(read_document(&fixture).await.providers[0].credentials[0]
        .discovery
        .is_some());
    assert_eq!(entered.available_permits(), 0);
    fixture.state.shutdown.request("test complete");
    server.abort();
}
