//! Human-reviewed cleanup contracts through real HTTP, durable CAS and loopback I/O.
use super::*;
use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use serde_json::{json, Value};
use std::sync::atomic::Ordering;
use tower::ServiceExt;

#[path = "maintenance_fixture.rs"]
mod fixture;
use fixture::{decode, request, selection, Fixture};
#[path = "maintenance_race_tests.rs"]
mod races;

#[tokio::test]
async fn maintenance_preview_then_selected_archive_is_durable_and_secret_free() {
    // The reviewed path overrides the legacy permanent-delete switch.
    let h = Fixture::start(false, true).await;
    let revision = h.state.route_config.snapshot().revision().id().to_string();
    let preview = h.preview().await;
    assert_eq!(preview["items"].as_array().unwrap().len(), 2);
    assert!(!preview.to_string().contains("fixture-private"));
    assert_eq!(h.state.route_config.snapshot().revision().id(), revision);
    assert!(!h.root.join("archive").exists());
    let response = h.request("apply", selection(&preview, json!(["a"]))).await;
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(response.headers()["cache-control"], "no-store");
    let receipt = decode(response).await;
    assert_eq!(receipt["receipt"]["removedCount"], 1);
    assert_eq!(
        receipt["receipt"]["items"][0]["status"],
        "archived_and_removed"
    );
    assert!(!receipt.to_string().contains("fixture-private"));
    assert_eq!(fs::read_dir(h.root.join("archive")).unwrap().count(), 1);
    let persisted = h.close().await;
    assert_eq!(persisted.providers[0].credentials.len(), 1);
    assert_eq!(
        persisted.providers[0].credentials[0].id.as_deref(),
        Some("b")
    );
}

#[tokio::test]
async fn maintenance_rejects_untrusted_access_invalid_selection_and_oversized_body() {
    let h = Fixture::start(false, false).await;
    for suffix in ["preview", "apply"] {
        for token in [None, Some("public-api-key")] {
            let response = h
                .router
                .clone()
                .oneshot(request(suffix, "{}".into(), token))
                .await
                .unwrap();
            assert!(matches!(
                response.status(),
                StatusCode::UNAUTHORIZED | StatusCode::FORBIDDEN
            ));
        }
    }
    assert_eq!(h.driver.calls.load(Ordering::SeqCst), 0);
    let preview = h.preview().await;
    for ids in [
        json!([]),
        json!(["a", "a"]),
        json!(["foreign"]),
        json!([" a"]),
        json!(vec!["a"; 129]),
    ] {
        assert!(h
            .request("apply", selection(&preview, ids))
            .await
            .status()
            .is_client_error());
    }
    let mut input = selection(&preview, json!(["a"]));
    input["unexpected"] = json!(true);
    assert_eq!(
        h.request("apply", input).await.status(),
        StatusCode::BAD_REQUEST
    );
    let mut input = selection(&preview, json!(["a"]));
    input["previewDigest"] = json!("0".repeat(64));
    assert_eq!(
        h.request("apply", input).await.status(),
        StatusCode::CONFLICT
    );
    let mut input = selection(&preview, json!(["a"]));
    input["expectedRevision"] = json!("stale");
    assert_eq!(
        h.request("apply", input).await.status(),
        StatusCode::CONFLICT
    );
    let response = h
        .router
        .clone()
        .oneshot(request(
            "apply",
            " ".repeat(65537),
            Some("fixture-management-token"),
        ))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::PAYLOAD_TOO_LARGE);
    assert!(!h.root.join("archive").exists());
    assert_eq!(h.close().await.providers[0].credentials.len(), 2);
}

#[tokio::test]
async fn maintenance_revalidates_classification_and_rejects_invalid_driver_output() {
    let h = Fixture::start(false, false).await;
    let preview = h.preview().await;
    *h.driver.response.lock().unwrap() =
        json!({"prune":[{"credential_id":"a","classification":"account_deleted"}]});
    assert_eq!(
        h.request("apply", selection(&preview, json!(["a"])))
            .await
            .status(),
        StatusCode::CONFLICT
    );
    for response in [
        json!({"prune":[{"credential_id":"foreign","classification":"account_deleted"}]}),
        json!({"prune":[{"credential_id":"a","classification":"quota_exhausted"}]}),
        json!({"prune":[{"credential_id":"a","classification":"account_deleted"},{"credential_id":"a","classification":"permanent_auth_failure"}]}),
        json!({"credentials":[{"id":"injected","api_key":"private"}]}),
    ] {
        *h.driver.response.lock().unwrap() = response;
        let response = h.request("preview", json!({})).await;
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        assert!(!decode(response).await.to_string().contains("private"));
    }
    assert!(!h.root.join("archive").exists());
    assert_eq!(h.close().await.providers[0].credentials.len(), 2);
}

#[tokio::test]
async fn maintenance_partial_cloud_archive_failure_removes_only_verified_subset() {
    let h = Fixture::start(true, false).await;
    let preview = h.preview().await;
    let response = h
        .request("apply", selection(&preview, json!(["a", "b"])))
        .await;
    assert_eq!(response.status(), StatusCode::OK);
    let receipt = decode(response).await;
    assert_eq!(receipt["receipt"]["removedCount"], 1);
    assert_eq!(
        receipt["receipt"]["items"][0]["status"],
        "archived_and_removed"
    );
    assert_eq!(receipt["receipt"]["items"][1]["status"], "archive_failed");
    assert_eq!(h.driver.archives.lock().unwrap().len(), 1);
    let persisted = h.close().await;
    assert_eq!(persisted.providers[0].credentials.len(), 1);
    assert_eq!(
        persisted.providers[0].credentials[0].id.as_deref(),
        Some("b")
    );
}
