//! Opt-in normal main browser host; management authorization and saved-account resolution stay real.
use super::{browser_fixture, fixture::Peer, test_support};
use axum::{extract::Request, middleware::Next};
use serde_json::{json, Value};
use std::{net::SocketAddr, path::PathBuf, time::Duration};

#[tokio::test]
#[ignore = "Requires tools/luma/mcp-browser-check.mjs and a new explicit fixture root"]
async fn luma_mcp_normal_main_browser_host() {
    tokio::time::timeout(Duration::from_secs(1800), Box::pin(serve()))
        .await
        .unwrap();
}
async fn serve() {
    let root = PathBuf::from(
        std::env::var_os("GATEWAY_LUMA_MCP_UI_ROOT").expect("Explicit fixture root required"),
    );
    assert!(
        root.is_absolute() && !root.exists(),
        "Use a new absolute fixture root"
    );
    std::fs::create_dir(&root).unwrap();
    let peer = Peer::new(browser_fixture::replies()).await;
    let state = browser_fixture::state(&root).await;
    let revision = state.route_config.snapshot().revision().id().to_owned();
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let endpoint = peer.endpoint.clone();
    let app = crate::http::router::build_router(state.clone()).layer(axum::middleware::from_fn(
        move |request: Request, next: Next| {
            let endpoint = endpoint.clone();
            async move { test_support::scope(endpoint, next.run(request)).await }
        },
    ));
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
    std::fs::write(root.join("ready.json"), serde_json::to_vec_pretty(&json!({
        "origin":format!("http://{address}"),"pid":std::process::id(),"syntheticOnly":true,
        "scenario":"luma-mcp-normal-main-v1","provider":"luma-mcp","credential":"saved-mcp","revision":revision
    })).unwrap()).unwrap();
    let success = loop {
        tokio::time::sleep(Duration::from_millis(200)).await;
        match std::fs::read(root.join("command.json")) {
            Ok(bytes) => {
                let command: Value = serde_json::from_slice(&bytes).unwrap();
                assert!(matches!(
                    command["action"].as_str(),
                    Some("finish" | "abort")
                ));
                break command["action"] == "finish";
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => (),
            Err(error) => panic!("Fixture command read failed: {error}"),
        }
    };
    shutdown.send(()).unwrap();
    tokio::time::timeout(Duration::from_secs(10), task)
        .await
        .unwrap()
        .unwrap();
    let final_revision = state.route_config.snapshot().revision().id().to_owned();
    let accounts = state.route_config.snapshot().document().providers[0]
        .credentials
        .clone();
    browser_fixture::close(&state).await;
    // Stop all resources before asserting acceptance, including on a browser abort.
    let calls = peer.calls.lock().unwrap().clone();
    let deletes = peer.deletes.lock().unwrap().clone();
    peer.stop().await;
    assert!(success, "Browser did not complete; isolated host shut down");
    let wire = browser_fixture::assert_wire(&calls, &deletes);
    assert_eq!(revision, final_revision);
    assert_eq!(accounts[0].api_key.as_deref(), Some("synthetic-access"));
    assert_eq!(accounts[1].api_key.as_deref(), Some("synthetic-other"));
    assert!(accounts
        .iter()
        .all(|account| account.enabled == Some(false)));
    std::fs::write(root.join("completed.json"), serde_json::to_vec_pretty(&json!({
        "status":"passed","wire":wire,"revisionUnchanged":true,"accountsUnchanged":true,
        "scope":"normal main UI and real management TCP; synthetic read-only MCP; no OAuth refresh or generation"
    })).unwrap()).unwrap();
}
