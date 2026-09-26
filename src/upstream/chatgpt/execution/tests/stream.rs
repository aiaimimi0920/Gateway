use super::*;

use axum::{body::Body, http::Response};
use futures::StreamExt;
use tokio::task::JoinHandle;

use crate::error::ErrorKind;

struct StreamServer {
    base_url: String,
    task: JoinHandle<()>,
}

impl StreamServer {
    async fn start(status: u16, content_type: Option<&'static str>, body: &'static str) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind");
        let base_url = format!("http://{}", listener.local_addr().expect("address"));
        let app = Router::new()
            .route(
                "/backend-api/sentinel/chat-requirements",
                post(record_requirements),
            )
            .route("/backend-api/f/conversation/prepare", post(record_prepare))
            .route(
                "/backend-api/f/conversation",
                post(move || async move {
                    let mut response = Response::builder().status(status);
                    if let Some(content_type) = content_type {
                        response = response.header("content-type", content_type);
                    }
                    response.body(Body::from(body)).expect("response")
                }),
            )
            .with_state(RequestState::default());
        let task = tokio::spawn(async move { axum::serve(listener, app).await.expect("serve") });
        Self { base_url, task }
    }

    async fn execute(&self) -> Result<UpstreamStreamingResponse, GatewayError> {
        let http = Client::builder().no_proxy().build().expect("client");
        let mut request = make_request();
        request.stream = true;
        execute_stream(
            &http,
            Duration::from_secs(5),
            &make_payload(&self.base_url),
            &request,
            "auto",
            None,
        )
        .await
    }

    async fn stop(mut self) {
        self.task.abort();
        let error = (&mut self.task).await.expect_err("server cancelled");
        assert!(error.is_cancelled());
    }
}

impl Drop for StreamServer {
    fn drop(&mut self) {
        // Also release the loopback listener when a regression panics before stop.
        self.task.abort();
    }
}

async fn response_error(
    status: u16,
    content_type: Option<&'static str>,
    body: &'static str,
) -> GatewayError {
    let server = StreamServer::start(status, content_type, body).await;
    let response = server.execute().await;
    server.stop().await;
    response
        .err()
        .expect("invalid response must return an error")
}

fn assert_non_sse(error: GatewayError) {
    assert_eq!(error.kind, ErrorKind::ServerError);
    assert_eq!(error.http_status, Some(500));
    assert!(error.retryable);
    assert_eq!(error.provider_name.as_deref(), Some(PROVIDER));
    assert_eq!(error.code.as_deref(), Some("chatgpt_web_non_sse_response"));
    assert_eq!(
        error.message,
        "ChatGPT Web reverse streaming response did not use the text/event-stream content type."
    );
}

#[tokio::test]
async fn successful_json_returns_error_without_panicking() {
    assert_non_sse(response_error(200, Some("application/json"), "{}").await);
}

#[tokio::test]
async fn successful_plain_text_returns_error_without_panicking() {
    assert_non_sse(response_error(200, Some("text/plain"), "ok").await);
}

#[tokio::test]
async fn missing_content_type_returns_error_without_panicking() {
    assert_non_sse(response_error(200, None, "").await);
}

#[tokio::test]
async fn no_content_returns_error_without_panicking() {
    assert_non_sse(response_error(204, None, "").await);
}

#[tokio::test]
async fn non_sse_diagnostic_does_not_publish_upstream_material() {
    assert_non_sse(
        response_error(
            200,
            Some("application/x-private-marker"),
            "synthetic-private-body-marker",
        )
        .await,
    );
}

#[tokio::test]
async fn browser_challenge_classification_is_preserved() {
    let error = response_error(200, Some("text/html"), "<html>Cloudflare challenge</html>").await;
    assert_eq!(error.kind, ErrorKind::ServiceUnavailable);
    assert_eq!(
        error.code.as_deref(),
        Some(surface::CHATGPT_WEB_BROWSER_CHALLENGE_REQUIRED_CODE)
    );
    assert_eq!(error.provider_name.as_deref(), Some(PROVIDER));
    assert_eq!(error.http_status, Some(200));
}

#[tokio::test]
async fn invalid_session_classification_is_preserved() {
    let error = response_error(
        401,
        Some("application/json"),
        r#"{"error":"token expired"}"#,
    )
    .await;
    assert_eq!(error.kind, ErrorKind::Authentication);
    assert_eq!(
        error.code.as_deref(),
        Some(surface::CHATGPT_WEB_SESSION_INVALID_CODE)
    );
    assert_eq!(error.provider_name.as_deref(), Some(PROVIDER));
    assert_eq!(error.http_status, Some(401));
}

