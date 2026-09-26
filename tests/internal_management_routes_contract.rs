//! Management admission must precede storage, provider and mutation work.

use axum::body::Body;
use axum::http::{Method, Request, StatusCode};
use neuro_gateway::http::router::build_router;
use std::sync::Arc;
use tower::ServiceExt;

#[path = "console_contract_support/mod.rs"]
mod support;
use support::{document, parse_json, ConsoleStateFixture, MANAGEMENT_TOKEN};

const READ_ROUTES: &[&str] = &[
    "model-associations",
    "costs",
    "pressure",
    "requests",
    "requests/audit-id",
    "requests/by-response/response-id",
    "requests/audit-id/artifacts",
    "requests/by-response/response-id/artifacts",
    "requests/summary",
    "analysis/samples",
    "analysis/summary",
    "analysis/prompt-cache/summary",
    "analysis/prompt-cache/trend-report",
    "analysis/export",
    "analysis/exports",
    "analysis/exports/summary",
    "analysis/exports/baseline-report",
    "analysis/exports/timeline-report",
    "analysis/exports/trend-report",
    "analysis/exports/anomaly-report",
    "analysis/exports/diff",
    "analysis/exports/export-id",
    "analysis/provider-routing/summary",
    "analysis/provider-routing/anomaly-report",
    "analysis/rate-limit-hotspots",
    "analysis/rate-limit-hotspots/snapshots",
    "analysis/rate-limit-hotspots/snapshots/summary",
    "analysis/rate-limit-hotspots/snapshots/trend-report",
    "analysis/rate-limit-hotspots/snapshots/snapshot-id",
    "analysis/rate-limit-hotspots/trend-report",
    "analysis/rate-limit-hotspots/anomaly-report",
    "analysis/rate-limit-hotspots/anomaly-snapshots",
    "analysis/rate-limit-hotspots/anomaly-snapshots/snapshot-id",
    "analysis/anomaly-policies",
    "analysis/anomaly-policies/summary",
    "analysis/anomaly-incidents",
    "analysis/anomaly-incidents/alert-queue",
    "analysis/anomaly-incidents/incident-id/history",
    "analysis/anomaly-incidents/incident-id/remediation-plan",
    "analysis/anomaly-incidents/incident-id/remediation-runs",
    "analysis/anomaly-incidents/summary",
    "analysis/remediation-runs",
    "analysis/remediation-queue",
    "analysis/remediation-runs/summary",
    "analysis/remediation-runs/effectiveness",
    "analysis/remediation-runs/effectiveness/snapshots",
    "analysis/remediation-runs/effectiveness/snapshots/summary",
    "analysis/remediation-runs/effectiveness/snapshots/trend-report",
    "analysis/remediation-runs/effectiveness/snapshots/anomaly-report",
    "analysis/remediation-runs/effectiveness/snapshots/anomaly-snapshots",
    "analysis/remediation-runs/effectiveness/snapshots/anomaly-snapshots/snapshot-id",
    "analysis/remediation-runs/effectiveness/snapshots/snapshot-id",
    "analysis/remediation-runs/run-id/impact",
    "provider-accounts",
    "provider-accounts/provider-id",
    "provider-accounts/provider-id/model-tiering",
    "provider-accounts/provider-id/quota",
    "provider-quotas/provider-id",
    "provider-inventory",
];

#[tokio::test]
async fn all_management_reads_reject_missing_or_public_credentials_before_io() {
    let fixture = ConsoleStateFixture::new(document("managed", "model", "synthetic-secret"), false);
    let router = build_router(Arc::clone(&fixture.state));
    for route in READ_ROUTES {
        for credential in [None, fixture.state.config.gateway_api_key.as_deref()] {
            let mut request = Request::get(format!("/v1/internal/gateway/{route}"))
                .header("x-operator-user-id", "forged-operator")
                .header("x-user-id", "forged-user");
            if let Some(credential) = credential {
                request = request.header("x-management-token", credential);
            }
            let response = router
                .clone()
                .oneshot(request.body(Body::empty()).unwrap())
                .await
                .unwrap();
            assert_eq!(response.status(), StatusCode::UNAUTHORIZED, "{route}");
            let body = parse_json(response).await;
            assert_eq!(
                body["error"]["message"], "Management token is required",
                "{route}"
            );
        }
    }
}

