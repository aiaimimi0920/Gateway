use super::fixture::{self, Fixture, KEYS};
use axum::http::StatusCode;
use http_body_util::BodyExt;
use serde_json::json;
use std::sync::atomic::Ordering;

#[tokio::test]
async fn rotation_limit_edits_and_restart_preserve_one_cash_account_and_old_inflight_bill() {
    let mut f = Fixture::new().await;
    f.price(1000, 2000, "0.8").await;
    let key = f.key(100_000).await;
    f.control.block.store(true, Ordering::SeqCst);
    let state = f.state.clone();
    let original_key = key.clone();
    let first = tokio::spawn(async move {
        fixture::relay(&state, &original_key, fixture::request(false), None).await
    });
    f.calls(1).await;
    let old_balance = f.balance(&key).await;
    let (status, _, next) = f
        .manage(
            &format!("{KEYS}/{}/rotate", key["id"].as_str().unwrap()),
            Some(json!({})),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{next}");
    assert_eq!(f.balance(&next).await, old_balance);
    f.control.permits.add_permits(1);
    assert_eq!(first.await.unwrap().0, StatusCode::OK);
    let ledger = f.finalized(&next, 1).await;
    assert_eq!(ledger[0]["accessKeyId"], key["id"]);
    f.limit(&next, 200).await;
    assert_eq!(f.balance(&next).await["spentMicros"], 240);
    let before_restart = f.balance(&next).await;
    f.restart().await;
    assert_eq!(f.balance(&next).await, before_restart);
    assert_eq!(
        fixture::relay(&f.state, &key, fixture::request(false), None)
            .await
            .0,
        StatusCode::UNAUTHORIZED
    );
    assert_eq!(
        fixture::relay(&f.state, &next, fixture::request(false), None)
            .await
            .0,
        StatusCode::TOO_MANY_REQUESTS
    );
    assert_eq!(f.control.calls.load(Ordering::SeqCst), 1);
    f.finish().await;
}

#[tokio::test]
async fn cancellation_before_response_and_mid_stream_retains_reservations() {
    let f = Fixture::new().await;
    f.price(1000, 2000, "0.8").await;
    let key = f.key(100_000).await;
    f.control.block.store(true, Ordering::SeqCst);
    let state = f.state.clone();
    let original_key = key.clone();
    let first = tokio::spawn(async move {
        fixture::relay(&state, &original_key, fixture::request(false), None).await
    });
    f.calls(1).await;
    first.abort();
    assert!(first.await.unwrap_err().is_cancelled());
    f.finalized(&key, 1).await;
    f.control.block.store(false, Ordering::SeqCst);
    f.control.stalled_stream.store(true, Ordering::SeqCst);
    let response = fixture::send(
        &f.state,
        "/v1/chat/completions",
        key["token"].as_str().unwrap(),
        Some(fixture::request(true)),
        None,
    )
    .await;
    assert_eq!(response.status(), StatusCode::OK);
    let mut body = response.into_body();
    assert!(
        tokio::time::timeout(std::time::Duration::from_secs(3), body.frame())
            .await
            .unwrap()
            .is_some()
    );
    drop(body);
    let ledger = f.finalized(&key, 2).await;
    assert!(ledger
        .as_array()
        .unwrap()
        .iter()
        .all(|row| row["status"] == "unresolved"));
    let balance = f.balance(&key).await;
    assert_eq!(balance["spentMicros"], 0);
    assert_eq!(balance["pendingRequests"], 2);
    assert!(balance["reservedMicros"].as_i64().unwrap() > 0);
    f.finish().await;
}
