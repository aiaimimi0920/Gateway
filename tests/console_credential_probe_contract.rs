//! Credential and provider probes use controlled loopback upstreams and secret gates.

use axum::body::Body;
use axum::http::{Request, StatusCode};
use neuro_gateway::http::router::build_router;
use neuro_gateway::routing::config::RouteConfigYaml;
use std::sync::Arc;
use tower::ServiceExt;

#[path = "console_contract_support/mod.rs"]
mod support;
use support::*;

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
    assert_eq!(
        explicit_body["result"]["probePoint"],
        format!("GET {base_url}/models")
    );
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
    assert_eq!(
        default_body["result"]["probePoint"],
        format!("GET {base_url}/models")
    );

    let serialized = serde_json::to_string(&(explicit_body, default_body)).unwrap();
    assert!(!serialized.contains("explicit-secret"));
    assert!(!serialized.contains("default-secret"));
    server.abort();
}

#[tokio::test]
async fn provider_probe_tests_all_accounts_and_aggregates_real_results() {
    let (base_url, server) = spawn_probe_server(ProbeServerState {
        accepted_authorizations: vec!["Bearer working-secret".to_string()],
        status: StatusCode::OK,
        body: r#"{"data":[]}"#.to_string(),
    })
    .await;
    let document: RouteConfigYaml = serde_yaml::from_str(&format!(
        r#"
providers:
  - id: aggregate-provider
    adapter: openai_compatible
    base_url: "{base_url}"
    api_key: provider-unused
    credentials:
      - id: working-account
        api_key: working-secret
      - id: failing-account
        api_key: rejected-secret
      - id: disabled-account
        api_key: disabled-secret
        enabled: false
model_routes: []
aliases: {{}}
"#
    ))
    .unwrap();
    let fixture = ConsoleStateFixture::new(document, false);
    let secret_grant = grant_console_secret_access(&fixture.state).await;

    let response = probe_provider(&fixture.state, &secret_grant, "aggregate-provider").await;
    assert_eq!(response.status(), StatusCode::OK);
    let body = parse_json(response).await;
    assert_eq!(body["result"]["providerId"], "aggregate-provider");
    assert_eq!(body["result"]["status"], "failed");
    assert_eq!(body["result"]["totalCount"], 3);
    assert_eq!(body["result"]["passedCount"], 1);
    assert_eq!(body["result"]["failedCount"], 1);
    assert_eq!(body["result"]["unsupportedCount"], 1);
    let results = body["result"]["results"]
        .as_array()
        .expect("provider probe results");
    assert_eq!(results.len(), 3);
    assert!(results.iter().all(|result| {
        result["probePoint"] == format!("GET {base_url}/models")
            && result["checkedAt"].as_str().is_some()
    }));
    assert!(results.iter().any(|result| {
        result["credentialId"] == "working-account" && result["status"] == "passed"
    }));
    assert!(results.iter().any(|result| {
        result["credentialId"] == "failing-account" && result["status"] == "failed"
    }));
    assert!(results.iter().any(|result| {
        result["credentialId"] == "disabled-account" && result["status"] == "unsupported"
    }));
    let serialized = serde_json::to_string(&body).unwrap();
    for secret in ["working-secret", "rejected-secret", "disabled-secret"] {
        assert!(
            !serialized.contains(secret),
            "provider probe leaked {secret}"
        );
    }
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
