//! Loopback-only synthetic rotation; each child process owns its request and env.
use axum::{
    body::Body,
    extract::State,
    http::HeaderMap,
    response::Response,
    routing::{get, post},
    Router,
};
use std::sync::{Arc, Mutex};

pub const APP: &str = r#"<script>window.WIZ_global_data = {"SNlM0e":"synthetic-at","cfb2h":"synthetic-build","FdrFJe":"synthetic-session"};</script>"#;
#[derive(Default)]
pub struct Capture {
    pub gets: Vec<String>,
    pub posts: Vec<String>,
    pub expected: Option<String>,
    pub fail_post: u16,
    pub delete: bool,
    pub break_storage: Option<std::path::PathBuf>,
    pub invalid_json: bool,
}
fn cookie(headers: &HeaderMap) -> String {
    headers
        .get("cookie")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("")
        .to_owned()
}
fn response(status: u16, content: impl Into<Body>, set_cookie: &str) -> Response {
    Response::builder()
        .status(status)
        .header("set-cookie", set_cookie)
        .body(content.into())
        .unwrap()
}
async fn app(State(state): State<Arc<Mutex<Capture>>>, headers: HeaderMap) -> Response {
    let raw = cookie(&headers);
    let mut state = state.lock().unwrap();
    state.gets.push(raw.clone());
    if state
        .expected
        .as_ref()
        .is_some_and(|expected| !raw.contains(expected))
    {
        return response(401, "<html>sign in</html>", "pref=poison; Path=/");
    }
    response(
        200,
        APP,
        if state.delete {
            "pref=; Path=/; Max-Age=0"
        } else {
            "pref=bootstrap; Path=/"
        },
    )
}
async fn generate(State(state): State<Arc<Mutex<Capture>>>, headers: HeaderMap) -> Response {
    let mut state = state.lock().unwrap();
    state.posts.push(cookie(&headers));
    if state.fail_post != 0 {
        return response(
            state.fail_post,
            "<html>sign in</html>",
            "pref=poison; Path=/",
        );
    }
    if let Some(dir) = state.break_storage.take() {
        let file = dir.join("http-auth-checkpoints-v1.sqlite3");
        std::fs::rename(&file, dir.join("preserved-checkpoint.sqlite3")).unwrap();
        std::fs::write(file, b"synthetic corrupted database").unwrap();
    }
    if state.invalid_json {
        return response(200, "synthetic-invalid", "pref=poison; Path=/");
    }
    let body = serde_json::json!({"candidates":[{"finishReason":"STOP","content":{"parts":[{"text":"synthetic"}]}}]}).to_string();
    response(
        200,
        body,
        if state.delete {
            "pref=; Path=/; Max-Age=0"
        } else {
            "pref=post; Path=/"
        },
    )
}
async fn speech(State(state): State<Arc<Mutex<Capture>>>, headers: HeaderMap) -> Response {
    state.lock().unwrap().posts.push(cookie(&headers));
    response(
        200,
        super::speech_fixture::frame("XqA3Ic", &super::speech_fixture::wav()),
        "pref=post; Path=/",
    )
}
pub struct Fixture {
    pub base: String,
    pub capture: Arc<Mutex<Capture>>,
    task: tokio::task::JoinHandle<()>,
}
impl Drop for Fixture {
    fn drop(&mut self) {
        self.task.abort();
    }
}
pub async fn fixture() -> Fixture {
    let capture = Arc::new(Mutex::new(Capture::default()));
    let app = Router::new()
        .route("/app", get(app))
        .route("/generate", post(generate))
        .route("/_/BardChatUi/data/batchexecute", post(speech))
        .with_state(capture.clone());
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());
    let task = tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    Fixture {
        base,
        capture,
        task,
    }
}
