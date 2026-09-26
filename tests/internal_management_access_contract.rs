//! Admission for credential, access-key and project management routes.

use axum::body::Body;
use axum::http::{Method, Request, StatusCode};
use neuro_gateway::http::router::build_router;
use serde_json::{json, Value};
use std::sync::Arc;
use tower::ServiceExt;

#[path = "console_contract_support/mod.rs"]
mod support;
use support::{document, parse_json, ConsoleStateFixture, MANAGEMENT_TOKEN};

const READ_ROUTES: &[&str] = &[
    "provider-credentials?maskSecrets=false",
    "provider-credentials/credential-id?maskSecrets=false",
    "provider-credentials/credential-id/quota",
    "provider-credentials/folder-sync/status",
    "provider-accounts/provider-id/credentials?maskSecrets=false",
    "access/catalog?includeTokens=true",
    "access/keys/key-id/balance",
    "access/preview/candidates?accessKeyId=key-id&model=model-a&endpointKind=responses",
    "access/preview/route-decision?accessKeyId=key-id&model=model-a&endpointKind=responses",
    "access/affinity?accessKeyId=key-id&model=model-a",
    "projects/project-id/api-access",
    "projects/project-id/prompt-cache/summary",
    "projects/project-id/prompt-cache/trend-report",
    "provider-credential-model-states",
    "usage-aggregates",
    "usage-aggregates/summary",
];

fn mutation_cases() -> Vec<(Method, &'static str, Value)> {
    let credential = json!({"label": "Synthetic", "payload": {"apiKey": "synthetic-secret"}});
    let capability = json!({
        "providerAccountId": "provider-id", "modelCode": "model-a",
        "endpointKind": "responses", "enabled": false
    });
    let platform = json!({
        "providerCapabilityId": "capability-id", "modelCode": "model-a",
        "endpointKind": "responses", "platformTier": "low", "status": "disabled",
        "operatorWeight": 0, "routingPriority": 0, "enabledForSale": false
    });
    let bundle = json!({"slug": "bundle-a", "displayName": "Bundle A", "status": "active"});
    let key = json!({
        "ownerType": "user", "ownerId": "user-a", "resolvedProjectId": "project-a",
        "resolvedTenantId": "tenant-a", "keyKind": "normal",
        "publicKeyPrefix": "synthetic", "displayName": "Synthetic"
    });
    vec![
        (Method::POST, "provider-credentials", credential.clone()),
        (
            Method::POST,
            "provider-accounts/provider-id/credentials",
            credential,
        ),
        (Method::PUT, "provider-credentials/credential-id", json!({})),
        (
            Method::DELETE,
            "provider-credentials/credential-id",
            json!({}),
        ),
        (
            Method::POST,
            "provider-credentials/credential-id/quota",
            json!({}),
        ),
        (
            Method::PUT,
            "provider-credentials/folder-sync/status",
            json!({"enabled": true}),
        ),
        (
            Method::POST,
            "provider-credentials/folder-sync/import",
            json!({}),
        ),
        (
            Method::POST,
            "provider-credentials/folder-sync/export",
            json!({}),
        ),
        (
            Method::POST,
            "access/provider-capabilities",
            capability.clone(),
        ),
        (
            Method::POST,
            "access/provider-capabilities/capability-id",
            capability,
        ),
        (Method::POST, "access/platform-access", platform.clone()),
        (Method::POST, "access/platform-access/access-id", platform),
        (Method::POST, "access/bundles", bundle.clone()),
        (Method::POST, "access/bundles/bundle-id", bundle),
        (Method::DELETE, "access/bundles/bundle-id", json!({})),
        (
            Method::POST,
            "access/bundles/bundle-id/items/replace",
            json!({"platformAccessIds": []}),
        ),
        (
            Method::POST,
            "access/bundles/bundle-id/user-keys/ensure",
            json!({"userId": "user-a"}),
        ),
        (Method::POST, "access/keys", key.clone()),
        (Method::POST, "access/keys/key-id", key),
        (Method::DELETE, "access/keys/key-id", json!({})),
        (Method::POST, "access/keys/key-id/rotate", json!({})),
        (Method::POST, "access/keys/key-id/revoke", json!({})),
        (
            Method::POST,
            "access/keys/key-id/balances/adjust",
            json!({"tokenDelta": 0}),
        ),
        (
            Method::POST,
            "access/keys/key-id/aggregate-memberships/replace",
            json!({"memberships": []}),
        ),
        (
            Method::POST,
            "access/affinity",
            json!({"accessKeyId": "key-id", "model": "model-a"}),
        ),
        (
            Method::POST,
            "api-access/resolve",
            json!({"projectId": "project-a"}),
        ),
        (
            Method::POST,
            "api-access/rotate",
            json!({"projectId": "project-a"}),
        ),
        (
            Method::POST,
            "projects/project-id/api-access/rotate",
            json!({}),
        ),
        (
            Method::POST,
            "benefit-projects/ensure",
            json!({"serviceId": "service-a", "userId": "user-a"}),
        ),
        (Method::POST, "user-credentials/issue", issue_body()),
        (
            Method::POST,
            "user-credentials/verify",
            json!({"credentialKey": "synthetic-key"}),
        ),
        (
            Method::POST,
            "user-credentials/revoke",
            json!({"credentialKey": "synthetic-key"}),
        ),
        (
            Method::POST,
            "usage-aggregates/flush",
            json!({"batchSize": 1}),
        ),
    ]
}

