//! Management routes preserve authentication, revision and response contracts.

use axum::body::Body;
use axum::http::{Request, StatusCode};
use neuro_gateway::console::secrets::redact_route_document;
use neuro_gateway::http::router::build_router;
use std::sync::Arc;
use tower::ServiceExt;

#[path = "console_contract_support/mod.rs"]
mod support;
use support::*;

#[tokio::test]
async fn route_config_management_route_requires_management_authentication() {
    let fixture = ConsoleStateFixture::new(document("managed", "old-model", "live-secret"), true);
    let response = build_router(Arc::clone(&fixture.state))
        .oneshot(
            Request::get("/v1/internal/gateway/route-config")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn route_config_management_route_returns_redacted_active_document() {
    let active = document("managed", "old-model", "live-secret");
    let fixture = ConsoleStateFixture::new(active.clone(), true);
    let response = build_router(Arc::clone(&fixture.state))
        .oneshot(
            Request::get("/v1/internal/gateway/route-config")
                .header("x-management-token", MANAGEMENT_TOKEN)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body = parse_json(response).await;
    assert_eq!(
        body["routeConfig"]["revision"]["id"].as_str(),
        Some(fixture.state.route_config.snapshot().revision().id())
    );
    assert_eq!(body["routeConfig"]["source"].as_str(), Some("database"));
    assert_eq!(body["routeConfig"]["mutationSupported"], true);
    assert_ne!(
        body["routeConfig"]["document"]["providers"][0]["api_key"].as_str(),
        Some("live-secret")
    );
    assert_eq!(
        redact_route_document(&active).unwrap().secrets.len(),
        body["routeConfig"]["secrets"].as_array().unwrap().len()
    );
}

#[tokio::test]
async fn route_config_console_alias_returns_no_store_and_etag() {
    let fixture = ConsoleStateFixture::new(document("managed", "old-model", "live-secret"), true);
    let revision_id = fixture
        .state
        .route_config
        .snapshot()
        .revision()
        .id()
        .to_string();

    let response = build_router(Arc::clone(&fixture.state))
        .oneshot(
            Request::get("/v1/internal/gateway/console/route-config")
                .header("x-management-token", MANAGEMENT_TOKEN)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(
        response
            .headers()
            .get("cache-control")
            .and_then(|value| value.to_str().ok()),
        Some("no-store")
    );
    assert_eq!(
        response
            .headers()
            .get("etag")
            .and_then(|value| value.to_str().ok()),
        Some(format!("\"{revision_id}\"").as_str())
    );
}

#[tokio::test]
async fn route_config_management_commit_applies_secret_patches_and_updates_runtime() {
    let active = document("managed", "old-model", "live-secret");
    let fixture = ConsoleStateFixture::new(active.clone(), true);
    let current_revision = fixture
        .state
        .route_config
        .snapshot()
        .revision()
        .id()
        .to_string();
    let mut draft = redact_route_document(&active).unwrap().document;
    draft.providers[0].supported_models = vec!["new-model".to_string()];
    draft.model_routes[0].pattern = "new-model".to_string();
    draft
        .aliases
        .insert("answer".to_string(), "new-model".to_string());

    let response = build_router(Arc::clone(&fixture.state))
        .oneshot(
            Request::post("/v1/internal/gateway/route-config")
                .header("x-management-token", MANAGEMENT_TOKEN)
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::json!({
                        "expectedRevision": current_revision,
                        "document": draft,
                        "secretPatches": [
                            { "path": "/providers/0/api_key", "operation": "keep" }
                        ],
                        "message": "update route config"
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body = parse_json(response).await;
    assert_eq!(
        body["routeConfig"]["document"]["aliases"]["answer"].as_str(),
        Some("new-model")
    );
    assert_eq!(
        fixture
            .state
            .route_config
            .snapshot()
            .resolve_alias(Some("answer")),
        Some("new-model".to_string())
    );
    assert_eq!(
        fixture.state.route_config.snapshot().source(),
        neuro_gateway::routing::config::ActiveConfigSource::Redis
    );
}

#[tokio::test]
async fn route_config_console_validate_returns_redacted_candidate_document() {
    let active = document("managed", "old-model", "live-secret");
    let fixture = ConsoleStateFixture::new(active.clone(), true);
    let mut draft = redact_route_document(&active).unwrap().document;
    draft.providers[0].supported_models = vec!["candidate-model".to_string()];
    draft.model_routes[0].pattern = "candidate-model".to_string();
    draft
        .aliases
        .insert("answer".to_string(), "candidate-model".to_string());

    let response = build_router(Arc::clone(&fixture.state))
        .oneshot(
            Request::post("/v1/internal/gateway/console/route-config/validate")
                .header("x-management-token", MANAGEMENT_TOKEN)
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::json!({
                        "document": draft,
                        "secretPatches": [
                            { "path": "/providers/0/api_key", "operation": "keep" }
                        ]
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body = parse_json(response).await;
    assert_eq!(
        body["validation"]["document"]["aliases"]["answer"].as_str(),
        Some("candidate-model")
    );
    assert_eq!(body["validation"]["requiresRepair"], false);
    assert_ne!(
        body["validation"]["document"]["providers"][0]["api_key"].as_str(),
        Some("live-secret")
    );
}

#[tokio::test]
async fn route_config_management_commit_rejects_stale_revision_with_conflict() {
    let active = document("managed", "old-model", "live-secret");
    let fixture = ConsoleStateFixture::new(active.clone(), true);
    let mut draft = redact_route_document(&active).unwrap().document;
    draft
        .aliases
        .insert("answer".to_string(), "different-model".to_string());

    let response = build_router(Arc::clone(&fixture.state))
        .oneshot(
            Request::post("/v1/internal/gateway/route-config")
                .header("x-management-token", MANAGEMENT_TOKEN)
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::json!({
                        "expectedRevision": "r0-deadbeefcafe",
                        "document": draft,
                        "secretPatches": [
                            { "path": "/providers/0/api_key", "operation": "keep" }
                        ]
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::CONFLICT);
    let body = parse_json(response).await;
    assert_eq!(
        body["error"]["code"].as_str(),
        Some("console_revision_conflict")
    );
}

#[tokio::test]
async fn route_config_management_commit_reports_runtime_unavailable_when_not_configured() {
    let active = document("managed", "old-model", "live-secret");
    let fixture = ConsoleStateFixture::new(active.clone(), false);
    let mut draft = redact_route_document(&active).unwrap().document;
    draft.providers[0].supported_models = vec!["new-model".to_string()];
    draft.model_routes[0].pattern = "new-model".to_string();
    let response = build_router(Arc::clone(&fixture.state))
        .oneshot(
            Request::post("/v1/internal/gateway/route-config")
                .header("x-management-token", MANAGEMENT_TOKEN)
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::json!({
                        "expectedRevision": fixture.state.route_config.snapshot().revision().id(),
                        "document": draft,
                        "secretPatches": [
                            { "path": "/providers/0/api_key", "operation": "keep" }
                        ]
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
    let body = parse_json(response).await;
    assert_eq!(
        body["error"]["code"].as_str(),
        Some("console_runtime_unavailable")
    );
}

#[tokio::test]
async fn route_config_console_put_rejects_if_match_mismatch() {
    let active = document("managed", "old-model", "live-secret");
    let fixture = ConsoleStateFixture::new(active.clone(), true);
    let current_revision = fixture
        .state
        .route_config
        .snapshot()
        .revision()
        .id()
        .to_string();
    let mut draft = redact_route_document(&active).unwrap().document;
    draft
        .aliases
        .insert("answer".to_string(), "new-model".to_string());

    let response = build_router(Arc::clone(&fixture.state))
        .oneshot(
            Request::put("/v1/internal/gateway/console/route-config")
                .header("x-management-token", MANAGEMENT_TOKEN)
                .header("if-match", "\"r99-deadbeefcafe\"")
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::json!({
                        "expectedRevision": current_revision,
                        "document": draft,
                        "secretPatches": [
                            { "path": "/providers/0/api_key", "operation": "keep" }
                        ]
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    let body = parse_json(response).await;
    assert_eq!(
        body["error"]["code"].as_str(),
        Some("console_if_match_mismatch")
    );
}

#[tokio::test]
async fn route_config_management_revisions_list_returns_archived_history() {
    let fixture = ConsoleStateFixture::new(document("managed", "old-model", "live-secret"), true);
    let snapshot = fixture
        .commit_route_document(
            document("managed", "new-model", "live-secret"),
            Some("archive history"),
        )
        .await;

    let response = build_router(Arc::clone(&fixture.state))
        .oneshot(
            Request::get("/v1/internal/gateway/route-config/revisions")
                .header("x-management-token", MANAGEMENT_TOKEN)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body = parse_json(response).await;
    let revisions = body["revisions"].as_array().expect("revisions array");
    assert_eq!(revisions.len(), 1);
    assert_eq!(
        revisions[0]["revision"]["id"].as_str(),
        Some(snapshot.revision().id())
    );
    assert_eq!(revisions[0]["active"], true);
    assert_eq!(revisions[0]["hasArchive"], true);
    assert_eq!(
        revisions[0]["revision"]["message"].as_str(),
        Some("archive history")
    );
}

#[tokio::test]
async fn route_config_management_revision_detail_returns_redacted_current_snapshot() {
    let active = document("managed", "old-model", "live-secret");
    let fixture = ConsoleStateFixture::new(active.clone(), true);
    let revision_id = fixture
        .state
        .route_config
        .snapshot()
        .revision()
        .id()
        .to_string();

    let response = build_router(Arc::clone(&fixture.state))
        .oneshot(
            Request::get(format!(
                "/v1/internal/gateway/route-config/revisions/{revision_id}"
            ))
            .header("x-management-token", MANAGEMENT_TOKEN)
            .body(Body::empty())
            .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body = parse_json(response).await;
    assert_eq!(
        body["routeConfig"]["revision"]["id"].as_str(),
        Some(revision_id.as_str())
    );
    assert_eq!(body["active"], true);
    assert_eq!(body["hasArchive"], false);
    assert_ne!(
        body["routeConfig"]["document"]["providers"][0]["api_key"].as_str(),
        Some("live-secret")
    );
    assert_eq!(
        redact_route_document(&active).unwrap().secrets.len(),
        body["routeConfig"]["secrets"].as_array().unwrap().len()
    );
}

#[tokio::test]
async fn route_config_management_revision_detail_returns_not_found_for_unknown_revision() {
    let fixture = ConsoleStateFixture::new(document("managed", "old-model", "live-secret"), true);

    let response = build_router(Arc::clone(&fixture.state))
        .oneshot(
            Request::get("/v1/internal/gateway/route-config/revisions/r99-deadbeefcafe")
                .header("x-management-token", MANAGEMENT_TOKEN)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::NOT_FOUND);
    let body = parse_json(response).await;
    assert_eq!(
        body["error"]["code"].as_str(),
        Some("console_revision_not_found")
    );
}

#[tokio::test]

async fn gemini_auth_manual_completion_route_is_registered_on_complete_suffix() {
    let fixture = ConsoleStateFixture::new(document("managed", "gpt-5.4", "live-secret"), false);

    let response = build_router(Arc::clone(&fixture.state))
        .oneshot(
            Request::post(
                "/v1/internal/gateway/console/gemini-auth-sessions/nonexistent-session/complete",
            )
            .header("x-management-token", MANAGEMENT_TOKEN)
            .body(Body::empty())
            .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::NOT_FOUND);
    let body = parse_json(response).await;
    assert_eq!(
        body["error"]["code"],
        "console_gemini_auth_session_not_found"
    );
}
