//! Real management router; no credential, browser launch or live OAuth request.
use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use neuro_gateway::http::router::build_router;
use serde_json::{json, Value};
use tower::ServiceExt;
#[path = "console_contract_support/mod.rs"]
mod support;
const ROOT: &str = "/v1/internal/gateway/console/grok-auth-sessions";

fn request(path: &str, body: Value, grant: Option<&str>) -> Request<Body> {
    let mut request = Request::post(path)
        .header("x-management-token", support::MANAGEMENT_TOKEN)
        .header("content-type", "application/json");
    if let Some(grant) = grant {
        request = request.header("x-secret-grant", grant);
    }
    request.body(Body::from(body.to_string())).unwrap()
}
fn document() -> neuro_gateway::routing::config::RouteConfigYaml {
    serde_json::from_value(json!({"providers":[{"id":"build","preset":"grok-oauth",
        "base_url":"https://cli-chat-proxy.grok.com/v1","explicit_credentials_only":true,"credentials":[]}]})).unwrap()
}
fn input(revision: &str) -> Value {
    json!({"providerId":"build","expectedRevision":revision,"clientId":"synthetic-explicit-client",
        "scopes":["openid","offline_access"],"redirectUri":"http://127.0.0.1:34567/callback"})
}
#[tokio::test]
async fn grok_login_uses_management_grant_saved_target_revision_and_no_store() {
    let fixture = support::ConsoleStateFixture::new(document(), true);
    let revision = fixture
        .state
        .route_config
        .snapshot()
        .revision()
        .id()
        .to_string();
    let app = build_router(fixture.state.clone());
    let grant = support::grant_console_secret_access(&fixture.state).await;
    let response = app
        .clone()
        .oneshot(request(ROOT, input(&revision), Some(&grant)))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(response.headers()["cache-control"], "no-store");
    let body = support::parse_json(response).await;
    assert_eq!(body["session"]["status"], "waiting_user");
    assert_eq!(body["session"]["expectedRevision"], revision);
    let authorization = body["session"]["authorizationUrl"].as_str().unwrap();
    assert!(authorization.starts_with("https://auth.x.ai/oauth2/authorize?"));
    assert!(authorization.contains("client_id=synthetic-explicit-client"));
    let id = body["session"]["id"].as_str().unwrap();
    let response = app
        .clone()
        .oneshot(request(ROOT, input(&revision), None))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::FORBIDDEN);
    for body in [
        input("stale"),
        {
            let mut body = input(&revision);
            body["providerId"] = json!("missing");
            body
        },
        {
            let mut body = input(&revision);
            body["issuer"] = json!("https://evil.invalid");
            body
        },
    ] {
        let response = app
            .clone()
            .oneshot(request(ROOT, body, Some(&grant)))
            .await
            .unwrap();
        assert!(!response.status().is_success());
    }
    let path = format!("{ROOT}/{id}");
    for (header, value) in [
        ("host", "other.invalid"),
        ("x-gateway-client-ip", "127.0.0.2"),
        ("x-management-token", "wrong"),
    ] {
        let mut probe = Request::get(&path)
            .header("x-management-token", support::MANAGEMENT_TOKEN)
            .body(Body::empty())
            .unwrap();
        probe.headers_mut().insert(header, value.parse().unwrap());
        let response = app.clone().oneshot(probe).await.unwrap();
        assert!(!response.status().is_success());
    }
    let response = app
        .clone()
        .oneshot(
            Request::get(&path)
                .header("x-management-token", support::MANAGEMENT_TOKEN)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    for action in ["complete", "cancel", "import"] {
        let response = app
            .clone()
            .oneshot(request(
                &format!("{path}/{action}"),
                json!({"expectedRevision":revision}),
                None,
            ))
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::FORBIDDEN);
    }
    let response=app.clone().oneshot(request(&format!("{path}/complete"),
        json!({"expectedRevision":revision,"callbackUrl":"https://evil.invalid/callback?code=private&state=private"}),Some(&grant))).await.unwrap();
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    assert!(!support::parse_json(response)
        .await
        .to_string()
        .contains("private"));
    let response = app
        .oneshot(request(
            &format!("{path}/cancel"),
            json!({"expectedRevision":revision}),
            Some(&grant),
        ))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let view = support::parse_json(response).await;
    assert_eq!(view["session"]["status"], "cancelled");
    assert!(!view.to_string().contains("access_token"));
    assert_eq!(
        fixture.state.route_config.snapshot().revision().id(),
        revision
    );
}

#[tokio::test]
async fn grok_start_rejects_other_products_readonly_runtime_and_untrusted_client_inputs() {
    let mut doc = document();
    let api=serde_json::from_value(json!({"id":"api","preset":"xai-api","base_url":"https://api.x.ai/v1","api_key":"synthetic-key"})).unwrap();
    doc.providers.push(api);
    let fixture = support::ConsoleStateFixture::new(doc, true);
    let revision = fixture
        .state
        .route_config
        .snapshot()
        .revision()
        .id()
        .to_owned();
    let grant = support::grant_console_secret_access(&fixture.state).await;
    let app = build_router(fixture.state.clone());
    for (field, value) in [
        ("providerId", json!("api")),
        ("credentialId", json!("missing")),
        ("clientId", json!("")),
        ("scopes", json!(["openid", "openid"])),
        ("redirectUri", json!("https://evil.invalid/callback")),
        ("requiredPrincipal", json!({"kind":"Team","id":"bad id"})),
        ("tokenEndpoint", json!("https://evil.invalid/token")),
    ] {
        let mut body = input(&revision);
        body[field] = value;
        let response = app
            .clone()
            .oneshot(request(ROOT, body, Some(&grant)))
            .await
            .unwrap();
        assert!(!response.status().is_success(), "field={field}");
    }
    let readonly = support::ConsoleStateFixture::new(document(), false);
    let grant = support::grant_console_secret_access(&readonly.state).await;
    let revision = readonly
        .state
        .route_config
        .snapshot()
        .revision()
        .id()
        .to_owned();
    let response = build_router(readonly.state.clone())
        .oneshot(request(ROOT, input(&revision), Some(&grant)))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
}
