//! Explicitly approved, ignored real-HTTP canary. Browser credentials cross a one-shot loopback
//! bridge in memory, then enter the normal management import and public chat pipeline.
use axum::{
    body::{Body, Bytes},
    extract::{DefaultBodyLimit, State},
    http::{HeaderMap, Method, Request, StatusCode},
    routing::post,
    Router,
};
use futures::StreamExt;
use neuro_gateway::{http::router::build_router, routing::config::RouteConfigYaml};
use serde_json::{json, Value};
use std::{
    path::{Path, PathBuf},
    sync::Arc,
    time::Duration,
};
use tokio::sync::{mpsc, oneshot};
use tower::ServiceExt;
#[path = "console_contract_support/mod.rs"]
mod support;

#[derive(Clone)]
struct Bridge {
    nonce: String,
    sender: mpsc::Sender<Vec<u8>>,
}
async fn receive(State(bridge): State<Bridge>, headers: HeaderMap, body: Bytes) -> StatusCode {
    if headers.get("x-import-nonce").and_then(|v| v.to_str().ok()) != Some(&bridge.nonce) {
        return StatusCode::FORBIDDEN;
    }
    match bridge.sender.try_send(body.to_vec()) {
        Ok(()) => StatusCode::NO_CONTENT,
        Err(_) => StatusCode::CONFLICT,
    }
}

async fn credential(root: &Path) -> Vec<u8> {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let nonce = uuid::Uuid::new_v4().to_string();
    let (sender, mut receiver) = mpsc::channel(1);
    let (stop, stopped) = oneshot::channel::<()>();
    let router = Router::new()
        .route("/import", post(receive))
        .layer(DefaultBodyLimit::max(32768))
        .with_state(Bridge {
            nonce: nonce.clone(),
            sender,
        });
    let task = tokio::spawn(async move {
        axum::serve(listener, router)
            .with_graceful_shutdown(async {
                let _ = stopped.await;
            })
            .await
            .unwrap();
    });
    let bridge_path = root.join("gateway-bridge.json");
    std::fs::write(
        &bridge_path,
        json!({"endpoint":format!("http://{address}/import"),"nonce":nonce}).to_string(),
    )
    .unwrap();
    println!("stitch_web_live awaiting in-memory credential bridge");
    let bytes = tokio::time::timeout(Duration::from_secs(480), receiver.recv()).await;
    let _ = stop.send(());
    task.await.unwrap();
    std::fs::remove_file(bridge_path).unwrap();
    bytes
        .expect("credential bridge timed out")
        .expect("credential bridge closed")
}

fn document() -> RouteConfigYaml {
    serde_json::from_value(json!({"providers":[{"id":"stitch-live","preset":"stitch-web",
        "base_url":"https://stitch.withgoogle.com","explicit_credentials_only":true,"credentials":[],
        "supported_models":["stitch-design"],"call_flow":{"special_adapter":"stitch_web","fingerprint_adapter":"session"}}],
        "model_routes":[{"pattern":"stitch-design","provider_ids":["stitch-live"]}],"aliases":{}})).unwrap()
}

async fn verify_html(root: &Path, manifest: &Value) -> usize {
    let raw = manifest["screens"][0]["html_url"]
        .as_str()
        .expect("HTML artifact required");
    let url = url::Url::parse(raw).unwrap();
    assert_eq!(
        url.origin().ascii_serialization(),
        "https://contribution.usercontent.google.com"
    );
    assert_eq!(url.path(), "/download");
    assert!(url.username().is_empty() && url.password().is_none());
    // Artifact fetch is unauthenticated, bounded and never executes generated HTML.
    let client = neuro_gateway::http_client::builder()
        .redirect(rquest::redirect::Policy::none())
        .timeout(Duration::from_secs(30))
        .build()
        .unwrap();
    let response = client.get(url.as_str()).send().await.unwrap();
    assert_eq!(response.status().as_u16(), 200);
    assert!(response
        .headers()
        .get("content-type")
        .unwrap()
        .to_str()
        .unwrap()
        .starts_with("text/html"));
    let mut bytes = Vec::new();
    let mut stream = response.bytes_stream();
    while let Some(chunk) = stream.next().await {
        let chunk = chunk.unwrap();
        assert!(bytes.len().saturating_add(chunk.len()) <= 2 * 1024 * 1024);
        bytes.extend_from_slice(&chunk);
    }
    let html = std::str::from_utf8(&bytes).unwrap();
    assert!(
        html.to_ascii_lowercase().contains("<html") && html.to_ascii_lowercase().contains("<body")
    );
    std::fs::write(root.join("public-api-screen.html"), &bytes).unwrap();
    bytes.len()
}

#[test]
#[ignore = "requires explicit approval for one real Stitch generation and a live in-memory login bridge"]
fn stitch_web_live_public_chat_once() {
    // Debug builds of the full public ingress have a large future. Keep this test's
    // stack requirement local rather than relying on the caller's RUST_MIN_STACK.
    std::thread::Builder::new()
        .name("stitch-http-live".into())
        .stack_size(64 * 1024 * 1024)
        .spawn(|| {
            tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .unwrap()
                .block_on(run_once())
        })
        .unwrap()
        .join()
        .unwrap();
}

