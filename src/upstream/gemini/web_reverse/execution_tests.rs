//! Actual loopback HTTP validates wire bytes and ordering; no model or account is used.
use super::*;
use axum::{
    body::Bytes,
    extract::{Form, State},
    http::HeaderMap,
    routing::{get, post},
    Router,
};
use serde_json::{json, Value};
use std::sync::{Arc, Mutex};

#[derive(Default)]
struct Capture {
    steps: Vec<&'static str>,
    uploads: Vec<(HeaderMap, Vec<u8>)>,
    generated: Vec<Value>,
    upload_reply: String,
    upload_status: u16,
}

async fn bootstrap(State(state): State<Arc<Mutex<Capture>>>) -> &'static str {
    state.lock().unwrap().steps.push("bootstrap");
    r#"<html><script>window.WIZ_global_data = {"SNlM0e":"synthetic-at","cfb2h":"build-fixture","FdrFJe":"sid-fixture","qKIAYe":"synthetic-push"};</script></html>"#
}

async fn upload(
    State(state): State<Arc<Mutex<Capture>>>,
    headers: HeaderMap,
    body: Bytes,
) -> (axum::http::StatusCode, String) {
    let mut capture = state.lock().unwrap();
    capture.steps.push("upload");
    capture.uploads.push((headers, body.to_vec()));
    (
        axum::http::StatusCode::from_u16(if capture.upload_status == 0 {
            200
        } else {
            capture.upload_status
        })
        .unwrap(),
        if capture.upload_reply.is_empty() {
            "/contrib_service/ttl_1d/synthetic-reference".into()
        } else {
            capture.upload_reply.clone()
        },
    )
}

async fn generate(
    State(state): State<Arc<Mutex<Capture>>>,
    Form(form): Form<HashMap<String, String>>,
) -> String {
    let outer: Value = serde_json::from_str(&form["f.req"]).unwrap();
    let inner: Value = serde_json::from_str(outer[1].as_str().unwrap()).unwrap();
    let mut capture = state.lock().unwrap();
    capture.steps.push("generate");
    capture.generated.push(inner);
    json!({"candidates":[{"finishReason":"STOP","content":{"parts":[{"text":"合成回执"}]}}]})
        .to_string()
}

async fn fixture() -> (String, Arc<Mutex<Capture>>, tokio::task::JoinHandle<()>) {
    let state = Arc::new(Mutex::new(Capture::default()));
    let router = Router::new()
        .route("/app", get(bootstrap))
        .route("/upload", post(upload))
        .route("/generate", post(generate))
        .with_state(state.clone());
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());
    let task = tokio::spawn(async move {
        axum::serve(listener, router).await.unwrap();
    });
    (base, state, task)
}

fn payload(base: &str) -> ProviderAccountPayload {
    serde_json::from_value(json!({"adapter":"gemini_web_reverse_modular_compatible","base_url":base,"api_key":"synthetic-selected-cookie",
        "chat_completions_path":"/generate","headers":{"cookie":"pref=synthetic-only","authorization":"do-not-copy-to-upload"},
        "extra_body":{"geminiWebBootstrap":{"push_id":"synthetic-push"}}})).unwrap()
}

fn request() -> CanonicalRelayRequest {
    crate::protocol::openai::normalize_chat_completions(json!({"model":"gemini-3-flash-preview","messages":[{"role":"user","content":[
        {"type":"text","text":"Describe attached audio"},{"type":"input_audio","input_audio":{"format":"mp3","data":"AAEC"}}
    ]}]})).unwrap()
}

