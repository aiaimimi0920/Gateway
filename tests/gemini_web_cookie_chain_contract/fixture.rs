//! Only synthetic Cookie material is served over loopback; all tasks are abort-owned.
use axum::{
    body::Body,
    extract::State,
    http::{HeaderMap, StatusCode},
    response::Response,
    routing::{get, post},
    Router,
};
use std::sync::{Arc, Mutex};

pub const APP: &str = r#"<script>window.WIZ_global_data = {"SNlM0e":"synthetic-at","cfb2h":"synthetic-build","FdrFJe":"synthetic-session"};</script>"#;

#[derive(Default)]
pub struct Capture {
    pub gets: usize,
    pub targets: usize,
    pub initial_cookies: Vec<String>,
    pub cookies: Vec<String>,
}

pub struct Options {
    pub cookies: Vec<&'static str>,
    pub status: StatusCode,
    pub app: String,
    pub speech: bool,
    pub chunked: bool,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            cookies: vec![],
            status: StatusCode::OK,
            app: APP.into(),
            speech: false,
            chunked: false,
        }
    }
}

#[derive(Clone)]
struct FixtureState {
    capture: Arc<Mutex<Capture>>,
    options: Arc<Options>,
}

async fn app(State(state): State<FixtureState>, request: HeaderMap) -> Response {
    state.capture.lock().unwrap().gets += 1;
    state.capture.lock().unwrap().initial_cookies.push(
        request
            .get("cookie")
            .and_then(|v| v.to_str().ok())
            .unwrap_or("")
            .into(),
    );
    let mut headers = HeaderMap::new();
    headers.insert("content-type", "text/html".parse().unwrap());
    headers.insert("location", "/target".parse().unwrap());
    for cookie in &state.options.cookies {
        headers.append("set-cookie", cookie.parse().unwrap());
    }
    let body = if state.options.chunked {
        Body::from_stream(futures::stream::iter([Ok::<_, std::convert::Infallible>(
            state.options.app.clone(),
        )]))
    } else {
        Body::from(state.options.app.clone())
    };
    let mut response = Response::new(body);
    *response.status_mut() = state.options.status;
    *response.headers_mut() = headers;
    response
}

async fn target(State(state): State<FixtureState>) -> &'static str {
    state.capture.lock().unwrap().targets += 1;
    APP
}

async fn generate(State(state): State<FixtureState>, headers: HeaderMap) -> String {
    state.capture.lock().unwrap().cookies.push(
        headers
            .get("cookie")
            .and_then(|v| v.to_str().ok())
            .unwrap_or("")
            .into(),
    );
    if state.options.speech {
        super::speech_fixture::frame("XqA3Ic", &super::speech_fixture::wav())
    } else {
        serde_json::json!({"candidates":[{"finishReason":"STOP","content":{"parts":[{"text":"synthetic"}]}}]}).to_string()
    }
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

pub async fn fixture(options: Options) -> Fixture {
    let capture = Arc::new(Mutex::new(Capture::default()));
    let app = Router::new()
        .route("/app", get(app))
        .route("/target", get(target))
        .route("/generate", post(generate))
        .route("/_/BardChatUi/data/batchexecute", post(generate))
        .with_state(FixtureState {
            capture: capture.clone(),
            options: Arc::new(options),
        });
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());
    let task = tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    Fixture {
        base,
        capture,
        task,
    }
}
