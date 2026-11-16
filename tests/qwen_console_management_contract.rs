//! Existing raw workspace configuration exercises real Console auth/SecretPatch/CAS, not a new preset.
use axum::http::{Method, StatusCode};
use neuro_gateway::console::secrets::redact_route_document;
use neuro_gateway::routing::config::RouteConfigYaml;
use serde_json::{json, Value};

#[path = "console_contract_support/mod.rs"]
mod support;
use support::{grant_console_secret_access, parse_json, send_console_json, ConsoleStateFixture};

const BASE: &str = "https://workspace-a.cn-beijing.maas.aliyuncs.com/compatible-mode/v1";
const OTHER: &str = "https://workspace-b.eu-central-1.maas.aliyuncs.com/compatible-mode/v1";

fn active() -> RouteConfigYaml {
    serde_json::from_value(
        json!({"providers":[{"id":"arbitrary-name", "adapter":"openai_compatible",
        "auth_mode":"qwen_workspace_api", "protocol_profile":"qwen_official_api", "base_url":BASE,
        "explicit_credentials_only":true, "credentials":[{"id":"a", "api_key":"synthetic-key"}]}]}),
    )
    .unwrap()
}

fn candidate(fixture: &ConsoleStateFixture, replace: bool) -> Value {
    let redacted = redact_route_document(&active()).unwrap();
    let mut draft = serde_json::to_value(redacted.document).unwrap();
    draft["providers"][0]["credentials"][0]["base_url"] = json!(OTHER);
    let patches: Vec<_> = redacted
        .secrets
        .iter()
        .filter(|s| s.configured)
        .map(|s| {
            if replace {
                json!({"path":s.path, "operation":"replace", "value":"synthetic-new"})
            } else {
                json!({"path":s.path, "operation":"keep"})
            }
        })
        .collect();
    json!({"document":draft, "secretPatches":patches,
        "expectedRevision":fixture.state.route_config.snapshot().revision().id()})
}

fn run<F: std::future::Future<Output = ()> + 'static>(test: fn() -> F) {
    // Real router futures exceed Windows' default test-thread stack; no production worker is added.
    std::thread::Builder::new()
        .stack_size(16 * 1024 * 1024)
        .spawn(move || {
            tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .unwrap()
                .block_on(test());
        })
        .unwrap()
        .join()
        .unwrap();
}

async fn rejects_keep() {
    let fixture = ConsoleStateFixture::new(active(), true);
    let original = fixture
        .state
        .route_config
        .snapshot()
        .revision()
        .id()
        .to_owned();
    let grant = grant_console_secret_access(&fixture.state).await;
    for (method, endpoint) in [
        (
            Method::POST,
            "/v1/internal/gateway/console/route-config/validate",
        ),
        (Method::PUT, "/v1/internal/gateway/console/route-config"),
    ] {
        for grant in [None, Some(grant.as_str())] {
            let response = send_console_json(
                &fixture.state,
                method.clone(),
                endpoint,
                candidate(&fixture, false),
                grant,
                None,
            )
            .await;
            assert_eq!(response.status(), StatusCode::BAD_REQUEST);
            let body = parse_json(response).await;
            assert_eq!(body["error"]["code"], "secret_keep_destination_changed");
            assert!(!body.to_string().contains("synthetic-"));
            assert_eq!(
                fixture.state.route_config.snapshot().revision().id(),
                original
            );
            assert_eq!(
                fixture.state.route_config.get_providers()[0].credential_pool[0]
                    .payload
                    .base_url,
                BASE
            );
        }
    }
}

async fn replaces_with_exact_grant_and_cas() {
    let fixture = ConsoleStateFixture::new(active(), true);
    let original = fixture
        .state
        .route_config
        .snapshot()
        .revision()
        .id()
        .to_owned();
    let body = candidate(&fixture, true);
    let endpoint = "/v1/internal/gateway/console/route-config";
    let missing = send_console_json(
        &fixture.state,
        Method::PUT,
        endpoint,
        body.clone(),
        None,
        None,
    )
    .await;
    assert_eq!(missing.status(), StatusCode::FORBIDDEN);
    let grant = grant_console_secret_access(&fixture.state).await;
    let wrong = send_console_json(
        &fixture.state,
        Method::PUT,
        endpoint,
        body.clone(),
        Some(&grant),
        Some("https://wrong-origin.invalid"),
    )
    .await;
    assert_eq!(wrong.status(), StatusCode::FORBIDDEN);
    assert_eq!(
        fixture.state.route_config.snapshot().revision().id(),
        original
    );
    let success = send_console_json(
        &fixture.state,
        Method::PUT,
        endpoint,
        body.clone(),
        Some(&grant),
        None,
    )
    .await;
    assert_eq!(success.status(), StatusCode::OK);
    assert!(!parse_json(success).await["routeConfig"]["document"]
        .to_string()
        .contains("synthetic-"));
    let snapshot = fixture.state.route_config.snapshot();
    assert_ne!(snapshot.revision().id(), original);
    let providers = fixture.state.route_config.get_providers();
    assert_eq!(providers[0].credential_pool[0].payload.base_url, OTHER);
    assert_eq!(
        providers[0].credential_pool[0].payload.api_key,
        "synthetic-new"
    );
    let stale = send_console_json(
        &fixture.state,
        Method::PUT,
        endpoint,
        body,
        Some(&grant),
        None,
    )
    .await;
    assert_eq!(stale.status(), StatusCode::CONFLICT);
    assert_eq!(
        fixture.state.route_config.snapshot().revision().id(),
        snapshot.revision().id()
    );
}

#[test]
fn qwen_console_rejects_retargeted_keep_with_or_without_secret_grant() {
    run(rejects_keep);
}

#[test]
fn qwen_console_replacement_requires_exact_secret_grant_and_current_revision() {
    run(replaces_with_exact_grant_and_cas);
}