#[tokio::test]
async fn loopback_preserves_bytes_and_binds_references_without_cross_origin_auth() {
    let (base, capture, server) = fixture().await;
    let http = crate::http_client::builder().no_proxy().build().unwrap();
    let result = exchange::exchange(
        &http,
        Duration::from_secs(5),
        &payload(&base),
        &request(),
        "gemini-3-flash-preview",
        None,
        &format!("{base}/upload"),
    )
    .await;
    server.abort();
    let _ = server.await;
    let (body, _) = result.unwrap();
    assert_eq!(
        surface::accumulate_complete_media_response(&body, "fixture")
            .unwrap()
            .text,
        "合成回执"
    );
    let data = capture.lock().unwrap();
    assert_eq!(data.steps, ["bootstrap", "upload", "generate"]);
    let (headers, bytes) = &data.uploads[0];
    assert!(!headers.contains_key("cookie") && !headers.contains_key("authorization"));
    assert_eq!(headers["x-tenant-id"], "bard-storage");
    assert_eq!(headers["push-id"], "synthetic-push");
    assert!(bytes.windows(3).any(|part| part == [0, 1, 2]));
    assert!(String::from_utf8_lossy(bytes)
        .contains("filename=\"input-1.mp3\"\r\nContent-Type: audio/mpeg"));
    assert_eq!(
        data.generated[0][0][3],
        json!([[
            ["/contrib_service/ttl_1d/synthetic-reference"],
            "input-1.mp3"
        ]])
    );
    assert!(!data.generated[0].to_string().contains("AAEC"));
}

#[tokio::test]
async fn upload_failure_or_malicious_reference_stops_before_generation() {
    for (status, reply) in [
        (503, "failure".to_string()),
        (200, "https://evil.invalid/x".into()),
        (200, "/contrib_service/../secret".into()),
        (200, "x".repeat(4097)),
        (302, "redirect".into()),
    ] {
        let (base, capture, server) = fixture().await;
        {
            let mut data = capture.lock().unwrap();
            data.upload_status = status;
            data.upload_reply = reply;
        }
        let http = crate::http_client::builder().no_proxy().build().unwrap();
        let result = exchange::exchange(
            &http,
            Duration::from_secs(5),
            &payload(&base),
            &request(),
            "fixture",
            None,
            &format!("{base}/upload"),
        )
        .await;
        server.abort();
        let _ = server.await;
        let error = protect(&request(), result.unwrap_err());
        assert!(!error.retryable);
        assert!(matches!(
            error.fallback_hint,
            crate::error::FallbackHint::Abort { .. }
        ));
        assert_eq!(capture.lock().unwrap().steps, ["bootstrap", "upload"]);
    }
}

#[tokio::test]
async fn invalid_media_is_rejected_before_even_bootstrap() {
    let (base, capture, server) = fixture().await;
    let http = crate::http_client::builder().no_proxy().build().unwrap();
    let mut req = request();
    req.messages[0]
        .content
        .push(crate::protocol::canonical::ContentPart::ImageUrl {
            image_url: "https://example.invalid/private".into(),
            detail: None,
        });
    assert!(exchange::exchange(
        &http,
        Duration::from_secs(5),
        &payload(&base),
        &req,
        "fixture",
        None,
        &format!("{base}/upload")
    )
    .await
    .is_err());
    server.abort();
    let _ = server.await;
    assert!(capture.lock().unwrap().steps.is_empty());
}

#[test]
fn media_errors_keep_challenge_and_auth_identity_without_private_body() {
    for (body, code) in [
        (
            "<html>challenge PRIVATE_MEDIA_CONTENT</html>",
            surface::GEMINI_WEB_BROWSER_CHALLENGE_REQUIRED_CODE,
        ),
        (
            "<html>sign in PRIVATE_MEDIA_CONTENT</html>",
            surface::GEMINI_WEB_SESSION_INVALID_CODE,
        ),
    ] {
        let error = protect(
            &request(),
            exchange::response_error(&request(), 200, Some("text/html"), body),
        );
        assert_eq!(error.code.as_deref(), Some(code));
        assert!(!error.message.contains("PRIVATE_MEDIA_CONTENT"));
        assert!(!error.retryable);
    }
}