#[tokio::test]
async fn upstream_failure_classification_is_preserved() {
    let error = response_error(503, Some("text/plain"), "temporarily unavailable").await;
    assert_eq!(error.kind, ErrorKind::ServerError);
    assert_eq!(error.provider_name.as_deref(), Some(PROVIDER));
    assert_eq!(error.http_status, Some(503));
}

#[tokio::test]
async fn successful_sse_is_consumed_as_canonical_stream() {
    for content_type in ["text/event-stream", "Text/Event-Stream; charset=utf-8"] {
        let server = StreamServer::start(
            200,
            Some(content_type),
            concat!(
                "data: {\"p\":\"/message/content/parts/0\",\"o\":\"append\",\"v\":\"Paris\"}\n\n",
                "data: [DONE]\n\n"
            ),
        )
        .await;
        let response = server.execute().await.expect("SSE response");
        let UpstreamStreamingResponse::Bytes(mut stream) = response else {
            panic!("expected translated byte stream");
        };
        let rendered = tokio::time::timeout(Duration::from_secs(5), async {
            let mut chunks = Vec::new();
            while let Some(chunk) = stream.next().await {
                chunks.push(String::from_utf8(chunk.expect("chunk").to_vec()).expect("UTF-8"));
            }
            chunks
        })
        .await
        .expect("stream completes");
        server.stop().await;
        assert!(rendered
            .iter()
            .any(|chunk| chunk.contains("\"content\":\"Paris\"")));
        assert!(rendered
            .iter()
            .any(|chunk| chunk.contains("\"finish_reason\":\"stop\"")));
        assert_eq!(
            rendered.last().map(String::as_str),
            Some("data: [DONE]\n\n")
        );
        assert_eq!(
            rendered
                .iter()
                .filter(|chunk| chunk.as_str() == "data: [DONE]\n\n")
                .count(),
            1
        );
    }
}

#[tokio::test]
async fn mime_subtype_suffixes_are_rejected() {
    for content_type in [
        "text/event-stream-evil",
        "text/event-streamish",
        "text/event-stream+json",
    ] {
        assert_non_sse(response_error(200, Some(content_type), "data: [DONE]\n\n").await);
    }
}

#[tokio::test]
async fn mime_type_prefixes_are_rejected() {
    for content_type in ["x-text/event-stream", "application/text/event-stream"] {
        assert_non_sse(response_error(200, Some(content_type), "data: [DONE]\n\n").await);
    }
}

#[tokio::test]
async fn mime_parameter_tokens_are_rejected() {
    for content_type in [
        "application/json; note=TEXT/EVENT-STREAM",
        "text/plain; note=\"text/event-stream\"",
        "application/octet-stream; text/event-stream",
    ] {
        assert_non_sse(response_error(200, Some(content_type), "data: [DONE]\n\n").await);
    }
}

#[tokio::test]
async fn mime_comma_lists_are_rejected() {
    for content_type in [
        "text/event-stream, application/json",
        "application/json, text/event-stream",
    ] {
        assert_non_sse(response_error(200, Some(content_type), "data: [DONE]\n\n").await);
    }
}

#[tokio::test]
async fn mime_ows_and_quoted_parameters_preserve_streaming() {
    for content_type in [
        "TEXT/EVENT-STREAM ; charset=utf-8",
        "text/event-stream\t; charset=\"utf-8\"; note=\"a;b\"",
    ] {
        let server = StreamServer::start(
            200,
            Some(content_type),
            concat!(
                "data: {\"p\":\"/message/content/parts/0\",\"o\":\"append\",\"v\":\"Paris\"}\n\n",
                "data: [DONE]\n\n"
            ),
        )
        .await;
        let response = server.execute().await.expect("SSE response");
        let UpstreamStreamingResponse::Bytes(mut stream) = response else {
            panic!("expected translated byte stream");
        };
        let rendered = tokio::time::timeout(Duration::from_secs(5), async {
            let mut chunks = Vec::new();
            while let Some(chunk) = stream.next().await {
                chunks.push(String::from_utf8(chunk.expect("chunk").to_vec()).expect("UTF-8"));
            }
            chunks
        })
        .await
        .expect("stream completes");
        server.stop().await;
        assert!(rendered
            .iter()
            .any(|chunk| chunk.contains("\"content\":\"Paris\"")));
        assert_eq!(
            rendered
                .iter()
                .filter(|chunk| chunk.contains("\"finish_reason\":\"stop\""))
                .count(),
            1
        );
        assert_eq!(
            rendered.last().map(String::as_str),
            Some("data: [DONE]\n\n")
        );
        assert_eq!(
            rendered
                .iter()
                .filter(|chunk| chunk.as_str() == "data: [DONE]\n\n")
                .count(),
            1
        );
    }
}
