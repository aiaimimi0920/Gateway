//! Sensitive validation and commits require the exact context-bound secret grant.

use axum::http::{Method, StatusCode};
use neuro_gateway::console::secrets::redact_route_document;

#[path = "console_contract_support/mod.rs"]
mod support;
use support::*;

#[tokio::test]
async fn route_config_console_validate_requires_exact_secret_grant_for_sensitive_inputs() {
    let active = document("managed", "old-model", "live-secret");
    let fixture = ConsoleStateFixture::new(active.clone(), true);
    let redacted = redact_route_document(&active).unwrap().document;
    let cases = [
        (
            "raw secret",
            document("managed", "old-model", "raw-request-secret"),
            serde_json::json!([]),
        ),
        (
            "replace patch",
            redacted.clone(),
            serde_json::json!([
                {
                    "path": "/providers/0/api_key",
                    "operation": "replace",
                    "value": "replacement-secret"
                }
            ]),
        ),
        (
            "clear patch",
            redacted,
            serde_json::json!([
                { "path": "/providers/0/api_key", "operation": "clear" }
            ]),
        ),
    ];

    for (label, draft, secret_patches) in cases {
        let body = serde_json::json!({
            "document": draft,
            "secretPatches": secret_patches,
        });
        for supplied_grant in [None, Some("grant-does-not-exist")] {
            let response = send_console_json(
                &fixture.state,
                Method::POST,
                "/v1/internal/gateway/console/route-config/validate",
                body.clone(),
                supplied_grant,
                None,
            )
            .await;
            assert_eq!(
                response.status(),
                StatusCode::FORBIDDEN,
                "{label} accepted a missing or unknown secret grant"
            );
            let response_body = parse_json(response).await;
            assert_eq!(
                response_body["error"]["code"], "console_secret_access_required",
                "{label} returned the wrong authorization error"
            );
        }

        let exact_grant = grant_console_secret_access(&fixture.state).await;
        let wrong_origin = send_console_json(
            &fixture.state,
            Method::POST,
            "/v1/internal/gateway/console/route-config/validate",
            body.clone(),
            Some(&exact_grant),
            Some("https://different-origin.example"),
        )
        .await;
        assert_eq!(
            wrong_origin.status(),
            StatusCode::FORBIDDEN,
            "{label} accepted a grant bound to another origin"
        );

        let exact = send_console_json(
            &fixture.state,
            Method::POST,
            "/v1/internal/gateway/console/route-config/validate",
            body,
            Some(&exact_grant),
            None,
        )
        .await;
        if label == "raw secret" {
            assert_eq!(exact.status(), StatusCode::UNPROCESSABLE_ENTITY);
            let response_body = parse_json(exact).await;
            assert_eq!(
                response_body["error"]["code"],
                "secret_value_must_use_patch"
            );
        } else {
            assert_eq!(
                exact.status(),
                StatusCode::OK,
                "{label} rejected the exact grant"
            );
        }
    }
}

#[tokio::test]
async fn route_config_console_commit_requires_exact_secret_grant_for_sensitive_inputs() {
    for (label, kind) in [
        ("raw secret", SensitiveCommitKind::Raw),
        ("replace patch", SensitiveCommitKind::Replace),
        ("clear patch", SensitiveCommitKind::Clear),
    ] {
        for supplied_grant in [None, Some("grant-does-not-exist")] {
            let active = document("managed", "old-model", "live-secret");
            let fixture = ConsoleStateFixture::new(active.clone(), true);
            let body = sensitive_commit_body(&fixture, &active, &kind);
            let response = send_console_json(
                &fixture.state,
                Method::PUT,
                "/v1/internal/gateway/console/route-config",
                body,
                supplied_grant,
                None,
            )
            .await;
            assert_eq!(
                response.status(),
                StatusCode::FORBIDDEN,
                "{label} accepted a missing or unknown secret grant"
            );
            let response_body = parse_json(response).await;
            assert_eq!(
                response_body["error"]["code"], "console_secret_access_required",
                "{label} returned the wrong authorization error"
            );
            assert_eq!(
                fixture.state.route_config.get_providers()[0]
                    .payload
                    .api_key,
                "live-secret",
                "{label} mutated the active secret before authorization"
            );
        }

        let active = document("managed", "old-model", "live-secret");
        let fixture = ConsoleStateFixture::new(active.clone(), true);
        let exact_grant = grant_console_secret_access(&fixture.state).await;
        let body = sensitive_commit_body(&fixture, &active, &kind);
        let exact = send_console_json(
            &fixture.state,
            Method::PUT,
            "/v1/internal/gateway/console/route-config",
            body,
            Some(&exact_grant),
            None,
        )
        .await;
        match kind {
            SensitiveCommitKind::Raw => {
                assert_eq!(exact.status(), StatusCode::UNPROCESSABLE_ENTITY);
                let response_body = parse_json(exact).await;
                assert_eq!(
                    response_body["error"]["code"],
                    "secret_value_must_use_patch"
                );
                assert_eq!(
                    fixture.state.route_config.get_providers()[0]
                        .payload
                        .api_key,
                    "live-secret"
                );
            }
            SensitiveCommitKind::Replace => {
                assert_eq!(exact.status(), StatusCode::OK);
                assert_eq!(
                    fixture.state.route_config.get_providers()[0]
                        .payload
                        .api_key,
                    "replacement-secret"
                );
            }
            SensitiveCommitKind::Clear => {
                assert_eq!(exact.status(), StatusCode::OK);
                assert!(fixture.state.route_config.get_providers()[0]
                    .payload
                    .api_key
                    .is_empty());
            }
        }
    }
}
