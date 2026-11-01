//! Explicit opt-in only: ordinary login, then one real public music request, no credential export.
use axum::{
    body::Body,
    http::{Method, Request, StatusCode},
};
use futures::StreamExt;
use neuro_gateway::{
    http::router::build_router, local_runtime::LocalRuntime, routing::config::RouteConfigYaml,
};
use serde_json::{json, Value};
use std::{path::PathBuf, sync::Arc, time::Duration};
use tower::ServiceExt;
#[path = "console_contract_support/mod.rs"]
mod support;
use support::*;

#[test]
#[ignore = "requires explicit FlowMusic login and consumes one music-generation request"]
fn flowmusic_live_music_once() {
    // Match other full-ingress contracts: the debug async dispatch tree needs more than libtest's Windows stack.
    std::thread::Builder::new()
        .stack_size(16 * 1024 * 1024)
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
    assert_eq!(std::env::var("GATEWAY_FLOWMUSIC_LIVE").as_deref(), Ok("1"));
    assert_eq!(
        std::env::var("FLOWMUSIC_LIVE_ALLOW_GENERATION").as_deref(),
        Ok("1"),
        "Generation must be independently authorized after reconciling earlier attempts"
    );
    let root = PathBuf::from(
        std::env::var_os("FLOWMUSIC_LIVE_ROOT").expect("Explicit private output root required"),
    );
    assert!(root.is_absolute());
    std::fs::create_dir_all(&root).unwrap();
    std::env::set_var("AI_GATEWAY_OBJECT_STORAGE_DRIVER", "local");
    std::env::set_var(
        "AI_GATEWAY_OBJECT_STORAGE_LOCAL_DIR",
        std::env::var_os("FLOWMUSIC_CREDENTIAL_ROOT")
            .map(PathBuf::from)
            .unwrap_or_else(|| root.join("credential-objects")),
    );
    std::env::set_var("FLOWMUSIC_CAPTURE_TEMP_ROOT", root.join("browser-import"));
    let mut document: Value = json!({"providers":[{"id":"flowmusic-web","preset":"producer","base_url":"https://www.flowmusic.app",
        "explicit_credentials_only":true,"credentials":[],"supported_models":["producer:standard"],
        "supported_protocol_families":["producer_music"],"execution_mode":"direct_http",
        "call_flow":{"call_adapter":"auto","fingerprint_adapter":"session","special_adapter":"flowmusic_web"}}],
        "model_routes":[{"pattern":"producer:standard","provider_ids":["flowmusic-web"]}],"aliases":{}});
    let mut fixture =
        ConsoleStateFixture::new(serde_json::from_value(document.clone()).unwrap(), true);
    let local = LocalRuntime::open(&root.join("local-runtime"))
        .await
        .unwrap();
    Arc::get_mut(&mut fixture.state).unwrap().local_runtime = Some(local);
    if let Ok(key) = std::env::var("FLOWMUSIC_LIVE_CREDENTIAL_OBJECT_KEY") {
        assert!(
            key.starts_with("credential-runtime/flowmusic/") && key.ends_with("/http-session.json")
        );
        document["providers"][0]["credentials"] = json!([{"id":"flowmusic-live-reused","account_name":"FlowMusic live", "runtime_state_object_key":key,"supported_models":["producer:standard"],"scheduled_probe_enabled":false}]);
    } else {
        let endpoint = "/v1/internal/gateway/console/gemini-auth-sessions";
        let response = send_console_json(&fixture.state, Method::POST, endpoint,
        json!({"targetFamily":"flowmusic-web","providerId":"flowmusic-web","accountLabel":"FlowMusic live"}), None, None).await;
        assert_eq!(response.status(), StatusCode::OK);
        let mut session = parse_json(response).await["session"].clone();
        let id = session["id"].as_str().unwrap().to_string();
        println!("FLOWMUSIC_WAITING_MANUAL_LOGIN: sign in, enter FlowMusic, then close only the dedicated browser window.");
        let deadline = tokio::time::Instant::now() + Duration::from_secs(24 * 60);
        while session["status"] == "waiting_user" || session["status"] == "pending" {
            assert!(tokio::time::Instant::now() < deadline, "Login timed out");
            tokio::time::sleep(Duration::from_secs(2)).await;
            let response = send_console_json(
                &fixture.state,
                Method::GET,
                &format!("{endpoint}/{id}"),
                Value::Null,
                None,
                None,
            )
            .await;
            session = parse_json(response).await["session"].clone();
        }
        if session["status"] != "succeeded" {
            std::fs::write(root.join("live.safe.json"), json!({"stage":"login","status":session["status"],"message":session["message"],"generated":false}).to_string()).unwrap();
            panic!("FlowMusic login import failed; see safe receipt.");
        }
        assert_eq!(session["generatedDrafts"][0]["secretEdits"], json!([]));
        document["providers"][0]["credentials"] =
            json!([session["generatedDrafts"][0]["credential"]]);
    }
    let compiled: RouteConfigYaml = serde_json::from_value(document).unwrap();
    fixture
        .commit_route_document(compiled, Some("FlowMusic isolated live music acceptance"))
        .await;
    // Never silently replay a crashed or failed canary under the same evidence root.
    std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(root.join("generation.submission.guard"))
        .expect("Existing attempt requires read-only reconciliation, not automatic retry");
    println!("FLOWMUSIC_HTTP_MUSIC_START: dedicated browser closed; one public request.");
    let started = std::time::Instant::now();
    let response = Box::pin(build_router(fixture.state.clone()).oneshot(Request::post("/v1/music/generations")
        .header("authorization","Bearer public-api-contract-secret").header("content-type","application/json")
        .body(Body::from(json!({"model":"producer:standard","prompt":"Generate a short original instrumental music track with gentle piano and soft ambient pads, no vocals."}).to_string())).unwrap())).await.unwrap();
    let status = response.status();
    let body = parse_json(response).await;
    let data = body["data"].as_array().cloned().unwrap_or_default();
    let receipt = json!({"stage":"public_music_api","status":status.as_u16(),"elapsedMs":started.elapsed().as_millis(),
        "provider":body["provider"],"completed":body["completed"],"mediaCompleted":body["media_completed"],
        "clipCount":data.len(),"errorCode":body["error"]["code"],"browserImportClosedBeforeGeneration":true,
        "inProcessPublicHandler":true,"fixtureUpstream":false,"singleSelectedCredential":true});
    std::fs::write(
        root.join("live.safe.json"),
        serde_json::to_vec_pretty(&receipt).unwrap(),
    )
    .unwrap();
    println!("FLOWMUSIC_RESULT: {receipt}");
    assert_eq!(
        status,
        StatusCode::OK,
        "Music generation failed; see safe receipt, do not auto-retry."
    );
    assert!(!data.is_empty(), "No music assets were returned");
    // Result URLs are user-created media, not authentication material. Do not retain raw SSE/account library.
    std::fs::write(
        root.join("music-results.json"),
        serde_json::to_vec_pretty(&data).unwrap(),
    )
    .unwrap();
    let url = data[0]["url"].as_str().unwrap();
    assert!(url.starts_with("https://"));
    let response = rquest::Client::new()
        .get(url)
        .header("Range", "bytes=0-1023")
        .timeout(Duration::from_secs(30))
        .send()
        .await
        .unwrap();
    let audio_status = response.status().as_u16();
    let media_type = response
        .headers()
        .get("content-type")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("")
        .to_string();
    let first = response.bytes_stream().next().await.unwrap().unwrap();
    let media =
        json!({"status":audio_status,"contentType":media_type,"nonempty":!first.is_empty()});
    std::fs::write(root.join("media.safe.json"), media.to_string()).unwrap();
    assert!(
        (200..300).contains(&audio_status)
            && !first.is_empty()
            && (media_type.starts_with("audio/") || media_type == "application/octet-stream")
    );
}
