//! Renewal may span a dry-run, but its result still requires the original owner's current authority.
use axum::{http::StatusCode, routing::post, Json, Router};
use neuro_gateway::{
    console::{ConsoleAuthRuntime, ConsoleConfig, ConsoleConfigValues, ConsoleRequestContext},
    routing::config::RouteConfigYaml,
};
use serde_json::json;
use std::{path::PathBuf, sync::Arc, time::Duration};
use tokio::sync::Notify;
#[path = "console_contract_support/mod.rs"]
mod support;
use support::*;

struct Probe {
    fixture: ConsoleStateFixture,
    auth_root: PathBuf,
    server: tokio::task::JoinHandle<()>,
    entered: Arc<Notify>,
    release: Arc<Notify>,
}

impl Probe {
    async fn new() -> Self {
        let entered = Arc::new(Notify::new());
        let release = Arc::new(Notify::new());
        let started = entered.clone();
        let resumed = release.clone();
        let upstream = Router::new().route(
            "/v1/chat/completions",
            post(move || {
                let started = started.clone();
                let resumed = resumed.clone();
                async move {
                    started.notify_one();
                    resumed.notified().await;
                    Json(json!({"choices":[{"message":{"content":"OK"}}]}))
                }
            }),
        );
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let base = format!("http://{}", listener.local_addr().unwrap());
        let server = tokio::spawn(async move { axum::serve(listener, upstream).await.unwrap() });
        let active: RouteConfigYaml = serde_json::from_value(json!({"providers":[{
            "id":"pool","adapter":"openai_compatible","base_url":base,
            "credentials":[{"id":"account","api_key":"fixture-secret"}]}],"model_routes":[]}))
        .unwrap();
        let mut fixture = ConsoleStateFixture::new(active, true);
        let auth_root =
            std::env::temp_dir().join(format!("discovery-authority-{}", uuid::Uuid::new_v4()));
        let config = ConsoleConfig::from_values(ConsoleConfigValues {
            state_dir: Some(auth_root.clone()),
            routes_file: Some(auth_root.join("routes.yaml")),
            secret_grant_ttl_secs: Some(1),
            ..Default::default()
        })
        .unwrap();
        Arc::get_mut(&mut fixture.state).unwrap().console_auth =
            Arc::new(ConsoleAuthRuntime::new(&config, Some(MANAGEMENT_TOKEN.into())).unwrap());
        Self {
            fixture,
            auth_root,
            server,
            entered,
            release,
        }
    }

    async fn start(&self) -> (String, tokio::task::JoinHandle<axum::response::Response>) {
        let grant = grant_console_secret_access(&self.fixture.state).await;
        let state = self.fixture.state.clone();
        let initial_grant = grant.clone();
        let pending = tokio::spawn(async move {
            send_console_json(
                &state,
                axum::http::Method::POST,
                "/v1/internal/gateway/console/account-discovery",
                json!({"credentialId":"account","models":["fixture"]}),
                Some(&initial_grant),
                None,
            )
            .await
        });
        tokio::time::timeout(Duration::from_secs(5), self.entered.notified())
            .await
            .unwrap();
        (grant, pending)
    }

    async fn finish(
        &self,
        pending: tokio::task::JoinHandle<axum::response::Response>,
    ) -> axum::response::Response {
        self.release.notify_one();
        tokio::time::timeout(Duration::from_secs(5), pending)
            .await
            .unwrap()
            .unwrap()
    }
}

impl Drop for Probe {
    fn drop(&mut self) {
        self.server.abort();
        std::fs::remove_dir_all(&self.auth_root).unwrap();
    }
}

