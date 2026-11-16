//! Public management entry exists independently of real credentials and the optional wire leaf.
use axum::http::{Method, StatusCode};
use neuro_gateway::routing::config::RouteConfigYaml;
use serde_json::json;
#[path = "console_contract_support/mod.rs"]
mod support;

const ROOT: &str = "/v1/internal/gateway/console/accio-auth-sessions";

#[tokio::test]
async fn saved_accio_target_requires_secret_consent_and_returns_only_a_bound_manual_attempt() {
    let document: RouteConfigYaml = serde_json::from_value(json!({"providers":[{
        "id":"operator-pool", "preset":"accio", "base_url":"https://phoenix-gw.alibaba.com",
        "explicit_credentials_only":true,"credentials":[]
    }]}))
    .unwrap();
    let fixture = support::ConsoleStateFixture::new(document, true);
    let revision = fixture
        .state
        .route_config
        .snapshot()
        .revision()
        .id()
        .to_owned();
    let body = json!({"providerId":"operator-pool","expectedRevision":revision,
        "clientId":"synthetic-explicit-client","loginSite":"china","callbackPort":34567,
        "utdid":"synthetic-explicit-device"});
    let grant = support::grant_console_secret_access(&fixture.state).await;
    let accepted = support::send_console_json(
        &fixture.state,
        Method::POST,
        ROOT,
        body.clone(),
        Some(&grant),
        None,
    )
    .await;
    assert_eq!(accepted.status(), StatusCode::OK);
    assert_eq!(accepted.headers()["cache-control"], "no-store");
    let response = support::parse_json(accepted).await;
    assert_eq!(response["session"]["providerId"], "operator-pool");
    assert_eq!(response["session"]["expectedRevision"], revision);
    assert_eq!(response["session"]["status"], "waiting_user");
    assert!(response["session"].get("identityVerified").is_none());
    assert!(!response.to_string().contains("synthetic-explicit-device"));
    assert!(response["session"]["credentialId"].is_null());
    let denied =
        support::send_console_json(&fixture.state, Method::POST, ROOT, body.clone(), None, None)
            .await;
    assert_eq!(denied.status(), StatusCode::FORBIDDEN);
    let foreign = support::send_console_json(
        &fixture.state,
        Method::POST,
        ROOT,
        body.clone(),
        Some(&grant),
        Some("https://foreign.invalid"),
    )
    .await;
    assert_eq!(foreign.status(), StatusCode::FORBIDDEN);
    let mut stale = body;
    stale["expectedRevision"] = json!("not-the-active-revision");
    let conflict = support::send_console_json(
        &fixture.state,
        Method::POST,
        ROOT,
        stale,
        Some(&grant),
        None,
    )
    .await;
    assert_eq!(conflict.status(), StatusCode::CONFLICT);
}

#[tokio::test]
async fn provider_level_account_material_cannot_be_inherited_by_a_new_authorization() {
    for (field, value) in [
        ("headers", json!({"Cookie":"synthetic-old-account-cookie"})),
        (
            "headers",
            json!({"AUTHORIZATION":"synthetic-old-authorization"}),
        ),
        ("headers", json!({"x-utdid":"synthetic-old-account-device"})),
        ("extra_body", json!({"sgcookie":"synthetic-old-account-sg"})),
        ("extra_body", json!({"tenant":"synthetic-old-tenant"})),
        ("extra_body", json!({"empid":"synthetic-old-account-id"})),
        ("extra_body", json!({"empId":"synthetic-old-account-id"})),
        (
            "extra_body",
            json!({"accountId":"synthetic-old-account-id"}),
        ),
        (
            "extra_body",
            json!({"account_id":"synthetic-old-account-id"}),
        ),
        ("extra_body", json!({"tenantId":"synthetic-old-tenant"})),
        ("extra_body", json!({"tenant_id":"synthetic-old-tenant"})),
        ("auth_token", json!("synthetic-old-account-auth")),
    ] {
        let mut provider = json!({"id":"operator-pool","preset":"accio",
            "base_url":"https://phoenix-gw.alibaba.com","explicit_credentials_only":true,"credentials":[]});
        provider[field] = value;
        let document: RouteConfigYaml =
            serde_json::from_value(json!({"providers":[provider]})).unwrap();
        let fixture = support::ConsoleStateFixture::new(document, true);
        let revision = fixture
            .state
            .route_config
            .snapshot()
            .revision()
            .id()
            .to_owned();
        let body = json!({"providerId":"operator-pool","expectedRevision":revision,
            "clientId":"synthetic-explicit-client","loginSite":"china","callbackPort":34567,
            "utdid":"synthetic-new-account-device"});
        let grant = support::grant_console_secret_access(&fixture.state).await;
        let response = support::send_console_json(
            &fixture.state,
            Method::POST,
            ROOT,
            body,
            Some(&grant),
            None,
        )
        .await;
        assert!(
            response.status().is_client_error(),
            "provider {field}: {}",
            response.status()
        );
        assert_eq!(
            fixture.state.route_config.snapshot().revision().id(),
            revision
        );
    }
}

#[tokio::test]
async fn missing_writable_runtime_and_draining_runtime_reject_new_attempts() {
    for with_runtime in [false, true] {
        let document: RouteConfigYaml = serde_json::from_value(json!({"providers":[{
            "id":"operator-pool","preset":"accio","base_url":"https://phoenix-gw.alibaba.com",
            "explicit_credentials_only":true,"credentials":[]
        }]}))
        .unwrap();
        let fixture = support::ConsoleStateFixture::new(document, with_runtime);
        // Consent precedes drain; normal middleware rejects all non-exempt requests afterwards.
        let grant = support::grant_console_secret_access(&fixture.state).await;
        if with_runtime {
            fixture.state.lifecycle.begin_drain("synthetic-test");
        }
        let revision = fixture
            .state
            .route_config
            .snapshot()
            .revision()
            .id()
            .to_owned();
        let response = support::send_console_json(&fixture.state, Method::POST, ROOT,
            json!({"providerId":"operator-pool","expectedRevision":revision,
                "clientId":"synthetic-explicit-client","loginSite":"china","callbackPort":34567,"utdid":"synthetic-device"}), Some(&grant), None).await;
        assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
        assert_eq!(
            support::parse_json(response).await["error"]["code"],
            if with_runtime {
                "gateway_draining"
            } else {
                "console_mutation_not_supported"
            }
        );
        assert_eq!(
            fixture.state.route_config.snapshot().revision().id(),
            revision
        );
    }
}
