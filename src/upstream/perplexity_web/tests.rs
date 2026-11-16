use super::*;
use axum::{
    body::Body, extract::Request, http::StatusCode, response::Response, routing::any, Router,
};
use bytes::Bytes;
use futures::Stream;
use serde_json::{json, Value};
use std::{
    collections::VecDeque,
    pin::Pin,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex,
    },
    task::{Context, Poll},
};

pub(super) struct Fixture {
    pub url: String,
    pub seen: Arc<Mutex<Vec<(String, axum::http::HeaderMap, Value)>>>,
    pub dropped: Arc<AtomicBool>,
    pub polled: Arc<AtomicBool>,
    task: tokio::task::JoinHandle<()>,
}
impl Drop for Fixture {
    fn drop(&mut self) {
        self.task.abort();
    }
}

struct TrackedBody {
    chunks: VecDeque<Bytes>,
    pending: bool,
    dropped: Arc<AtomicBool>,
    polled: Arc<AtomicBool>,
}
impl Stream for TrackedBody {
    type Item = Result<Bytes, std::io::Error>;
    fn poll_next(mut self: Pin<&mut Self>, _: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        self.polled.store(true, Ordering::SeqCst);
        if let Some(bytes) = self.chunks.pop_front() {
            Poll::Ready(Some(Ok(bytes)))
        } else if self.pending {
            Poll::Pending
        } else {
            Poll::Ready(None)
        }
    }
}
impl Drop for TrackedBody {
    fn drop(&mut self) {
        self.dropped.store(true, Ordering::SeqCst);
    }
}

pub(super) async fn fixture(
    status: u16,
    content_type: &'static str,
    chunks: Vec<Bytes>,
    pending: bool,
    location: Option<String>,
) -> Fixture {
    let seen = Arc::new(Mutex::new(Vec::new()));
    let dropped = Arc::new(AtomicBool::new(false));
    let polled = Arc::new(AtomicBool::new(false));
    let (captured, released, started) = (seen.clone(), dropped.clone(), polled.clone());
    let app = Router::new().route(
        "/rest/sse/perplexity_ask",
        any(move |request: Request| {
            let (seen, dropped, polled, chunks, location) = (
                captured.clone(),
                released.clone(),
                started.clone(),
                chunks.clone(),
                location.clone(),
            );
            async move {
                let (parts, body) = request.into_parts();
                let bytes = axum::body::to_bytes(body, 128 * 1024).await.unwrap();
                let body: Value = serde_json::from_slice(&bytes).unwrap();
                seen.lock()
                    .unwrap()
                    .push((parts.uri.to_string(), parts.headers, body));
                let mut response = Response::builder()
                    .status(StatusCode::from_u16(status).unwrap())
                    .header("content-type", content_type);
                if let Some(location) = location {
                    response = response.header("location", location);
                }
                response
                    .body(Body::from_stream(TrackedBody {
                        chunks: chunks.into(),
                        pending,
                        dropped,
                        polled,
                    }))
                    .unwrap()
            }
        }),
    );
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let task = tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });
    Fixture {
        url: format!("http://{address}/rest/sse/perplexity_ask"),
        seen,
        dropped,
        polled,
        task,
    }
}

