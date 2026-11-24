//! Opt-in normal main UI host; the production management/probe owners retain all authorization.
use super::*;
use axum::{extract::Request as HttpRequest, middleware::Next};
use std::{net::SocketAddr, path::PathBuf};

#[tokio::test]
#[ignore = "Requires tools/luma/inference-browser-check.mjs and a new explicit fixture root"]
async fn luma_inference_normal_main_browser_host() {
    tokio::time::timeout(Duration::from_secs(1800), Box::pin(serve()))
        .await
        .unwrap();
}

async fn serve() {
    let root = PathBuf::from(
        std::env::var_os("GATEWAY_LUMA_INFERENCE_UI_ROOT").expect("Explicit fixture root required"),
    );
    assert!(
        root.is_absolute() && !root.exists(),
        "Use a new absolute fixture root"
    );
    std::fs::create_dir(&root).unwrap();
    let peer = peer::Peer::new("chat", false, false, 200).await;
    let fixture = Box::pin(Fixture::new(&peer)).await;
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
    let app = build_router(fixture.state.clone()).layer(axum::middleware::from_fn(
        move |request: HttpRequest, next: Next| {
            let origin = remap.clone();
            async move { super::super::test_support::origin(origin, next.run(request)).await }
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
    std::fs::write(
        root.join("ready.json"),
        serde_json::to_vec_pretty(&json!({
            "origin":format!("http://{address}"),"pid":std::process::id(),"syntheticOnly":true,
            "scenario":"luma-inference-normal-main-v1","provider":"luma_inference-public",
            "credential":"exact","model":MODEL,"revision":revision,"storageMode":"local"
        }))
        .unwrap(),
    )
    .unwrap();
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
    let rows = fixture
        .state
        .local_runtime
        .as_ref()
        .unwrap()
        .list_audits(&db::RequestAuditFilters::default())
        .await
        .unwrap();
    let balance = fixture.remaining().await;
    let final_revision = fixture
        .state
        .route_config
        .snapshot()
        .revision()
        .id()
        .to_owned();
    fixture.finish().await;
    // Check the wire before stopping the peer, but keep cleanup ahead of acceptance assertions.
    let calls = peer.calls.lock().unwrap().clone();
    peer.stop().await;
    assert!(success, "Browser did not complete; isolated host shut down");
    assert_eq!(balance, 2);
    assert_eq!(revision, final_revision);
    assert_eq!(
        calls.len(),
        1,
        "opening/closing the dialog must not call inference"
    );
    let (path, headers, raw) = &calls[0];
    assert_eq!(path, "/v1/chat/completions");
    assert_eq!(headers["authorization"], format!("Bearer {KEY}"));
    assert_eq!(headers["user-agent"], "NeuroGateway/LumaInference");
    for header in [
        "cookie",
        "x-api-key",
        "x-management-token",
        "x-secret-grant",
        "originator",
        "chatgpt-account-id",
    ] {
        assert!(!headers.contains_key(header));
    }
    assert_eq!(raw["model"], UPSTREAM_MODEL);
    assert_eq!(raw["messages"][0]["content"], "合成问题");
    assert_eq!(raw["max_tokens"], 256);
    assert_eq!(raw["stream"], false);
    assert_eq!(rows.len(), 1);
    // Console text probes record the resolved model; the assessment retains the requested alias.
    assert_eq!(rows[0].requested_model.as_deref(), Some(UPSTREAM_MODEL));
    assert_eq!(rows[0].resolved_model.as_deref(), Some(UPSTREAM_MODEL));
    assert_eq!(
        rows[0].provider_account_id.as_deref(),
        Some("luma_inference-public")
    );
    assert_eq!(
        rows[0].route_trace.as_ref().unwrap()["realCredentialRef"],
        "exact"
    );
    let saved = serde_json::to_string(&rows[0]).unwrap();
    assert!(!saved.contains(KEY) && !saved.contains("caller-secret"));
    assert_eq!(rows[0].status, "completed");
    assert_eq!(
        (
            rows[0].prompt_tokens,
            rows[0].completion_tokens,
            rows[0].total_tokens
        ),
        (Some(2), Some(3), Some(5))
    );
    assert!(rows[0].access_key_id.is_none());
    std::fs::write(
        root.join("completed.json"),
        serde_json::to_vec_pretty(&json!({
            "status":"passed","ordinaryKeyBalance":balance,"revisionUnchanged":true,
            "upstreamCalls":1,"path":path,"model":raw["model"],"audit":rows[0],
            "scope":"normal main UI to management TCP and synthetic inference; no vendor charge"
        }))
        .unwrap(),
    )
    .unwrap();
}
