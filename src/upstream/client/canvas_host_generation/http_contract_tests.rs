//! Loopback proof that the selected Canvas account needs no request-time browser.
use super::*;
use axum::{extract::State, http::StatusCode, response::IntoResponse, routing::any, Router};
use std::sync::{Arc, Mutex};

#[derive(Clone)]
struct Fixture {
    requests: Arc<Mutex<Vec<(String, String)>>>,
    status: StatusCode,
    response: String,
    bootstrap_status: StatusCode,
    delay: Duration,
}

fn rpc_response() -> String {
    let answer = json!({"candidates":[{"finishReason":"STOP","content":{"parts":[{"text":"fixture-answer"}]}}]});
    json!([[
        "wrb.fr",
        "q4uTj",
        json!([answer.to_string()]).to_string(),
        null,
        null,
        null,
        "generic"
    ]])
    .to_string()
}

async fn handler(State(state): State<Fixture>, req: axum::extract::Request) -> impl IntoResponse {
    let path = req.uri().path().to_string();
    let uri = req.uri().to_string();
    let body = axum::body::to_bytes(req.into_body(), 1024 * 1024)
        .await
        .unwrap();
    state
        .requests
        .lock()
        .unwrap()
        .push((uri, String::from_utf8(body.to_vec()).unwrap()));
    if path.ends_with("/app") {
        return (state.bootstrap_status, [("location", "/redirect-must-not-be-followed")], r#"<script>WIZ_global_data={"cfb2h":"fixture-build","FdrFJe":"fixture-session"};</script>"#.to_string());
    }
    tokio::time::sleep(state.delay).await;
    (
        state.status,
        [("location", "/redirect-must-not-be-followed")],
        state.response.clone(),
    )
}

async fn fixture(
    status: StatusCode,
    response: String,
) -> (ProviderAccountPayload, Fixture, tokio::task::JoinHandle<()>) {
    fixture_options(status, response, StatusCode::OK, Duration::ZERO).await
}

async fn fixture_options(
    status: StatusCode,
    response: String,
    bootstrap_status: StatusCode,
    delay: Duration,
) -> (ProviderAccountPayload, Fixture, tokio::task::JoinHandle<()>) {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let state = Fixture {
        requests: Default::default(),
        status,
        response,
        bootstrap_status,
        delay,
    };
    let payload = serde_json::from_value(json!({
        "adapter":"gemini_canvas_web_reverse_compatible", "api_key":"must-not-forward",
        "base_url":format!("http://{}",listener.local_addr().unwrap()),
        "execution_mode":"browser_backed", "extra_body":{"shareId":"fixture","pureHttpMode":"disabled",
            "cookieHeader":"SAPISID=fixture-signing-cookie; SID=fixture-login-cookie"}
    })).unwrap();
    let app = Router::new()
        .fallback(any(handler))
        .with_state(state.clone());
    let task = tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    (payload, state, task)
}

#[tokio::test]
async fn canvas_host_generation_works_with_browsers_disabled_even_for_old_browser_backed_accounts()
{
    let (payload, state, task) = fixture(StatusCode::OK, rpc_response()).await;
    let mut client = UpstreamClient::new(3);
    client.http = crate::http_client::builder().no_proxy().build().unwrap();
    client.request_time_browser_policy = RequestTimeBrowserPolicy::Disabled;
    let result = client
        .execute_gemini_canvas_host_generate(
            &payload,
            "gemini-3-flash-preview",
            &json!({"contents":[{"parts":[{"text":"fixture prompt"}]}]}),
            Duration::from_secs(3),
        )
        .await;
    task.abort();
    let result = result.expect("Canvas must call q4uTj directly without starting a browser");
    assert_eq!(
        result["candidates"][0]["content"]["parts"][0]["text"],
        "fixture-answer"
    );
    let requests = state.requests.lock().unwrap();
    assert_eq!(requests.len(), 2);
    assert!(requests[1]
        .0
        .starts_with("/_/BardChatUi/data/batchexecute?"));
    let form: std::collections::HashMap<_, _> =
        url::form_urlencoded::parse(requests[1].1.as_bytes())
            .into_owned()
            .collect();
    let envelope: Value = serde_json::from_str(&form["f.req"]).unwrap();
    assert_eq!(envelope[0][0][0], "q4uTj");
    let request: Value = serde_json::from_str(envelope[0][0][1].as_str().unwrap()).unwrap();
    assert_eq!(request[2], 1);
    assert!(!requests[1].1.contains("must-not-forward"));
}

#[tokio::test]
async fn canvas_host_http_error_never_retries_or_launches_a_browser() {
    let (payload, state, task) = fixture(
        StatusCode::SERVICE_UNAVAILABLE,
        "private-secret-error".into(),
    )
    .await;
    let mut client = UpstreamClient::new(3);
    client.http = crate::http_client::builder().no_proxy().build().unwrap();
    client.request_time_browser_policy = RequestTimeBrowserPolicy::Disabled;
    let result = client
        .execute_gemini_canvas_host_generate(
            &payload,
            "gemini-3-flash-preview",
            &json!({"contents":[]}),
            Duration::from_secs(3),
        )
        .await;
    task.abort();
    let error = result.unwrap_err();
    assert_eq!(
        error.code.as_deref(),
        Some("gemini_canvas_http_upstream_failed")
    );
    assert!(!error.message.contains("private-secret"));
    assert!(!error.retryable);
    assert!(matches!(
        error.fallback_hint,
        crate::error::FallbackHint::Abort { .. }
    ));
    assert_eq!(state.requests.lock().unwrap().len(), 2);
}

#[tokio::test]
async fn canvas_http_redirect_is_not_followed_and_nonzero_account_is_scoped() {
    let (mut payload, state, task) = fixture(StatusCode::FOUND, "private-redirect".into()).await;
    payload
        .extra_body
        .as_mut()
        .unwrap()
        .insert("authUser".into(), json!("2"));
    let mut client = UpstreamClient::new(3);
    client.http = crate::http_client::builder().no_proxy().build().unwrap();
    let result = client
        .execute_gemini_canvas_host_generate(
            &payload,
            "gemini-3-flash-preview",
            &json!({"contents":[]}),
            Duration::from_secs(3),
        )
        .await;
    task.abort();
    assert_eq!(
        result.unwrap_err().code.as_deref(),
        Some("gemini_canvas_http_redirect_rejected")
    );
    let requests = state.requests.lock().unwrap();
    assert_eq!(requests.len(), 2);
    assert!(requests[0].0.starts_with("/u/2/app?"));
    assert!(requests[1].0.contains("authuser=2"));
}

#[tokio::test]
async fn canvas_http_unknown_model_and_elapsed_deadline_send_nothing() {
    let (payload, state, task) = fixture(StatusCode::OK, rpc_response()).await;
    let mut client = UpstreamClient::new(3);
    client.http = crate::http_client::builder().no_proxy().build().unwrap();
    for (model, timeout) in [
        ("unsupported", Duration::from_secs(3)),
        ("gemini-3-flash-preview", Duration::ZERO),
    ] {
        assert!(client
            .execute_gemini_canvas_host_generate(&payload, model, &json!({"contents":[]}), timeout)
            .await
            .is_err());
    }
    task.abort();
    assert!(state.requests.lock().unwrap().is_empty());
}

#[tokio::test]
async fn canvas_http_bootstrap_redirect_stops_before_generation() {
    let (payload, state, task) = fixture_options(
        StatusCode::OK,
        rpc_response(),
        StatusCode::FOUND,
        Duration::ZERO,
    )
    .await;
    let mut client = UpstreamClient::new(3);
    client.http = crate::http_client::builder().no_proxy().build().unwrap();
    let result = client
        .execute_gemini_canvas_host_generate(
            &payload,
            "gemini-3-flash-preview",
            &json!({"contents":[]}),
            Duration::from_secs(3),
        )
        .await;
    task.abort();
    assert_eq!(
        result.unwrap_err().code.as_deref(),
        Some("gemini_canvas_http_redirect_rejected")
    );
    assert_eq!(state.requests.lock().unwrap().len(), 1);
}

#[tokio::test]
async fn canvas_http_deadline_covers_the_whole_exchange_without_retry() {
    let (payload, state, task) = fixture_options(
        StatusCode::OK,
        rpc_response(),
        StatusCode::OK,
        Duration::from_secs(1),
    )
    .await;
    let mut client = UpstreamClient::new(3);
    client.http = crate::http_client::builder().no_proxy().build().unwrap();
    let started = std::time::Instant::now();
    let result = client
        .execute_gemini_canvas_host_generate(
            &payload,
            "gemini-3-flash-preview",
            &json!({"contents":[]}),
            Duration::from_millis(200),
        )
        .await;
    task.abort();
    assert!(result.is_err());
    assert!(started.elapsed() < Duration::from_secs(1));
    assert_eq!(state.requests.lock().unwrap().len(), 2);
}