async fn run_once() {
    assert_eq!(
        std::env::var("STITCH_LIVE_ALLOW_GENERATION").as_deref(),
        Ok("1")
    );
    let root =
        PathBuf::from(std::env::var("STITCH_LIVE_ROOT").expect("owned temporary root required"));
    let owned =
        Path::new("C:/Users/Public/nas_home/AI/GameEditor/linshi/gateway-stitch-web-20261101");
    std::fs::create_dir_all(&root).unwrap();
    assert!(std::fs::canonicalize(&root)
        .unwrap()
        .starts_with(std::fs::canonicalize(owned).unwrap()));
    std::env::set_var("AI_GATEWAY_OBJECT_STORAGE_DRIVER", "local");
    std::env::set_var(
        "AI_GATEWAY_OBJECT_STORAGE_LOCAL_DIR",
        root.join("private-objects"),
    );
    let bytes = credential(&root).await;
    let mut fixture = support::ConsoleStateFixture::new(document(), true);
    let state = Arc::get_mut(&mut fixture.state).unwrap();
    state.config.upstream_timeout_secs = 180;
    state.upstream_client = neuro_gateway::upstream::client::UpstreamClient::new(180);
    let grant = support::grant_console_secret_access(&fixture.state).await;
    let response = support::send_console_json(&fixture.state, Method::POST,
        "/v1/internal/gateway/console/gemini-auth-sessions", json!({"targetFamily":"stitch-web","providerId":"stitch-live",
        "expectedRevision":fixture.state.route_config.snapshot().revision().id(),"accountLabel":"Stitch live HTTP",
        "webSessionJson":std::str::from_utf8(&bytes).unwrap()}), Some(&grant), None).await;
    assert_eq!(
        response.status(),
        StatusCode::OK,
        "normal private import must succeed"
    );
    let imported = support::parse_json(response).await;
    let mut credential = imported["session"]["generatedDrafts"][0]["credential"].clone();
    // Explicit routing hint established by the separately approved live protocol canary.
    credential["supported_models"] = json!(["stitch-design"]);
    credential["model_selection"] = json!({"fetched":[],"manual":["stitch-design"]});
    let object_key = credential["runtime_state_object_key"]
        .as_str()
        .unwrap()
        .to_owned();
    let mut doc = serde_json::to_value(document()).unwrap();
    doc["providers"][0]["credentials"] = json!([credential]);
    fixture
        .commit_route_document(
            serde_json::from_value(doc).unwrap(),
            Some("isolated Stitch live canary"),
        )
        .await;
    // Persist the one-shot guard before the public ingress. A lost response must not trigger another generation.
    std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(root.join("public-generation-attempted"))
        .unwrap();
    let request = Request::post("/v1/chat/completions").header("authorization", "Bearer public-api-contract-secret")
        .header("content-type", "application/json").body(Body::from(json!({"model":"stitch-design","messages":[{"role":"user",
            "content":"Design a simple single-screen web reading list with three sample books and a blue Add book button. No login form or external integrations."}]}).to_string())).unwrap();
    let response = build_router(fixture.state.clone())
        .oneshot(request)
        .await
        .unwrap();
    let status = response.status();
    let body = support::parse_json(response).await;
    // The Cookie object stays within this explicitly isolated private storage owner, never a session export.
    neuro_gateway::object_storage::gateway_object_storage()
        .unwrap()
        .delete_object(&object_key)
        .await
        .unwrap();
    if status != StatusCode::OK {
        let receipt = json!({"status":"failed","httpStatus":status.as_u16(),"errorCode":body["error"]["code"],"browserUsed":false});
        std::fs::write(root.join("public-api.safe.json"), receipt.to_string()).unwrap();
        panic!(
            "public Stitch request failed: status={}, code={}",
            status, body["error"]["code"]
        );
    }
    let manifest: Value =
        serde_json::from_str(body["choices"][0]["message"]["content"].as_str().unwrap()).unwrap();
    assert_eq!(manifest["provider"], "stitch-web");
    let html_bytes = verify_html(&root, &manifest).await;
    let receipt = json!({"status":"passed","endpoint":"/v1/chat/completions","httpStatus":200,"browserUsed":false,
        "model":body["model"],"finishReason":body["choices"][0]["finish_reason"],"project":manifest["project"],
        "projectUrl":manifest["project_url"],"screenCount":manifest["screens"].as_array().unwrap().len(),"htmlBytes":html_bytes,
        "runtimeCookieObjectDeleted":true,"credentialSessionExported":false});
    std::fs::write(
        root.join("public-api.safe.json"),
        serde_json::to_vec_pretty(&receipt).unwrap(),
    )
    .unwrap();
    println!("stitch_web_live public API HTTP 200, HTML bytes={html_bytes}, browserUsed=false");
}
