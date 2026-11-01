//! A real restarting HTTP proxy must not require a Gateway restart or rewrite the tool wire.
use super::connect_tests::{policy, tool_gate};
use super::*;
use std::sync::{
    atomic::{AtomicUsize, Ordering},
    Arc,
};

#[tokio::test]
async fn restarting_proxy_recovers_the_same_tool_request_over_the_same_gate() {
    let reserved = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = reserved.local_addr().unwrap();
    drop(reserved);
    let captured = Arc::new(parking_lot::Mutex::new(Vec::new()));
    let server = parking_lot::Mutex::new(None);
    let client = reqwest::Client::builder()
        .no_proxy()
        .proxy(reqwest::Proxy::http(format!("http://{address}")).unwrap())
        .build()
        .unwrap();
    let body =
        "{ \"input\": \"original\", \"tools\": [{\"type\":\"function\",\"name\":\"fixture\"}] }\n";
    let original = client
        .post("http://origin.invalid/v1/responses?fixture=1")
        .header("authorization", "Bearer fixture-secret")
        .header("x-request-id", "fixed-request-id")
        .body(body);
    let gate = tool_gate();
    let calls = AtomicUsize::new(0);
    let result = gate
        .request_budget
        .run(gate.execute_observed(
            || async {
                let result =
                    crate::http_client::send_native_codex(original.try_clone().unwrap(), "fixture")
                        .await;
                if calls.fetch_add(1, Ordering::SeqCst) == 0 {
                    let error = result.as_ref().unwrap_err();
                    assert!(error.native_connect_retry_allowed());
                    assert!(!error.message.contains("fixture-secret"));
                    let listener = tokio::net::TcpListener::bind(address).await.unwrap();
                    let captured = captured.clone();
                    let app = axum::Router::new().fallback(move |req: axum::extract::Request| {
                        let captured = captured.clone();
                        async move {
                            let (parts, body) = req.into_parts();
                            let bytes = axum::body::to_bytes(body, 8192).await.unwrap();
                            captured
                                .lock()
                                .push((parts.method, parts.uri, parts.headers, bytes));
                            "{\"ok\":true}"
                        }
                    });
                    *server.lock() = Some(tokio::spawn(async move {
                        axum::serve(listener, app).await.unwrap()
                    }));
                }
                result
            },
            |_| {},
            &policy(),
        ))
        .await
        .unwrap();
    assert_eq!(result.text().await.unwrap(), "{\"ok\":true}");
    assert_eq!(calls.load(Ordering::SeqCst), 2);
    assert_eq!(gate.request_budget.attempts(), 2);
    assert_eq!(gate.outbound_attempt_count.load(Ordering::SeqCst), 2);
    let received = captured.lock();
    assert_eq!(
        received.len(),
        1,
        "failed connector must not reach the origin"
    );
    assert_eq!(received[0].0, "POST");
    assert_eq!(
        received[0].1.to_string(),
        "http://origin.invalid/v1/responses?fixture=1"
    );
    assert_eq!(received[0].2["authorization"], "Bearer fixture-secret");
    assert_eq!(received[0].2["x-request-id"], "fixed-request-id");
    assert_eq!(received[0].3.as_ref(), body.as_bytes());
    drop(received);
    let task = server.lock().take().unwrap();
    task.abort();
    let _ = task.await;
}
