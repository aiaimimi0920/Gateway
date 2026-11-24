//! Opt-in normal management browser -> TCP -> SQLite -> synthetic MCP creation.
use super::{browser_fixture, opened, progress, replies, started, test_support, usage, Peer};
use axum::{extract::Request, middleware::Next, routing::get, Json};
use serde_json::{json, Value};
use std::{net::SocketAddr, path::PathBuf, time::Duration};

#[tokio::test]
#[ignore = "Requires a normal UI build, installed Edge and a new explicit output root"]
async fn luma_mcp_creation_normal_main_browser() {
    let root = PathBuf::from(std::env::var_os("GATEWAY_LUMA_MCP_CREATION_UI_ROOT").unwrap());
    let ui = PathBuf::from(std::env::var_os("GATEWAY_LUMA_MCP_CREATION_UI_BUILD").unwrap());
    assert!(root.is_absolute() && !root.exists());
    assert!(ui.is_absolute() && ui.join("index.html").is_file());
    std::fs::create_dir(&root).unwrap();
    let asset = "https://cdn.example.test/creation.png?fixture=1";
    let peer = Peer::new(replies(vec![
        opened(),
        started(),
        progress("completed", false, json!([{"asset_id":"artifact:a1","type":"image","model":null}])),
        json!({"assets":[{"asset_id":"a1","type":"image","has_image":true,"state":"ready","url":asset}]}),
        usage(false),
    ])).await;
    let state = browser_fixture::state(&root).await;
    let revision = state.route_config.snapshot().revision().id().to_owned();
    let endpoint = peer.endpoint.clone();
    let seen = peer.calls.clone();
    let expected = revision.clone();
    let app = crate::http::router::build_router(state.clone())
        .route(
            "/__fixture/luma-mcp-creation",
            get(move || {
                let status = json!({"scenario":"luma-mcp-creation-v1", "revision":expected,
                    "posts":seen.lock().unwrap().len()});
                async move { Json(status) }
            }),
        )
        .layer(axum::middleware::from_fn(
            move |request: Request, next: Next| {
                let endpoint = endpoint.clone();
                async move { test_support::scope(endpoint, next.run(request)).await }
            },
        ));
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let origin = format!("http://{}", listener.local_addr().unwrap());
    let (shutdown, stopped) = tokio::sync::oneshot::channel();
    let server = tokio::spawn(async move {
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
    let browser_dir = root.join("browser");
    let browser = tokio::time::timeout(
        Duration::from_secs(150),
        tokio::process::Command::new("node")
            .current_dir(env!("CARGO_MANIFEST_DIR"))
            .arg("tools/luma/mcp-creation-browser-check.mjs")
            .arg(origin)
            .arg(ui)
            .arg(&browser_dir)
            .kill_on_drop(true)
            .status(),
    )
    .await;
    shutdown.send(()).unwrap();
    tokio::time::timeout(Duration::from_secs(10), server)
        .await
        .unwrap()
        .unwrap();
    let result = std::fs::read(browser_dir.join("result.json"));
    let observed = result
        .as_ref()
        .ok()
        .and_then(|bytes| serde_json::from_slice::<Value>(bytes).ok());
    let id = observed.as_ref().and_then(|value| value["taskId"].as_str());
    let row: Option<String> = if let Some(id) = id {
        sqlx::query_scalar("SELECT payload FROM request_audits WHERE id = ?")
            .bind(id)
            .fetch_optional(&state.local_runtime.as_ref().unwrap().pool)
            .await
            .unwrap()
    } else {
        None
    };
    let final_revision = state.route_config.snapshot().revision().id().to_owned();
    browser_fixture::close(&state).await;
    let calls = peer.calls.lock().unwrap().clone();
    let deletes = peer.deletes.lock().unwrap().len();
    peer.stop().await;
    assert!(
        browser.unwrap().unwrap().success(),
        "Browser did not complete; see {}",
        browser_dir.display()
    );
    let id = id.unwrap();
    let row: Value = serde_json::from_str(&row.unwrap()).unwrap();
    let task = &row["routeTrace"]["lumaMcpCreation"];
    assert_eq!(task["id"], id);
    assert_eq!(task["runId"], "r1");
    assert_eq!(task["state"], "completed");
    assert_eq!(task["usage"]["credits"].as_u64(), Some(9));
    assert_eq!(task["usage"]["settled"], true);
    assert_eq!(task["assets"][0]["url"], asset);
    assert_eq!(task["providerId"], "luma-mcp");
    assert_eq!(task["credentialId"], "saved-mcp");
    assert_eq!(task["revision"], revision);
    assert_eq!(revision, final_revision);
    let tools: Vec<_> = calls
        .iter()
        .filter(|(_, body)| body["method"] == "tools/call")
        .map(|(_, body)| &body["params"])
        .collect();
    assert_eq!(
        tools
            .iter()
            .map(|tool| tool["name"].as_str().unwrap())
            .collect::<Vec<_>>(),
        [
            "luma_session_open",
            "luma_generate_image",
            "luma_run_await",
            "luma_workspace_view_asset",
            "luma_usage",
        ]
    );
    assert_eq!(tools[0]["arguments"]["workspace_key"], id);
    assert_eq!(tools[1]["arguments"]["session_id"], "s1");
    assert_eq!(tools[2]["arguments"]["run_id"], "r1");
    assert_eq!(calls.len(), 15);
    assert_eq!(deletes, 5);
    for (headers, _) in &calls {
        assert_eq!(headers["authorization"], "Bearer synthetic-access");
        assert!(!headers.contains_key("x-secret-grant") && !headers.contains_key("cookie"));
    }
    std::fs::write(root.join("completed.json"), serde_json::to_vec_pretty(&json!({
        "status":"passed", "scope":"normal management UI to TCP, SQLite and synthetic MCP",
        "mcpPosts":calls.len(), "transportDeletes":deletes, "generationSubmissions":1,
        "sameTaskRecovered":true,"assetLink":true,"syntheticSessionCredits":9,"revisionUnchanged":true
    })).unwrap()).unwrap();
}
