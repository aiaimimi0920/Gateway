//! Real loopback exchanges verify protocol selection, phase ordering and fail-closed behavior.
use super::*;
use axum::{
    body::Body,
    http::{Response, Uri},
};
use futures::StreamExt;

const PREPARE: &str = surface::CHATGPT_WEB_REQUIREMENTS_PREPARE_PATH;
const FINALIZE: &str = surface::CHATGPT_WEB_REQUIREMENTS_FINALIZE_PATH;
const LEGACY: &str = surface::CHATGPT_WEB_DEFAULT_REQUIREMENTS_PATH;
const CONVERSATION: &str = surface::CHATGPT_WEB_DEFAULT_CONVERSATION_PATH;

struct Server {
    url: String,
    seen: RequestState,
    task: tokio::task::JoinHandle<()>,
}

impl Server {
    async fn start(failure: Option<(&'static str, u16, &'static str)>) -> Self {
        let seen = RequestState::default();
        let captured = seen.clone();
        let app = Router::new()
            .route("/", axum::routing::get(|| async { ([("content-type", "text/html")], "<html></html>") }))
            .fallback(move |uri: Uri, headers: AxumHeaderMap, body: String| {
            let state = captured.clone();
            async move {
                record(&state, uri.path(), headers, body);
                let (status, body, content_type) = match failure {
                    Some((path, status, body)) if uri.path() == path => (status, body.to_string(), "application/json"),
                    _ if uri.path() == PREPARE => (200, json!({"prepare_token":"fixture-prepare", "turnstile":{"required":false,"token":"fixture-turnstile"}}).to_string(), "application/json"),
                    _ if uri.path() == FINALIZE || uri.path() == LEGACY => (200, json!({"token":"fixture-final", "so_token":"fixture-so"}).to_string(), "application/json"),
                    _ if uri.path() == CONVERSATION => (200, "data: {\"p\":\"/message/content/parts/0\",\"o\":\"append\",\"v\":\"Paris\"}\n\ndata: [DONE]\n\n".to_string(), "text/event-stream"),
                    _ => (404, "{}".to_string(), "application/json"),
                };
                Response::builder().status(status).header("content-type", content_type)
                    .header("location", CONVERSATION).body(Body::from(body)).unwrap()
            }
        });
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let url = format!("http://{}", listener.local_addr().unwrap());
        let task = tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
        Self { url, seen, task }
    }

    fn payload(&self) -> ProviderAccountPayload {
        let mut payload = make_payload(&self.url);
        payload
            .extra_body
            .as_mut()
            .unwrap()
            .remove("chatgptWebRequirementsMode");
        payload
    }

    fn paths(&self) -> Vec<String> {
        self.seen
            .seen
            .lock()
            .unwrap()
            .iter()
            .map(|s| s.path.clone())
            .collect()
    }
}

impl Drop for Server {
    fn drop(&mut self) {
        self.task.abort();
    }
}

async fn run(payload: &ProviderAccountPayload, streaming: bool) -> Result<(), GatewayError> {
    let client = Client::builder().no_proxy().build().unwrap();
    let mut request = make_request();
    request.stream = streaming;
    if streaming {
        let UpstreamStreamingResponse::NativeEvents(mut stream) = execute_stream(
            &client,
            Duration::from_secs(5),
            payload,
            &request,
            "gpt-5.6-sol",
            None,
        )
        .await?
        else {
            panic!("expected bytes")
        };
        let mut text = String::new();
        while let Some(chunk) = stream.next().await {
            text.push_str(
                std::str::from_utf8(&chunk.expect("valid fixture stream chunk")).unwrap(),
            );
        }
        assert!(text.contains("Paris"));
    } else {
        assert_eq!(
            execute(
                &client,
                Duration::from_secs(5),
                payload,
                &request,
                "gpt-5.6-sol",
                None
            )
            .await?
            .text,
            "Paris"
        );
    }
    Ok(())
}