pub(super) fn request() -> CanonicalRelayRequest {
    crate::protocol::openai::normalize_chat_completions(
        json!({"model":"turbo","messages":[{"role":"user","content":"合成问题"}]}),
    )
    .unwrap()
}
pub(super) fn session() -> Session {
    Session::from_tokens(
        "synthetic-private-session",
        Some("synthetic-private-clearance"),
        "fixture-agent",
    )
    .unwrap()
}
pub(super) fn client() -> rquest::Client {
    rquest::Client::builder().no_proxy().build().unwrap()
}
pub(super) fn answer_wire() -> Bytes {
    Bytes::from(format!(
        "data: {}\n\nevent: end_of_stream\n\n",
        json!({"blocks":[{"intended_usage":"ask_text","markdown_block":{"answer":"合成答案"}}],"read_write_token":"private-followup"})
    ))
}
pub(super) async fn dropped(fixture: &Fixture) {
    tokio::time::timeout(Duration::from_secs(2), async {
        while !fixture.dropped.load(Ordering::SeqCst) {
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .expect("response body must be dropped");
}

#[tokio::test]
async fn perplexity_web_loopback_uses_cookie_profile_and_never_default_api_headers() {
    let fixture = fixture(
        200,
        "text/event-stream; charset=utf-8",
        vec![answer_wire()],
        true,
        None,
    )
    .await;
    let mut defaults = rquest::header::HeaderMap::new();
    defaults.insert(
        "authorization",
        rquest::header::HeaderValue::from_static("Bearer foreign-key"),
    );
    defaults.insert(
        "x-api-key",
        rquest::header::HeaderValue::from_static("foreign-key"),
    );
    let client = rquest::Client::builder()
        .no_proxy()
        .default_headers(defaults)
        .build()
        .unwrap();
    let answer = exchange(
        &client,
        &session(),
        &request(),
        Duration::from_secs(2),
        &fixture.url,
    )
    .await
    .unwrap();
    assert_eq!(answer.text, "合成答案");
    assert!(!format!("{answer:?}").contains("private-followup"));
    let seen = fixture.seen.lock().unwrap();
    assert_eq!(seen.len(), 1);
    assert_eq!(seen[0].0, "/rest/sse/perplexity_ask");
    assert!(!seen[0].1.contains_key("authorization"));
    assert!(!seen[0].1.contains_key("x-api-key"));
    assert_eq!(seen[0].1["cookie"], "__Secure-next-auth.session-token=synthetic-private-session; cf_clearance=synthetic-private-clearance");
    assert_eq!(seen[0].1["origin"], "https://www.perplexity.ai");
    assert_eq!(seen[0].1["accept"], "text/event-stream");
    assert_eq!(seen[0].2["query_str"], "合成问题");
    assert_eq!(seen[0].2["params"]["model_preference"], "turbo");
    drop(seen);
    dropped(&fixture).await;
}

#[tokio::test]
async fn perplexity_web_http_failures_never_retry_or_echo_private_response() {
    for (status, content_type, code) in [
        (401, "text/event-stream", "perplexity_web_authentication"),
        (403, "text/event-stream", "perplexity_web_authentication"),
        (429, "text/event-stream", "perplexity_web_rate_limit"),
        (500, "text/event-stream", "perplexity_web_http_failed"),
        (200, "text/html", "perplexity_web_challenge"),
        (200, "application/json", "perplexity_web_content_type"),
    ] {
        let fixture = fixture(
            status,
            content_type,
            vec![Bytes::from_static(b"private-upstream-body")],
            false,
            None,
        )
        .await;
        let error = exchange(
            &client(),
            &session(),
            &request(),
            Duration::from_secs(2),
            &fixture.url,
        )
        .await
        .unwrap_err();
        assert_eq!(error.code.as_deref(), Some(code));
        assert!(!error.retryable);
        assert!(matches!(
            error.fallback_hint,
            crate::error::FallbackHint::Abort { .. }
        ));
        assert!(!format!("{error:?}").contains("private-upstream-body"));
        assert_eq!(fixture.seen.lock().unwrap().len(), 1);
    }
}

#[tokio::test]
async fn perplexity_web_redirect_is_not_followed_even_with_a_following_client() {
    let target = fixture(200, "text/event-stream", vec![answer_wire()], false, None).await;
    let redirect = fixture(
        307,
        "text/event-stream",
        vec![],
        false,
        Some(target.url.clone()),
    )
    .await;
    let client = rquest::Client::builder()
        .no_proxy()
        .redirect(rquest::redirect::Policy::limited(8))
        .build()
        .unwrap();
    let error = exchange(
        &client,
        &session(),
        &request(),
        Duration::from_secs(2),
        &redirect.url,
    )
    .await
    .unwrap_err();
    assert_eq!(error.code.as_deref(), Some("perplexity_web_redirect"));
    assert_eq!(redirect.seen.lock().unwrap().len(), 1);
    assert!(target.seen.lock().unwrap().is_empty());
}

#[tokio::test]
async fn perplexity_web_invalid_surface_and_deadline_do_not_send() {
    let fixture = fixture(200, "text/event-stream", vec![answer_wire()], false, None).await;
    let mut invalid = request();
    invalid.extra.insert("temperature".into(), json!(0.2));
    assert!(exchange(
        &client(),
        &session(),
        &invalid,
        Duration::from_secs(1),
        &fixture.url
    )
    .await
    .is_err());
    for deadline in [Duration::ZERO, Duration::from_secs(121)] {
        let error = exchange(&client(), &session(), &request(), deadline, &fixture.url)
            .await
            .unwrap_err();
        assert_eq!(
            error.code.as_deref(),
            Some("perplexity_web_deadline_invalid")
        );
    }
    assert!(fixture.seen.lock().unwrap().is_empty());
    assert_eq!(ASK_URL, "https://www.perplexity.ai/rest/sse/perplexity_ask");
}
