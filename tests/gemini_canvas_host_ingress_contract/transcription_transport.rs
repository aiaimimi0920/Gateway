//! Real loopback worker transport, synthetic identity and a decodable MP3 tone.
//! Stub transcript text proves wiring only; it is not live-model recognition evidence.
use super::{candidate, ProviderExecutionMode};
use axum::{
    extract::State,
    routing::{get, post},
    Json, Router,
};
use base64::Engine;
use neuro_gateway::{error::FallbackHint, protocol::openai, upstream::client::UpstreamClient};
use serde_json::{json, Value};
use std::{
    sync::{Arc, Mutex},
    time::Duration,
};

struct IsolatedPoolEnv(Option<std::ffi::OsString>);
impl Drop for IsolatedPoolEnv {
    fn drop(&mut self) {
        if let Some(value) = &self.0 {
            std::env::set_var("GEMINI_CANVAS_BROWSER_POOL_BASE_URL", value);
        } else {
            std::env::remove_var("GEMINI_CANVAS_BROWSER_POOL_BASE_URL");
        }
    }
}

#[derive(Default)]
struct Capture {
    seen: Vec<Value>,
    reject: bool,
}

async fn fetch(State(state): State<Arc<Mutex<Capture>>>, Json(body): Json<Value>) -> Json<Value> {
    let mut capture = state.lock().unwrap();
    capture.seen.push(body);
    if capture.reject {
        return Json(
            json!({"ok":false,"error":{"code":"fixture_host_rejected","status":502,"message":"fixture rejection"}}),
        );
    }
    Json(
        json!({"ok":true,"result":{"operation":"fetch","status":200,"contentType":"application/json","bodyText":
            json!({"candidates":[{"finishReason":"STOP","content":{"parts":[{"text":"合成回执"}]}}]}).to_string()
        }}),
    )
}

struct Server(tokio::task::JoinHandle<()>);
impl Drop for Server {
    fn drop(&mut self) {
        self.0.abort();
    }
}

#[test]
fn transcription_json_and_text_keep_selected_identity_exact_mp3_and_no_replay() {
    // The real pipeline has a larger stack budget than Windows test threads' 2 MiB.
    std::thread::Builder::new()
        .stack_size(16 * 1024 * 1024)
        .spawn(|| {
            tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .unwrap()
                .block_on(run_transport());
        })
        .unwrap()
        .join()
        .unwrap();
}

async fn run_transport() {
    // This test executable owns its environment; CI runs it with --test-threads=1.
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());
    let _environment = IsolatedPoolEnv(std::env::var_os("GEMINI_CANVAS_BROWSER_POOL_BASE_URL"));
    std::env::set_var("GEMINI_CANVAS_BROWSER_POOL_BASE_URL", &base);
    let state = Arc::new(Mutex::new(Capture::default()));
    let app = Router::new()
        .route("/health", get(|| async { "OK" }))
        .route("/fetch", post(fetch))
        .with_state(state.clone());
    let _server = Server(tokio::spawn(async {
        axum::serve(listener, app).await.unwrap();
    }));
    tokio::time::timeout(Duration::from_secs(10), async {
        let client = UpstreamClient::new(2);
        let mut selected = candidate("gemini_canvas_web_reverse_compatible");
        selected.payload.runtime_state_object_key = Some("fixture/selected-session.json".into());
        let audio = include_bytes!("../fixtures/gemini/transcription-tone.mp3");
        let data = base64::engine::general_purpose::STANDARD.encode(audio);
        let raw = json!({"model":"gemini-3-flash-preview","file":{
            "file_name":"tone.mp3","mime_type":"audio/mpeg","base64":data
        },"language":"zh","temperature":"0"});
        let req = openai::normalize_audio_transcriptions(raw.clone()).unwrap();
        let reply = client
            .execute_json_passthrough(
                "fixture-provider",
                ProviderExecutionMode::BrowserBacked,
                &selected.payload,
                &req,
                "gemini-3-flash-preview",
                None,
            )
            .await
            .unwrap();
        assert_eq!(reply, json!({"text":"合成回执"}));
        let mut raw_text = raw.clone();
        raw_text["response_format"] = json!("text");
        let text_req = openai::normalize_audio_transcriptions(raw_text).unwrap();
        let text = client
            .execute_binary_passthrough(
                &selected.payload,
                &text_req,
                "gemini-3-flash-preview",
                None,
            )
            .await
            .unwrap();
        assert_eq!(
            text.content_type.as_deref(),
            Some("text/plain; charset=utf-8")
        );
        assert_eq!(text.body.as_ref(), "合成回执".as_bytes());
        assert_eq!(state.lock().unwrap().seen.len(), 2);
        let mut unsupported = raw;
        unsupported["response_format"] = json!("srt");
        let bad = openai::normalize_audio_transcriptions(unsupported).unwrap();
        assert!(client
            .execute_binary_passthrough(&selected.payload, &bad, "gemini-3-flash-preview", None)
            .await
            .is_err());
        assert_eq!(
            state.lock().unwrap().seen.len(),
            2,
            "invalid options must not reach the worker"
        );
        super::transcription_http::exercise(audio).await;
        assert_eq!(
            state.lock().unwrap().seen.len(),
            4,
            "only the two authorized valid multipart requests generate"
        );
        state.lock().unwrap().reject = true;
        let error = client
            .execute_json_passthrough(
                "fixture-provider",
                ProviderExecutionMode::BrowserBacked,
                &selected.payload,
                &req,
                "gemini-3-flash-preview",
                None,
            )
            .await
            .unwrap_err();
        assert!(!error.retryable);
        assert!(matches!(error.fallback_hint, FallbackHint::Abort { .. }));
        let capture = state.lock().unwrap();
        assert_eq!(capture.seen.len(), 5, "failed generation must not replay");
        for invocation in &capture.seen {
            assert_eq!(
                invocation["runtimeStateObjectKey"],
                "fixture/selected-session.json"
            );
            assert_eq!(invocation["googleFetchMode"], "canvas_proxy");
            assert!(invocation["fetchRequest"]["url"]
                .as_str()
                .unwrap()
                .ends_with("/models/gemini-3-flash-preview:generateContent"));
            let body = &invocation["fetchRequest"]["jsonBody"];
            let inline = &body["contents"][0]["parts"][0]["inlineData"];
            assert_eq!(inline["mimeType"], "audio/mpeg");
            assert_eq!(
                base64::engine::general_purpose::STANDARD
                    .decode(inline["data"].as_str().unwrap())
                    .unwrap(),
                audio
            );
            assert!(body.get("apiKey").is_none());
        }
    })
    .await
    .expect("bounded synthetic worker test");
}
