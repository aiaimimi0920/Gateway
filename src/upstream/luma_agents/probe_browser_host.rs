//! Explicit opt-in normal-main browser host; only this test layer remaps synthetic upstream I/O.
use super::{pipeline_fixture::*, test_support::*};
use axum::{extract::Request, middleware::Next};
use serde_json::json;
use std::{net::SocketAddr, path::PathBuf, time::Duration};

#[tokio::test]
#[ignore = "Requires an explicit new fixture root and a dedicated browser session"]
async fn luma_agents_probe_normal_main_browser_host() {
    tokio::time::timeout(Duration::from_secs(1800), Box::pin(serve()))
        .await
        .unwrap();
}
async fn serve() {
    let root = PathBuf::from(
        std::env::var_os("GATEWAY_LUMA_PROBE_UI_ROOT").expect("Explicit fixture root required"),
    );
    assert!(
        root.is_absolute() && !root.exists(),
        "Use a new absolute fixture root"
    );
    std::fs::create_dir(&root).unwrap();
    let server = Server::start(vec![
        Reply::json(201, job("queued")),
        Reply::json(200, job("completed")),
        Reply::png(),
    ])
    .await;
    let fixture = Box::pin(Fixture::new(&server, 5)).await;
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let remap = server.origin.clone();
    let app = crate::http::router::build_router(fixture.state.clone()).layer(
        axum::middleware::from_fn(move |request: Request, next: Next| {
            let origin = remap.clone();
            async move { within(&origin, next.run(request)).await }
        }),
    );
    let (shutdown, stopped) = tokio::sync::oneshot::channel();
    let task = tokio::spawn(async move {
        axum::serve(
            listener,
            app.into_make_service_with_connect_info::<SocketAddr>(),
        )
        .with_graceful_shutdown(async {
            let _ = stopped.await;
        })
        .await
        .unwrap();
    });
    std::fs::write(
        root.join("ready.json"),
        serde_json::to_vec_pretty(&json!({
        "origin":format!("http://{address}"),"pid":std::process::id(),"storageMode":"local",
        "provider":"luma-pipeline","credential":"exact","initialBalance":2,
        "syntheticOnly":true,"remapScope":"test middleware only"}))
        .unwrap(),
    )
    .unwrap();
    loop {
        tokio::time::sleep(Duration::from_millis(200)).await;
        match std::fs::read(root.join("command.json")) {
            Ok(bytes) => {
                assert_eq!(
                    serde_json::from_slice::<serde_json::Value>(&bytes).unwrap(),
                    json!({"action":"finish"})
                );
                break;
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => (),
            Err(error) => panic!("Fixture command read failed: {error}"),
        }
    }
    shutdown.send(()).unwrap();
    tokio::time::timeout(Duration::from_secs(10), task)
        .await
        .unwrap()
        .unwrap();
    let row = fixture.audit().await;
    assert_eq!(row.status, "completed");
    assert_eq!(row.response_id, ID);
    assert_eq!(row.resolved_model.as_deref(), Some("uni-1"));
    assert!(row.total_tokens.is_none() && row.access_key_id.is_none());
    assert_eq!(
        row.route_trace.as_ref().unwrap()["consoleMediaProbe"]["chargeConfirmed"],
        true
    );
    assert_eq!(fixture.balance().await.remaining_messages, Some(2));
    let calls = server.calls();
    assert_eq!(calls.len(), 3);
    assert_eq!(calls[0].method, "POST");
    assert_eq!(calls[0].path, "/v1/generations");
    assert_eq!(calls[0].body["type"], "image");
    assert_eq!(calls[0].headers["authorization"], format!("Bearer {KEY}"));
    assert_eq!(calls[1].path, format!("/v1/generations/{ID}"));
    assert_eq!(calls[1].method, "GET");
    assert!(!calls[2].headers.contains_key("authorization"));
    let completed = json!({"status":"passed","ordinaryKeyBalance":2,"generation":ID,
        "audit":row,"upstreamCalls":calls.iter().map(|call|json!({"method":call.method,"path":call.path.split('?').next().unwrap()})).collect::<Vec<_>>(),
        "scope":"normal main to management TCP and synthetic upstream; no real platform or public Key charge"});
    fixture.finish().await;
    server.stop().await;
    // The receipt is terminal only after the TCP server, SQLite and upstream fixture have exited.
    std::fs::write(
        root.join("completed.json"),
        serde_json::to_vec_pretty(&completed).unwrap(),
    )
    .unwrap();
}