#[tokio::test]
async fn management_mutations_reject_forged_actors_before_side_effects() {
    let fixture = ConsoleStateFixture::new(document("managed", "model", "synthetic-secret"), false);
    let router = build_router(Arc::clone(&fixture.state));
    let account = serde_json::json!({
        "label": "Synthetic", "adapter": "openai", "protocolFamily": "openai",
        "payload": {"api_key": "synthetic-secret"}
    });
    let cases = [
        (Method::POST, "analysis/export", serde_json::json!({})),
        (
            Method::POST,
            "analysis/anomaly-incidents/sync",
            serde_json::json!({}),
        ),
        (
            Method::POST,
            "analysis/anomaly-incidents/incident-id/alert-dispatch",
            serde_json::json!({}),
        ),
        (
            Method::POST,
            "analysis/anomaly-incidents/incident-id/acknowledge",
            serde_json::json!({}),
        ),
        (
            Method::POST,
            "analysis/anomaly-incidents/incident-id/resolve",
            serde_json::json!({}),
        ),
        (
            Method::POST,
            "analysis/anomaly-policies/sweep-sync",
            serde_json::json!({}),
        ),
        (
            Method::POST,
            "analysis/remediation-runs/effectiveness/snapshot",
            serde_json::json!({}),
        ),
        (
            Method::POST,
            "analysis/rate-limit-hotspots/snapshot",
            serde_json::json!({}),
        ),
        (Method::POST, "provider-accounts", account.clone()),
        (Method::POST, "provider-accounts/provider-id", account),
        (
            Method::DELETE,
            "provider-accounts/provider-id",
            serde_json::json!({}),
        ),
        (
            Method::POST,
            "provider-accounts/provider-id/model-tiering",
            serde_json::json!({
                "model": "test-model", "platformTier": "standard", "enabled": false
            }),
        ),
        (
            Method::POST,
            "provider-accounts/provider-id/model-pricing",
            serde_json::json!({"entries": []}),
        ),
        (
            Method::POST,
            "provider-accounts/source-profile/backfill",
            serde_json::json!({}),
        ),
        (
            Method::POST,
            "provider-accounts/provider-id/quota",
            serde_json::json!({}),
        ),
        (
            Method::POST,
            "provider-accounts/provider-id/probe",
            serde_json::json!({}),
        ),
        (
            Method::POST,
            "provider-accounts/sweep-cooling",
            serde_json::json!({}),
        ),
    ];
    for (method, route, body) in cases {
        let response = router
            .clone()
            .oneshot(
                Request::builder()
                    .method(method)
                    .uri(format!("/v1/internal/gateway/{route}"))
                    .header("content-type", "application/json")
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
async fn supported_management_tokens_preserve_unconfigured_database_response() {
    let fixture = ConsoleStateFixture::new(document("managed", "model", "synthetic-secret"), false);
    let router = build_router(Arc::clone(&fixture.state));
    let bearer = format!("Bearer {MANAGEMENT_TOKEN}");
    for route in [
        "requests",
        "analysis/anomaly-incidents",
        "provider-accounts",
        "provider-inventory",
    ] {
        for (header, value) in [
            ("x-management-token", MANAGEMENT_TOKEN),
            ("x-internal-api-key", MANAGEMENT_TOKEN),
            ("authorization", bearer.as_str()),
        ] {
            let response = router
                .clone()
                .oneshot(
                    Request::get(format!("/v1/internal/gateway/{route}"))
                        .header(header, value)
                        .body(Body::empty())
                        .unwrap(),
                )
                .await
                .unwrap();
            assert_eq!(
                response.status(),
                StatusCode::SERVICE_UNAVAILABLE,
                "{route} {header}"
            );
            assert!(parse_json(response).await["error"]["message"].is_string());
        }
    }
}

#[tokio::test]
async fn invalid_explicit_management_token_cannot_fall_back_to_valid_bearer() {
    let fixture = ConsoleStateFixture::new(document("managed", "model", "synthetic-secret"), false);
    let router = build_router(Arc::clone(&fixture.state));
    for route in ["requests", "provider-accounts"] {
        let response = router
            .clone()
            .oneshot(
                Request::get(format!("/v1/internal/gateway/{route}"))
                    .header("x-management-token", "invalid-token")
                    .header("authorization", format!("Bearer {MANAGEMENT_TOKEN}"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED, "{route}");
    }
}