fn issue_body() -> Value {
    json!({"userId": "user-a", "credentialType": "api", "durationDays": 1, "scope": ["chat"]})
}

#[tokio::test]
async fn control_reads_reject_missing_and_public_credentials_before_io() {
    let fixture = ConsoleStateFixture::new(document("managed", "model", "synthetic-secret"), false);
    let router = build_router(Arc::clone(&fixture.state));
    for route in READ_ROUTES {
        for credential in [None, fixture.state.config.gateway_api_key.as_deref()] {
            let mut request = Request::get(format!("/v1/internal/gateway/{route}"))
                .header("x-operator-user-id", "forged-operator");
            if let Some(credential) = credential {
                request = request.header("x-management-token", credential);
            }
            let response = router
                .clone()
                .oneshot(request.body(Body::empty()).unwrap())
                .await
                .unwrap();
            assert_eq!(response.status(), StatusCode::UNAUTHORIZED, "{route}");
            assert_eq!(
                parse_json(response).await["error"]["message"],
                "Management token is required"
            );
        }
    }
}

#[tokio::test]
async fn control_mutations_reject_forged_actors_before_writes() {
    let fixture = ConsoleStateFixture::new(document("managed", "model", "synthetic-secret"), false);
    let router = build_router(Arc::clone(&fixture.state));
    for (method, route, body) in mutation_cases() {
        let response = router
            .clone()
            .oneshot(
                Request::builder()
                    .method(method)
                    .uri(format!("/v1/internal/gateway/{route}"))
                    .header("content-type", "application/json")
                    .header("x-user-id", "forged-user")
                    .header("x-operator-user-id", "forged-operator")
                    .body(Body::from(body.to_string()))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED, "{route}");
    }
}

#[tokio::test]
async fn legacy_user_credential_aliases_keep_the_management_gate() {
    let fixture = ConsoleStateFixture::new(document("managed", "model", "synthetic-secret"), false);
    let router = build_router(Arc::clone(&fixture.state));
    for (operation, body) in [
        ("issue", issue_body()),
        ("verify", json!({"credentialKey": "synthetic-key"})),
        ("revoke", json!({"credentialKey": "synthetic-key"})),
    ] {
        let response = router
            .clone()
            .oneshot(
                Request::post(format!("/v1/internal/user-credentials/{operation}"))
                    .header("content-type", "application/json")
                    .body(Body::from(body.to_string()))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED, "{operation}");
    }
}

#[tokio::test]
async fn admitted_control_reads_preserve_the_missing_database_response() {
    let fixture = ConsoleStateFixture::new(document("managed", "model", "synthetic-secret"), false);
    let router = build_router(Arc::clone(&fixture.state));
    for route in [
        "provider-credentials/credential-id",
        "provider-accounts/provider-id/credentials",
        "access/catalog",
        "access/keys/key-id/balance",
        "projects/project-id/api-access",
        "projects/project-id/prompt-cache/summary",
        "provider-credential-model-states",
        "usage-aggregates",
    ] {
        let response = router
            .clone()
            .oneshot(
                Request::get(format!("/v1/internal/gateway/{route}"))
                    .header("x-management-token", MANAGEMENT_TOKEN)
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(
            response.status(),
            StatusCode::SERVICE_UNAVAILABLE,
            "{route}"
        );
    }
}
