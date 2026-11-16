//! Accio credential-only rotation secret uses normal management auth, SecretPatch and revision CAS.
use axum::http::{Method, StatusCode};
use neuro_gateway::{console::secrets::redact_route_document, routing::config::RouteConfigYaml};
use serde_json::{json, Value};
#[path = "console_contract_support/mod.rs"]
mod support;
use support::{grant_console_secret_access, parse_json, send_console_json, ConsoleStateFixture};

const ENDPOINT: &str = "/v1/internal/gateway/console/route-config";
const COOKIE: &str = "/providers/0/credentials/0/accio_sgcookie";
fn active() -> RouteConfigYaml {
    serde_json::from_value(
        json!({"providers":[{"id":"arbitrary-accio-pool", "preset":"accio",
        "base_url":"https://phoenix-gw.alibaba.com", "explicit_credentials_only":true,
        "credentials":[{"id":"stable-account", "api_key":"synthetic-access",
        "refresh_token":"synthetic-refresh", "accio_sgcookie":"synthetic-old-cookie",
        "refresh_endpoint":"https://phoenix-gw.alibaba.com/api/auth/refresh_token",
        "refresh_client_id":"synthetic-client", "expires_at":"2099-01-01T00:00:00Z",
        "headers":{"utdid":"synthetic-device"}}]}]}),
    )
    .unwrap()
}
fn candidate(fixture: &ConsoleStateFixture, active: &RouteConfigYaml, operation: &str) -> Value {
    let snapshot = fixture.state.route_config.snapshot();
    let redacted = redact_route_document(active).unwrap();
    let patches: Vec<_> = redacted.secrets.iter().filter(|secret| secret.configured).map(|secret| {
        if secret.path == COOKIE {
            if operation == "replace" {
                json!({"path":secret.path, "operation":operation,"value":"synthetic-new-cookie"})
            } else { json!({"path":secret.path, "operation":operation}) }
        } else { json!({"path":secret.path,"operation":"keep"}) }
    }).collect();
    json!({"document":redacted.document, "secretPatches":patches, "expectedRevision":snapshot.revision().id()})
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

async fn replaces_and_clears() {
    let fixture = ConsoleStateFixture::new(active(), true);
    let body = candidate(&fixture, &active(), "replace");
    assert!(body["secretPatches"]
        .as_array()
        .unwrap()
        .iter()
        .any(|patch| patch["path"] == COOKIE));
    let original = fixture
        .state
        .route_config
        .snapshot()
        .revision()
        .id()
        .to_owned();
    let response = send_console_json(
        &fixture.state,
        Method::PUT,
        ENDPOINT,
        body.clone(),
        None,
        None,
    )
    .await;
    assert_eq!(response.status(), StatusCode::FORBIDDEN);
    let grant = grant_console_secret_access(&fixture.state).await;
    let wrong = send_console_json(
        &fixture.state,
        Method::PUT,
        ENDPOINT,
        body.clone(),
        Some(&grant),
        Some("https://foreign.invalid"),
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
        ENDPOINT,
        body.clone(),
        Some(&grant),
        None,
    )
    .await;
    assert_eq!(success.status(), StatusCode::OK);
    let response = parse_json(success).await;
    assert!(!response.to_string().contains("synthetic-new-cookie"));
    assert!(!response.to_string().contains("synthetic-old-cookie"));
    let snapshot = fixture.state.route_config.snapshot();
    let saved = fixture
        .state
        .route_config_runtime
        .as_ref()
        .unwrap()
        .coordinator()
        .unwrap()
        .load_revision(snapshot.revision().id())
        .unwrap()
        .unwrap();
    let credential = &saved.document().providers[0].credentials[0];
    assert_eq!(
        credential.accio_sgcookie.as_deref(),
        Some("synthetic-new-cookie")
    );
    assert_eq!(credential.id.as_deref(), Some("stable-account"));
    assert_eq!(credential.headers["utdid"], "synthetic-device");
    let stale = send_console_json(
        &fixture.state,
        Method::PUT,
        ENDPOINT,
        body,
        Some(&grant),
        None,
    )
    .await;
    assert_eq!(stale.status(), StatusCode::CONFLICT);
    let clear = send_console_json(
        &fixture.state,
        Method::PUT,
        ENDPOINT,
        candidate(&fixture, &active(), "clear"),
        Some(&grant),
        None,
    )
    .await;
    assert_eq!(clear.status(), StatusCode::OK);
    assert!(fixture
        .state
        .route_config_runtime
        .as_ref()
        .unwrap()
        .coordinator()
        .unwrap()
        .load_revision(fixture.state.route_config.snapshot().revision().id())
        .unwrap()
        .unwrap()
        .document()
        .providers[0]
        .credentials[0]
        .accio_sgcookie
        .is_none());
}

async fn rejects_retargeted_keep_and_embedded_secret() {
    let fixture = ConsoleStateFixture::new(active(), true);
    let original = fixture
        .state
        .route_config
        .snapshot()
        .revision()
        .id()
        .to_owned();
    let grant = grant_console_secret_access(&fixture.state).await;
    for field in ["base_url", "refresh_endpoint", "refresh_client_id"] {
        let mut body = candidate(&fixture, &active(), "keep");
        body["document"]["providers"][0]["credentials"][0][field] =
            json!(if field == "refresh_client_id" {
                "another-client"
            } else {
                "https://foreign.invalid"
            });
        let response = send_console_json(
            &fixture.state,
            Method::PUT,
            ENDPOINT,
            body,
            Some(&grant),
            None,
        )
        .await;
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        let body = parse_json(response).await;
        assert_eq!(body["error"]["code"], "secret_keep_destination_changed");
        assert!(!body.to_string().contains("synthetic-old-cookie"));
    }
    let mut embedded = candidate(&fixture, &active(), "keep");
    embedded["document"]["providers"][0]["credentials"][0]["accio_sgcookie"] =
        json!("embedded-cookie");
    let response = send_console_json(
        &fixture.state,
        Method::PUT,
        ENDPOINT,
        embedded,
        Some(&grant),
        None,
    )
    .await;
    assert_eq!(response.status(), StatusCode::UNPROCESSABLE_ENTITY);
    let body = parse_json(response).await;
    assert_eq!(body["error"]["code"], "secret_value_must_use_patch");
    assert!(!body.to_string().contains("embedded-cookie"));
    assert_eq!(
        fixture.state.route_config.snapshot().revision().id(),
        original
    );
}

#[test]
fn accio_rotation_secret_uses_grant_origin_revision_cas_and_clear() {
    run(replaces_and_clears);
}
#[test]
fn accio_secret_keep_cannot_change_audience_and_draft_cannot_embed_secret() {
    run(rejects_retargeted_keep_and_embedded_secret);
}

async fn credential_only_and_product_transitions() {
    for into_accio in [false, true] {
        let mut document = active();
        document.providers[0].credentials[0].accio_sgcookie = None;
        if into_accio {
            document.providers[0].preset = Some("openai".into());
            document.providers[0].base_url = "https://generic.invalid/v1".into();
            document.providers[0].credentials[0].refresh_endpoint =
                Some("https://generic.invalid/token".into());
        }
        let fixture = ConsoleStateFixture::new(document.clone(), true);
        let grant = grant_console_secret_access(&fixture.state).await;
        let mut provider_level = candidate(&fixture, &document, "keep");
        provider_level["secretPatches"].as_array_mut().unwrap().push(json!({"path":"/providers/0/accio_sgcookie","operation":"replace","value":"synthetic-cookie"}));
        let response = send_console_json(
            &fixture.state,
            Method::PUT,
            ENDPOINT,
            provider_level,
            Some(&grant),
            None,
        )
        .await;
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        assert_eq!(
            parse_json(response).await["error"]["code"],
            "secret_path_outside_schema"
        );
        let mut body = candidate(&fixture, &document, "keep");
        body["document"]["providers"][0]["preset"] =
            json!(if into_accio { "accio" } else { "openai" });
        body["document"]["providers"][0]["base_url"] = json!(if into_accio {
            "https://phoenix-gw.alibaba.com"
        } else {
            "https://generic.invalid/v1"
        });
        body["document"]["providers"][0]["credentials"][0]["refresh_endpoint"] =
            json!(if into_accio {
                "https://phoenix-gw.alibaba.com/api/auth/refresh_token"
            } else {
                "https://generic.invalid/token"
            });
        let response = send_console_json(
            &fixture.state,
            Method::PUT,
            ENDPOINT,
            body,
            Some(&grant),
            None,
        )
        .await;
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        assert_eq!(
            parse_json(response).await["error"]["code"],
            "secret_keep_destination_changed"
        );
    }
}

#[test]
fn accio_legacy_without_sg_and_product_transitions_keep_secret_bindings() {
    run(credential_only_and_product_transitions);
}