#[tokio::test]
async fn default_plain_handshake_prepares_then_finalizes_before_generating() {
    for streaming in [false, true] {
        let server = Server::start(None).await;
        run(&server.payload(), streaming).await.unwrap();
        assert_eq!(server.paths(), [PREPARE, FINALIZE, CONVERSATION]);
        let seen = server.seen.seen.lock().unwrap();
        let prepare: Value = serde_json::from_str(&seen[0].body).unwrap();
        assert!(prepare["p"].as_str().is_some_and(|v| !v.is_empty()));
        let finalize: Value = serde_json::from_str(&seen[1].body).unwrap();
        assert_eq!(
            finalize,
            json!({"prepare_token":"fixture-prepare", "proof_token":"", "turnstile_token":"fixture-turnstile"})
        );
        assert_eq!(
            seen[2].headers["openai-sentinel-chat-requirements-token"],
            "fixture-final"
        );
        assert_eq!(seen[2].headers["openai-sentinel-so-token"], "fixture-so");
        assert_eq!(
            serde_json::from_str::<Value>(&seen[2].body).unwrap()["model"],
            "gpt-5.6-sol"
        );
        for item in seen.iter() {
            assert!(!item.headers.contains_key("x-oai-is"));
            assert!(!item.headers.contains_key("x-conduit-token"));
            assert_eq!(item.headers["authorization"], "Bearer session-token");
        }
    }
}

#[tokio::test]
async fn rejected_phase_never_downgrades_retries_or_enters_later_phases() {
    for phase in [PREPARE, FINALIZE] {
        for status in [302, 401, 403, 503] {
            for streaming in [false, true] {
                let server = Server::start(Some((
                    phase,
                    status,
                    r#"{"error":"private-rejection-marker"}"#,
                )))
                .await;
                let error = run(&server.payload(), streaming).await.unwrap_err();
                assert!(error.message.contains(if phase == PREPARE {
                    "stage=requirements_prepare"
                } else {
                    "stage=requirements_finalize"
                }));
                assert!(error.message.contains(&format!("upstream_status={status}")));
                assert!(!error.message.contains("private-rejection-marker"));
                assert_eq!(
                    server.paths(),
                    if phase == PREPARE {
                        vec![PREPARE]
                    } else {
                        vec![PREPARE, FINALIZE]
                    }
                );
            }
        }
    }
}

#[tokio::test]
async fn malformed_or_missing_phase_tokens_stop_without_body_echo() {
    for (phase, body, code) in [
        (PREPARE, "{}", "chatgpt_web_requirements_missing_token"),
        (FINALIZE, "{}", "chatgpt_web_requirements_missing_token"),
        (
            PREPARE,
            "private-invalid-json-marker",
            "chatgpt_web_requirements_decode_failed",
        ),
        (FINALIZE, "[]", "chatgpt_web_requirements_invalid_shape"),
        (
            FINALIZE,
            r#"{"token":123}"#,
            "chatgpt_web_requirements_invalid_token",
        ),
        (
            PREPARE,
            r#"{"arkose":{"required":true}}"#,
            surface::CHATGPT_WEB_BROWSER_CHALLENGE_REQUIRED_CODE,
        ),
    ] {
        let server = Server::start(Some((phase, 200, body))).await;
        let error = run(&server.payload(), false).await.unwrap_err();
        assert_eq!(error.code.as_deref(), Some(code));
        assert!(!error.message.contains("private-invalid-json-marker"));
        assert_eq!(server.paths().len(), if phase == PREPARE { 1 } else { 2 });
    }
}

#[tokio::test]
async fn explicit_legacy_override_retains_single_stage_plain_compatibility() {
    let server = Server::start(None).await;
    let mut payload = server.payload();
    payload
        .extra_body
        .as_mut()
        .unwrap()
        .insert("chatgptWebRequirementsMode".into(), json!("legacy"));
    run(&payload, false).await.unwrap();
    assert_eq!(server.paths(), [LEGACY, CONVERSATION]);
}

#[tokio::test]
async fn invalid_mode_is_rejected_before_network_without_echoing_value() {
    let server = Server::start(None).await;
    let mut payload = server.payload();
    payload.extra_body.as_mut().unwrap().insert(
        "chatgptWebRequirementsMode".into(),
        json!("private-mode-marker"),
    );
    let error = run(&payload, false).await.unwrap_err();
    assert_eq!(
        error.code.as_deref(),
        Some("chatgpt_web_requirements_mode_invalid")
    );
    assert!(!error.message.contains("private-mode-marker"));
    assert!(server.paths().is_empty());
}
