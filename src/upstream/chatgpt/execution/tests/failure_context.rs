//! Loopback failures must identify the rejected stage without publishing response bodies.
use super::*;
use axum::{body::Body, http::Response};

struct FailureServer {
    url: String,
    seen: Arc<Mutex<Vec<String>>>,
    task: tokio::task::JoinHandle<()>,
}

impl Drop for FailureServer {
    fn drop(&mut self) {
        self.task.abort();
    }
}

impl FailureServer {
    async fn start(rejected_path: &'static str) -> Self {
        let seen = Arc::new(Mutex::new(Vec::new()));
        let captured = seen.clone();
        let app = Router::new().fallback(move |uri: axum::http::Uri| {
            let seen = captured.clone();
            async move {
                seen.lock().unwrap().push(uri.path().to_string());
                let (status, content_type, body) = if uri.path() == rejected_path {
                    (
                        403,
                        "text/html",
                        "<html>Just a moment: verify you are human. private-body-marker</html>",
                    )
                } else if uri.path() == "/" {
                    (200, "text/html", "<html></html>")
                } else if uri.path() == surface::CHATGPT_WEB_DEFAULT_REQUIREMENTS_PATH {
                    (
                        200,
                        "application/json",
                        r#"{"token":"fixture-requirements"}"#,
                    )
                } else {
                    (200, "application/json", "{}")
                };
                Response::builder()
                    .status(status)
                    .header("content-type", content_type)
                    .body(Body::from(body))
                    .unwrap()
            }
        });
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let url = format!("http://{}", listener.local_addr().unwrap());
        let task = tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
        Self { url, seen, task }
    }
}

#[tokio::test]
async fn http_failures_report_stage_status_and_stop_before_later_requests() {
    let client = Client::builder().no_proxy().build().unwrap();
    for (stage, path, request_count) in [
        ("bootstrap", "/", 1),
        (
            "requirements",
            surface::CHATGPT_WEB_DEFAULT_REQUIREMENTS_PATH,
            2,
        ),
        (
            "prepare",
            surface::CHATGPT_WEB_DEFAULT_F_CONVERSATION_PREPARE_PATH,
            3,
        ),
        (
            "conversation",
            surface::CHATGPT_WEB_DEFAULT_F_CONVERSATION_PATH,
            4,
        ),
    ] {
        for streaming in [false, true] {
            let server = FailureServer::start(path).await;
            let mut payload = make_payload(&server.url);
            payload.extra_body.as_mut().unwrap().insert(
                "conversationPath".into(),
                json!("/backend-api/f/conversation"),
            );
            if stage == "bootstrap" {
                payload.extra_body = None;
            }
            let mut request = make_request();
            request.stream = streaming;
            let error = if streaming {
                execute_stream(
                    &client,
                    Duration::from_secs(5),
                    &payload,
                    &request,
                    "fixture-model",
                    None,
                )
                .await
                .err()
                .expect("rejected stage")
            } else {
                execute(
                    &client,
                    Duration::from_secs(5),
                    &payload,
                    &request,
                    "fixture-model",
                    None,
                )
                .await
                .err()
                .expect("rejected stage")
            };
            assert_eq!(error.http_status, Some(403));
            assert_eq!(
                error.code.as_deref(),
                Some(surface::CHATGPT_WEB_BROWSER_CHALLENGE_REQUIRED_CODE)
            );
            assert!(
                error.message.contains(&format!("stage={stage}")),
                "{}",
                error.message
            );
            assert!(
                error.message.contains("upstream_status=403"),
                "{}",
                error.message
            );
            assert!(!error.message.contains("private-body-marker"));
            let seen = server.seen.lock().unwrap();
            assert_eq!(
                seen.len(),
                request_count,
                "no later stage may run after rejection"
            );
            assert_eq!(seen.last().map(String::as_str), Some(path));
        }
    }
}
