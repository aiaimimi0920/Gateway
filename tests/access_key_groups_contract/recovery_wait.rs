//! Synthetic upstream only: recovery owns one request, not a new route or caller identity.
use super::capacity::Fixture;
use super::*;
use std::{sync::atomic::Ordering, time::Duration};

fn request(f: &Fixture, stream: bool, tools: bool) -> Request<Body> {
    let mut body = json!({"model":"gpt-fixture","input":"fixture","stream":stream});
    if tools {
        body["tools"] =
            json!([{"type":"function","name":"fixture","parameters":{"type":"object"}}]);
    }
    Request::post("/v1/responses")
        .header("content-type", "application/json")
        .header("authorization", format!("Bearer {}", f.token))
        .body(Body::from(body.to_string()))
        .unwrap()
}

// Give real loopback sockets/SQLite workers time to settle between virtual timer ticks.
// The runnable driver prevents Tokio from auto-advancing past pending network IO.
async fn drive<T>(task: &tokio::task::JoinHandle<T>, seconds: u32) {
    for _ in 0..seconds {
        for _ in 0..100 {
            tokio::task::yield_now().await;
        }
        if task.is_finished() {
            return;
        }
        std::thread::sleep(Duration::from_millis(3));
        tokio::time::advance(Duration::from_secs(1)).await;
    }
    assert!(
        task.is_finished(),
        "recovery did not settle within virtual budget"
    );
}

async fn recovers(refusals: usize, stream: bool, tools: bool) {
    let f = Box::pin(Fixture::with_wait(refusals, None)).await;
    let app = build_router(f.state.clone());
    let input = request(&f, stream, tools);
    tokio::time::pause();
    let task = tokio::spawn(async move { Box::pin(app.oneshot(input)).await.unwrap() });
    drive(&task, 1800).await;
    let response = task.await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    assert!(String::from_utf8_lossy(&bytes).contains("OK"));
    assert_eq!(
        f.mock.calls.load(Ordering::SeqCst),
        refusals + 1,
        "only actual refusals permit readmission"
    );
    tokio::time::resume();
    f.no_persisted_cooldown().await;
    f.close().await;
}

#[tokio::test]
async fn recovery_json_survives_more_than_six_explicit_capacity_refusals() {
    Box::pin(recovers(7, false, false)).await;
}
#[tokio::test]
async fn recovery_sse_before_first_event_and_tool_declarations_are_safe() {
    Box::pin(recovers(7, true, true)).await;
}

#[tokio::test]
async fn one_request_survives_more_recovery_rounds_than_the_per_key_slot_limit() {
    Box::pin(recovers(17, false, false)).await;
}

#[tokio::test]
async fn bounded_wait_stops_at_original_deadline_without_an_extra_send() {
    let f = Box::pin(Fixture::with_wait(
        100,
        Some(json!({"mode":"bounded","total_timeout_seconds":3})),
    ))
    .await;
    let app = build_router(f.state.clone());
    let input = request(&f, false, false);
    let task = tokio::spawn(async move { Box::pin(app.oneshot(input)).await.unwrap() });
    // Establish the real refusal before advancing virtual time; SQLite/loopback IO
    // is not a timer and must not be raced by a three-second synthetic deadline.
    tokio::time::timeout(Duration::from_secs(5), async {
        while f.mock.calls.load(Ordering::SeqCst) == 0 {
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap();
    tokio::time::sleep(Duration::from_millis(10)).await;
    tokio::time::pause();
    drive(&task, 20).await;
    let response = task.await.unwrap();
    assert_eq!(response.status(), StatusCode::TOO_MANY_REQUESTS);
    assert!(f.mock.calls.load(Ordering::SeqCst) <= 2);
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    assert!(String::from_utf8_lossy(&bytes).contains("turn_rate_limit_exceeded"));
    let before = f.mock.calls.load(Ordering::SeqCst);
    tokio::time::advance(Duration::from_secs(100)).await;
    assert_eq!(f.mock.calls.load(Ordering::SeqCst), before);
    tokio::time::resume();
    f.close().await;
}

#[tokio::test]
async fn real_http_disconnect_cancels_long_wait_without_background_replay() {
    use tokio::io::AsyncWriteExt;
    let f = Box::pin(Fixture::with_wait(100, None)).await;
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let app = build_router(f.state.clone());
    let server = tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    let mut tcp = tokio::net::TcpStream::connect(address).await.unwrap();
    let body = r#"{"model":"gpt-fixture","input":"disconnect","stream":false}"#;
    tcp.write_all(format!("POST /v1/responses HTTP/1.1\r\nHost: {address}\r\nAuthorization: Bearer {}\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n{body}", f.token, body.len()).as_bytes()).await.unwrap();
    tokio::time::timeout(Duration::from_secs(5), async {
        while f.mock.calls.load(Ordering::SeqCst) == 0 {
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap();
    drop(tcp);
    tokio::time::sleep(Duration::from_secs(5)).await;
    assert_eq!(
        f.mock.calls.load(Ordering::SeqCst),
        1,
        "a disconnected HTTP caller must not trigger another send"
    );
    server.abort();
    let _ = server.await;
    f.close().await;
}

#[tokio::test]
async fn waiting_request_rechecks_manual_disable_and_entitlement_group_membership() {
    for revoke_group in [false, true] {
        let f = Box::pin(Fixture::with_wait(100, None)).await;
        let app = build_router(f.state.clone());
        let input = request(&f, false, false);
        let task = tokio::spawn(async move { Box::pin(app.oneshot(input)).await.unwrap() });
        tokio::time::timeout(Duration::from_secs(5), async {
            while f.mock.calls.load(Ordering::SeqCst) == 0 {
                tokio::task::yield_now().await;
            }
        })
        .await
        .unwrap();
        let mut document: Value =
            serde_json::from_slice(&std::fs::read(&f.state.config.console.routes_file).unwrap())
                .unwrap();
        if revoke_group {
            document["account_groups"] = json!([{"id":"private","name":"Private","provider_credential_ids":["only-account"]}]);
        } else {
            document["providers"][0]["credentials"][0]["enabled"] = json!(false);
        }
        f.state
            .route_config
            .replace_document(serde_json::from_value(document).unwrap())
            .unwrap();
        tokio::time::pause();
        drive(&task, 20).await;
        assert_eq!(task.await.unwrap().status(), StatusCode::FORBIDDEN);
        assert_eq!(f.mock.calls.load(Ordering::SeqCst), 1);
        tokio::time::resume();
        f.close().await;
    }
}
