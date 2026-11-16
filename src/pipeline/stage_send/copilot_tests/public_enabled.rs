//! Management commit through the real public router; no manually prepared attempts or eligibility bypass.
use super::*;
use crate::console::secrets::{redact_route_document, SecretPatch};
use axum::{
    body::Body,
    http::{Method, Request, StatusCode},
};
use http_body_util::BodyExt;
use tower::ServiceExt;
#[path = "public_rejections.rs"]
mod rejections;
#[path = "public_revocation.rs"]
mod revocation;

async fn saved() -> fixture::Fixture {
    let f = fixture::Fixture::with_runtime(true).await;
    f.local
        .save_access_key_with_quota(
            Some(&f.key),
            fixture::input(json!({
                "scopes":["relay"],"models":["public-alias"],"providerIds":["copilot"]
            })),
            Some(&crate::access_balance::quota::KeyQuotaInput {
                mode: "message_prepaid".into(),
                limit: Some(2),
                currency: None,
            }),
        )
        .await
        .unwrap();
    let snapshot = f.console.state.route_config.snapshot();
    let redacted = redact_route_document(snapshot.document()).unwrap();
    let mut document = redacted.document;
    document.providers[0].supported_models = vec!["synthetic-model".into()];
    document
        .aliases
        .insert("public-alias".into(), "synthetic-model".into());
    let patches: Vec<_> = redacted
        .secrets
        .iter()
        .map(|secret| {
            if secret.path.ends_with("copilot_github_token") {
                SecretPatch::replace(&secret.path, json!("synthetic-token"))
            } else {
                SecretPatch::keep(&secret.path)
            }
        })
        .collect();
    let grant = fixture::support::grant_console_secret_access(&f.console.state).await;
    let response = fixture::support::send_console_json(
        &f.console.state,
        Method::PUT,
        "/v1/internal/gateway/console/route-config",
        json!({
            "expectedRevision":snapshot.revision().id(),"document":document,"secretPatches":patches
        }),
        Some(&grant),
        None,
    )
    .await;
    let status = response.status();
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    assert_eq!(
        status,
        StatusCode::OK,
        "{}",
        String::from_utf8_lossy(&bytes)
    );
    assert!(!String::from_utf8_lossy(&bytes).contains("synthetic-token"));
    assert_eq!(
        f.console
            .state
            .route_config
            .snapshot()
            .resolve_candidates(Some("public-alias"))
            .len(),
        1
    );
    f
}

async fn call(
    f: &fixture::Fixture,
    path: &str,
    body: serde_json::Value,
) -> (StatusCode, serde_json::Value) {
    let token = f.local.access_key_secret(&f.key).await.unwrap();
    let response = Box::pin(
        crate::http::router::build_router(f.console.state.clone()).oneshot(
            Request::post(path)
                .header("authorization", format!("Bearer {token}"))
                .header("content-type", "application/json")
                .body(Body::from(body.to_string()))
                .unwrap(),
        ),
    )
    .await
    .unwrap();
    let status = response.status();
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    (status, serde_json::from_slice(&bytes).unwrap())
}

#[tokio::test]
#[ignore = "requires explicit synthetic CLI and fresh isolated probe/workspace roots; run serially"]
async fn copilot_public_saved_identity_chat_and_responses_use_real_gate() {
    let f = saved().await;
    // Unsupported requests must reject before SDK startup, and refund any message reservation.
    for (path, body) in [
        (
            "/v1/chat/completions",
            json!({"model":"public-alias","messages":[{"role":"user","content":"ok"}],"stream":true}),
        ),
        (
            "/v1/responses",
            json!({"model":"public-alias","input":"ok","stream":true}),
        ),
        (
            "/v1/chat/completions",
            json!({"model":"public-alias","messages":[{"role":"user","content":"ok"},{"role":"assistant","content":"history"}]}),
        ),
        (
            "/v1/responses",
            json!({"model":"public-alias","input":"ok","temperature":0.5}),
        ),
    ] {
        let expected = if body["stream"] == true {
            StatusCode::SERVICE_UNAVAILABLE
        } else {
            StatusCode::BAD_REQUEST
        };
        let (status, value) = call(&f, path, body).await;
        assert_eq!(status, expected, "{value}");
    }
    assert_eq!(
        f.local
            .access_balance(&f.key)
            .await
            .unwrap()
            .unwrap()
            .remaining_messages,
        Some(2)
    );
    let root = std::path::PathBuf::from(std::env::var_os("GATEWAY_PROBE_ROOT").unwrap());
    assert!(!std::fs::read_dir(&root)
        .unwrap()
        .filter_map(Result::ok)
        .any(|e| e.file_name().to_string_lossy().starts_with("child-")));
    for (path, body) in [
        (
            "/v1/chat/completions",
            json!({"model":"public-alias","messages":[{"role":"user","content":"ok"}]}),
        ),
        (
            "/v1/responses",
            json!({"model":"public-alias","input":"known-usage"}),
        ),
    ] {
        let (status, value) = call(&f, path, body).await;
        assert_eq!(status, StatusCode::OK, "{value}");
        assert_eq!(value["model"], "public-alias");
        assert!(value.get("usage").is_none());
        assert!(value.to_string().contains("synthetic answer"));
        assert!(!value.to_string().contains("synthetic-token"));
    }
    assert_eq!(
        f.local
            .access_balance(&f.key)
            .await
            .unwrap()
            .unwrap()
            .remaining_messages,
        Some(0)
    );
    let rows = f
        .local
        .list_audits(&crate::db::RequestAuditFilters::default())
        .await
        .unwrap();
    assert_eq!(rows.len(), 6);
    assert_eq!(rows.iter().filter(|r| r.status == "completed").count(), 2);
    for row in rows {
        if row.status == "completed" {
            assert_eq!(
                row.route_trace.as_ref().unwrap()["realCredentialRef"],
                "account"
            );
        }
        assert!(row.upstream_status.is_none() && row.total_tokens.is_none());
        assert_eq!(row.access_key_id.as_deref(), Some(f.key.as_str()));
        assert_eq!(
            row.route_trace.as_ref().unwrap()["routeAttemptCount"],
            u32::from(row.status == "completed")
        );
    }
    finish(f).await;
}
