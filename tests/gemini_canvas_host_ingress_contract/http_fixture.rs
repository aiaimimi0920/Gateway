//! Synthetic q4uTj exchange; no browser, external service or real credential is used.
use axum::{
    extract::{Request, State},
    http::StatusCode,
    response::IntoResponse,
    routing::any,
    Router,
};
use serde_json::{json, Value};
use std::sync::{Arc, Mutex};

#[derive(Default)]
pub(super) struct Capture {
    pub seen: Vec<Value>,
    pub reject: bool,
}

pub(super) struct Host {
    pub base: String,
    pub capture: Arc<Mutex<Capture>>,
    task: tokio::task::JoinHandle<()>,
}

impl Drop for Host {
    fn drop(&mut self) {
        self.task.abort();
    }
}

async fn receive(State(state): State<Arc<Mutex<Capture>>>, req: Request) -> impl IntoResponse {
    if req.uri().path() == "/app" {
        return (StatusCode::OK, r#"<script>WIZ_global_data={"cfb2h":"fixture-build","FdrFJe":"fixture-session"};</script>"#.to_owned());
    }
    assert_eq!(req.method(), axum::http::Method::POST);
    assert_eq!(req.uri().path(), "/_/BardChatUi/data/batchexecute");
    let cookie = req.headers()["cookie"].to_str().unwrap().to_owned();
    assert!(req.headers()["authorization"]
        .to_str()
        .unwrap()
        .starts_with("SAPISIDHASH "));
    let body = axum::body::to_bytes(req.into_body(), 1024 * 1024)
        .await
        .unwrap();
    let form: std::collections::HashMap<_, _> =
        url::form_urlencoded::parse(&body).into_owned().collect();
    let envelope: Value = serde_json::from_str(&form["f.req"]).unwrap();
    assert_eq!(envelope[0][0][0], "q4uTj");
    let request: Value = serde_json::from_str(envelope[0][0][1].as_str().unwrap()).unwrap();
    assert_eq!(request[2], 1);
    let input: Value = serde_json::from_str(request[1].as_str().unwrap()).unwrap();
    assert_eq!(input["model"], "gemini-3-flash-preview");
    let mut capture = state.lock().unwrap();
    capture.seen.push(json!({"body":input,"cookie":cookie}));
    if capture.reject {
        return (StatusCode::BAD_GATEWAY, "synthetic rejected".into());
    }
    let answer =
        json!({"candidates":[{"finishReason":"STOP","content":{"parts":[{"text":"合成回执"}]}}]});
    (
        StatusCode::OK,
        json!([[
            "wrb.fr",
            "q4uTj",
            json!([answer.to_string()]).to_string(),
            null,
            null,
            null,
            "generic"
        ]])
        .to_string(),
    )
}

impl Host {
    pub async fn start() -> Self {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let base = format!("http://{}", listener.local_addr().unwrap());
        let capture = Arc::new(Mutex::new(Capture::default()));
        let app = Router::new()
            .fallback(any(receive))
            .with_state(capture.clone());
        let task = tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
        Self {
            base,
            capture,
            task,
        }
    }

    pub fn payload(&self) -> crate::routing::candidate::ProviderAccountPayload {
        serde_json::from_value(json!({"adapter":"gemini_canvas_web_reverse_compatible",
            "base_url":self.base,"api_key":"must-not-forward","extra_body":selected_extra()}))
        .unwrap()
    }
}

pub(super) fn selected_extra() -> Value {
    json!({"cookieHeader":"SAPISID=selected-signing-cookie; SID=selected-login-cookie","pureHttpMode":"disabled"})
}

pub(super) fn route_document(base: &str) -> Vec<u8> {
    serde_json::to_vec(&json!({"providers":[{"id":"fixture-provider","preset":"gemini-canvas-browser-relay",
        "base_url":base,"supported_models":["gemini-3-flash-preview"],
        "credentials":[{"id":"fixture-credential","api_key":"must-not-forward","extra_body":selected_extra()}]}],
        "model_routes":[{"pattern":"gemini-3-flash-preview","provider_ids":["fixture-provider"]}]})).unwrap()
}
