//! Real Console HTTP auth/SecretPatch/CAS chain with synthetic state, never upstream inference.
use axum::http::{Method, StatusCode};
use neuro_gateway::console::secrets::redact_route_document;
use neuro_gateway::routing::config::RouteConfigYaml;
use serde_json::{json, Value};

#[path = "console_contract_support/mod.rs"]
mod support;
use support::{grant_console_secret_access, parse_json, send_console_json, ConsoleStateFixture};

fn active() -> RouteConfigYaml {
    serde_json::from_value(json!({"providers":[{"id":"bedrock", "preset":"bedrock-converse", "auth_mode":"aws_sigv4",
        "base_url":"https://bedrock-runtime.us-east-1.amazonaws.com", "explicit_credentials_only":true,
        "credentials":[{"id":"a", "extra_body":{"AWSACCESSKEYID":"synthetic-access", "awsSecretAccessKey":"synthetic-secret", "awsRegion":"us-east-1"}}]}]})).unwrap()
}

fn candidate(fixture: &ConsoleStateFixture, replace: bool) -> Value {
    let redacted = redact_route_document(&active()).unwrap();
    let mut draft = serde_json::to_value(redacted.document).unwrap();
    draft["providers"][0]["credentials"][0]["base_url"] =
        json!("https://bedrock-runtime.us-west-2.amazonaws.com");
    draft["providers"][0]["credentials"][0]["extra_body"]["awsRegion"] = json!("us-west-2");
    let patches: Vec<_> = redacted
        .secrets
        .iter()
        .filter(|s| s.configured)
        .map(|s| {
            if replace {
                json!({"path":s.path,"operation":"replace","value":"synthetic-replaced"})
            } else {
                json!({"path":s.path,"operation":"keep"})
            }
        })
        .collect();
    json!({"document":draft, "secretPatches":patches,
        "expectedRevision":fixture.state.route_config.snapshot().revision().id()})
}

fn run<F: std::future::Future<Output = ()> + 'static>(test: fn() -> F) {
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
    let revision = fixture
        .state
        .route_config
        .snapshot()
        .revision()
        .id()
        .to_owned();
    let exact = grant_console_secret_access(&fixture.state).await;
    for (method, endpoint) in [
        (
            Method::POST,
            "/v1/internal/gateway/console/route-config/validate",
        ),
        (Method::PUT, "/v1/internal/gateway/console/route-config"),
    ] {
        for grant in [None, Some(exact.as_str())] {
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
                revision
            );
            assert_eq!(
                fixture.state.route_config.get_providers()[0].credential_pool[0]
                    .payload
                    .extra_body
                    .as_ref()
                    .unwrap()["awsRegion"],
                json!("us-east-1")
            );
        }
    }
}

async fn replacement_and_cas() {
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
    let response = send_console_json(
        &fixture.state,
        Method::PUT,
        endpoint,
        body.clone(),
        Some(&grant),
        None,
    )
    .await;
    assert_eq!(response.status(), StatusCode::OK);
    let response = parse_json(response).await;
    assert!(!response["routeConfig"]["document"]
        .to_string()
        .contains("synthetic-"));
    let snapshot = fixture.state.route_config.snapshot();
    assert_ne!(snapshot.revision().id(), original);
    let providers = fixture.state.route_config.get_providers();
    let credential = providers[0].credential_pool[0]
        .payload
        .extra_body
        .as_ref()
        .unwrap();
    assert_eq!(credential["AWSACCESSKEYID"], json!("synthetic-replaced"));
    assert_eq!(credential["awsRegion"], json!("us-west-2"));
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
fn bedrock_console_rejects_retargeted_keep_even_with_a_secret_grant() {
    run(rejects_keep);
}

#[test]
fn bedrock_console_replacement_requires_exact_grant_and_current_revision() {
    run(replacement_and_cas);
}
