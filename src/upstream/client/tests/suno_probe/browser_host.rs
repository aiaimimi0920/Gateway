//! Opt-in normal UI host: real management TCP/Grant with one synthetic bound executor and MP3s.
use super::*;
use axum::{extract::Request, middleware::Next};
use std::{net::SocketAddr, path::PathBuf, time::Duration};

#[tokio::test]
#[ignore = "Requires an explicit new fixture root and a dedicated browser session"]
async fn suno_probe_browser_normal_main_tcp_host() {
    tokio::time::timeout(Duration::from_secs(1800), Box::pin(serve()))
        .await
        .unwrap();
}
async fn serve() {
    let root = PathBuf::from(
        std::env::var_os("GATEWAY_SUNO_PROBE_UI_ROOT").expect("Explicit new root required"),
    );
    assert!(
        root.is_absolute() && !root.exists(),
        "Use a new absolute fixture root"
    );
    std::fs::create_dir(&root).unwrap();
    let peer = Server::start(completed()).await;
    let fixture = browser_fixture(&peer, 0).await;
    let document = serde_json::to_value(fixture.state.route_config.snapshot().document()).unwrap();
    let revision = fixture
        .state
        .route_config
        .snapshot()
        .revision()
        .id()
        .to_owned();
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let remap = peer.origin.clone();
    let app = crate::http::router::build_router(fixture.state.clone()).layer(
        axum::middleware::from_fn(move |request: Request, next: Next| {
            let origin = remap.clone();
            async move { crate::upstream::public_assets::within(&origin, next.run(request)).await }
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
    write(
        &root,
        "ready.json",
        json!({"origin":format!("http://{address}"),"pid":std::process::id(),
        "storageMode":"local","provider":"suno-pipeline","credential":"exact","model":MODEL,
        "initialBalance":2,"syntheticOnly":true,"assetRemap":"test middleware only"}),
    );
    loop {
        // A zero-call observation after the normal UI's cancel proves no paid work started.
        write(
            &root,
            "observed.json",
            json!({"upstreamCalls":peer.calls().len()}),
        );
        tokio::time::sleep(Duration::from_millis(200)).await;
        match std::fs::read(root.join("command.json")) {
            Ok(bytes) => {
                assert_eq!(
                    serde_json::from_slice::<Value>(&bytes).unwrap(),
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
    audit(&row, true);
    wire(&peer, 2);
    assert_eq!(fixture.balance().await.remaining_messages, Some(2));
    assert_eq!(
        serde_json::to_value(fixture.state.route_config.snapshot().document()).unwrap(),
        document
    );
    assert_eq!(
        fixture.state.route_config.snapshot().revision().id(),
        revision
    );
    let receipt = json!({"status":"passed","ordinaryKeyBalance":2,"audit":row,
        "configurationUnchanged":true,"revisionUnchanged":true,"upstreamCalls":3,
        "scope":"normal main UI to management TCP and synthetic bound executor; no real Suno"});
    fixture.finish().await;
    peer.stop().await;
    // Terminal receipt follows the actual TCP, SQLite, FreeBuff worker and upstream joins.
    write(&root, "completed.json", receipt);
}
fn write(root: &std::path::Path, name: &str, value: Value) {
    // Publish a complete snapshot; the browser must never mistake a partial read for zero calls.
    let staging = root.join(format!("{name}.staging"));
    std::fs::write(&staging, serde_json::to_vec_pretty(&value).unwrap()).unwrap();
    std::fs::rename(staging, root.join(name)).unwrap();
}