#[tokio::test]
async fn discovery_accepts_same_owner_renewal_after_the_original_grant_expires() {
    let probe = Probe::new().await;
    let (original, pending) = probe.start().await;
    tokio::time::sleep(Duration::from_millis(1100)).await;
    let renewed = grant_console_secret_access(&probe.fixture.state).await;
    assert_ne!(original, renewed);
    let actor = probe
        .fixture
        .state
        .console_auth
        .authenticate_management_token(&ConsoleRequestContext::loopback(), MANAGEMENT_TOKEN)
        .unwrap();
    assert!(probe
        .fixture
        .state
        .console_auth
        .verify_secret_grant(&ConsoleRequestContext::loopback(), &actor, &original)
        .is_err());
    let revision = probe
        .fixture
        .state
        .route_config
        .snapshot()
        .revision()
        .id()
        .to_owned();
    let response = probe.finish(pending).await;
    let status = response.status();
    assert_eq!(response.headers()["cache-control"], "no-store");
    let body = parse_json(response).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["discovery"]["protocol"], "chat_completions");
    assert_eq!(body["revision"], revision);
    assert!(!body.to_string().contains("fixture-secret"));
    assert_eq!(
        probe.fixture.state.route_config.snapshot().revision().id(),
        revision
    );
}

#[tokio::test]
async fn discovery_still_rejects_expired_authority_without_renewal() {
    let probe = Probe::new().await;
    let (_, pending) = probe.start().await;
    tokio::time::sleep(Duration::from_millis(1100)).await;
    let response = probe.finish(pending).await;
    assert_eq!(response.status(), StatusCode::FORBIDDEN);
    assert_eq!(
        parse_json(response).await["error"]["code"],
        "console_secret_access_required"
    );
}

#[tokio::test]
async fn discovery_rejects_logout_even_while_the_original_grant_would_be_valid() {
    let probe = Probe::new().await;
    let (_, pending) = probe.start().await;
    probe
        .fixture
        .state
        .console_auth
        .logout(&ConsoleRequestContext::loopback(), MANAGEMENT_TOKEN)
        .unwrap();
    let response = probe.finish(pending).await;
    assert_eq!(response.status(), StatusCode::FORBIDDEN);
    assert_eq!(
        parse_json(response).await["error"]["code"],
        "console_secret_access_required"
    );
}

#[tokio::test]
async fn discovery_does_not_accept_another_origin_or_client_ip_as_renewal() {
    for context in [
        ConsoleRequestContext::new("127.0.0.1".parse().unwrap(), "other-origin".into()),
        ConsoleRequestContext::new("127.0.0.2".parse().unwrap(), "local".into()),
    ] {
        let probe = Probe::new().await;
        let (_, pending) = probe.start().await;
        probe
            .fixture
            .state
            .console_auth
            .logout(&ConsoleRequestContext::loopback(), MANAGEMENT_TOKEN)
            .unwrap();
        probe
            .fixture
            .state
            .console_auth
            .confirm_secret_access(&context, MANAGEMENT_TOKEN, MANAGEMENT_TOKEN)
            .unwrap();
        let response = probe.finish(pending).await;
        assert_eq!(response.status(), StatusCode::FORBIDDEN);
    }
}

#[tokio::test]
async fn discovery_reauthenticates_the_original_management_token_after_rotation() {
    let mut probe = Probe::new().await;
    let config = ConsoleConfig::from_values(ConsoleConfigValues {
        state_dir: Some(probe.auth_root.clone()),
        routes_file: Some(probe.auth_root.join("routes.yaml")),
        ..Default::default()
    })
    .unwrap();
    let auth = Arc::new(ConsoleAuthRuntime::new(&config, None).unwrap());
    let context = ConsoleRequestContext::loopback();
    auth.bootstrap(&context, MANAGEMENT_TOKEN).unwrap();
    Arc::get_mut(&mut probe.fixture.state).unwrap().console_auth = auth.clone();
    let (_, pending) = probe.start().await;
    auth.rotate_token(&context, MANAGEMENT_TOKEN, "new-manager")
        .unwrap();
    auth.confirm_secret_access(&context, "new-manager", "new-manager")
        .unwrap();
    let response = probe.finish(pending).await;
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    assert_eq!(
        parse_json(response).await["error"]["code"],
        "console_management_token_invalid"
    );
}