#[tokio::test]
async fn document_inputs_loopback_upload_exact_bytes_before_generate_without_auth_or_base64_leak() {
    use base64::Engine;
    let (base, capture, server) = fixture().await;
    let pdf = b"%PDF-1.4\n%%EOF\n";
    let text = "synthetic document text";
    let encoded = base64::engine::general_purpose::STANDARD.encode(pdf);
    let req = crate::protocol::openai::normalize_chat_completions(json!({
        "model":"gemini-3-flash-preview","messages":[{"role":"user","content":[
            {"type":"text","text":"Summarize the attached documents."},
            {"type":"file","file":{"filename":"../../private.pdf","file_data":format!("data:application/pdf;base64,{encoded}")}},
            {"type":"input_file","filename":"C:/private.txt","file_data":format!("data:text/plain;base64,{}",base64::engine::general_purpose::STANDARD.encode(text))}
        ]}]
    })).unwrap();
    let http = crate::http_client::builder().no_proxy().build().unwrap();
    let result = exchange::exchange(
        &http,
        Duration::from_secs(5),
        &payload(&base),
        &req,
        "gemini-3-flash-preview",
        None,
        &format!("{base}/upload"),
    )
    .await;
    server.abort();
    let _ = server.await;
    result.unwrap();
    let data = capture.lock().unwrap();
    assert_eq!(data.steps, ["bootstrap", "upload", "upload", "generate"]);
    for ((headers, bytes), (expected, mime, name)) in data.uploads.iter().zip([
        (pdf.as_slice(), "application/pdf", "input-1.pdf"),
        (text.as_bytes(), "text/plain", "input-2.txt"),
    ]) {
        assert!(!headers.contains_key("cookie") && !headers.contains_key("authorization"));
        assert!(bytes.windows(expected.len()).any(|part| part == expected));
        let multipart = String::from_utf8_lossy(bytes);
        assert!(multipart.contains(&format!("filename=\"{name}\"\r\nContent-Type: {mime}")));
        assert!(!multipart.contains("private"));
    }
    assert_eq!(
        data.generated[0][0][3],
        json!([
            [
                ["/contrib_service/ttl_1d/synthetic-reference"],
                "input-1.pdf"
            ],
            [
                ["/contrib_service/ttl_1d/synthetic-reference"],
                "input-2.txt"
            ]
        ])
    );
    let generated = data.generated[0].to_string();
    assert!(!generated.contains(&encoded) && !generated.contains("private"));
}

#[tokio::test]
async fn document_inputs_invalid_or_oversized_never_contact_bootstrap_or_upload() {
    let (base, capture, server) = fixture().await;
    let http = crate::http_client::builder().no_proxy().build().unwrap();
    let file = json!({"type":"input_file","file_data":"data:text/plain;base64,aGVsbG8="});
    for parts in [
        vec![json!({"type":"input_file","file_url":"https://private.invalid/a.pdf"})],
        vec![json!({"type":"file","file":{"file_id":"file-not-resolved"}})],
        vec![json!({"type":"input_file","file_data":"data:application/pdf;base64,AAEC"})],
        vec![json!({"type":"input_file","file_data":"data:text/plain;base64,/w=="})],
        vec![file; 17],
        vec![
            json!({"type":"input_file","file_data":format!("data:text/plain;base64,{}", "A".repeat(16 * 1024 * 1024 + 4))}),
        ],
    ] {
        let mut req = request();
        req.messages[0].content = vec![crate::protocol::canonical::ContentPart::Text {
            text: "Summarize.".into(),
        }];
        req.messages[0].content.extend(
            parts
                .into_iter()
                .map(|value| crate::protocol::canonical::ContentPart::Raw { value }),
        );
        assert!(exchange::exchange(
            &http,
            Duration::from_secs(5),
            &payload(&base),
            &req,
            "gemini-3-flash-preview",
            None,
            &format!("{base}/upload")
        )
        .await
        .is_err());
    }
    server.abort();
    let _ = server.await;
    assert!(capture.lock().unwrap().steps.is_empty());
}
