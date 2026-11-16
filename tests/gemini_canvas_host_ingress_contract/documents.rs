//! Public HTTP to selected Canvas HTTP account; synthetic replies are not model evidence.
use super::http_fixture::{route_document, Host};
use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use base64::Engine;
use http_body_util::BodyExt;
use neuro_gateway::{config::GatewayStorageMode, http::router::build_router};
use serde_json::json;
use tower::ServiceExt;

#[test]
fn document_inputs_public_http_preserves_selected_identity_and_rejects_before_generation() {
    // Match the pipeline stack budget and serialize this environment-owning contract executable.
    std::thread::Builder::new()
        .stack_size(16 * 1024 * 1024)
        .spawn(|| {
            tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .unwrap()
                .block_on(async {
                    tokio::time::timeout(std::time::Duration::from_secs(20), exercise())
                        .await
                        .unwrap();
                });
        })
        .unwrap()
        .join()
        .unwrap();
}

async fn exercise() {
    let host = Host::start().await;
    let seen = host.capture.clone();
    let root =
        std::env::temp_dir().join(format!("gateway-canvas-documents-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&root).unwrap();
    let mut config = super::support::test_config();
    config.storage_mode = GatewayStorageMode::Local;
    config.console.state_dir = root.join("state");
    config.console.routes_file = root.join("routes.yaml");
    std::fs::write(&config.console.routes_file, route_document(&host.base)).unwrap();
    let state = Box::pin(neuro_gateway::runtime::build_app_state(config))
        .await
        .unwrap();
    let pdf = b"%PDF-1.4\n%%EOF\n";
    let data = base64::engine::general_purpose::STANDARD.encode(pdf);
    let cases = [
        (
            json!({"type":"file","file":{"filename":"../../private.pdf","file_data":format!("data:application/pdf;base64,{data}")}}),
            true,
            StatusCode::OK,
        ),
        (
            json!({"type":"input_file","filename":"C:/private.pdf","file_data":format!("data:application/pdf;base64,{data}")}),
            true,
            StatusCode::OK,
        ),
        (
            json!({"type":"input_file","file_url":"https://private.invalid/document.pdf"}),
            true,
            StatusCode::BAD_REQUEST,
        ),
        (
            json!({"type":"file","file":{"file_id":"file-not-resolved"}}),
            true,
            StatusCode::BAD_REQUEST,
        ),
        (
            json!({"type":"input_file","file_data":"data:application/pdf;base64,AAEC"}),
            true,
            StatusCode::BAD_REQUEST,
        ),
        (
            json!({"type":"input_file","file_data":"data:text/plain;base64,/w=="}),
            true,
            StatusCode::BAD_REQUEST,
        ),
        (
            json!({"type":"input_file","file_data":format!("data:application/pdf;base64,{data}")}),
            false,
            StatusCode::UNAUTHORIZED,
        ),
    ];
    for endpoint in ["/v1/chat/completions", "/v1/responses"] {
        for (file, authorized, expected) in cases.clone() {
            let before = seen.lock().unwrap().seen.len();
            let raw = json!({"model":"gemini-3-flash-preview","messages":[{"role":"user","content":[
                {"type":"text","text":"Summarize the document."},file,
                {"type":"input_file","filename":"C:/not-read.txt","file_data":"data:text/plain;base64,aGVsbG8="}
            ]}]});
            let raw = if endpoint == "/v1/responses" {
                json!({"model":"gemini-3-flash-preview","input":[{"role":"user","content":[
                    {"type":"input_text","text":"Summarize the document."},file,
                    {"type":"input_file","filename":"C:/not-read.txt","file_data":"data:text/plain;base64,aGVsbG8="}
                ]}]})
            } else {
                raw
            };
            let mut request = Request::post(endpoint).header("content-type", "application/json");
            if authorized {
                request = request.header("authorization", "Bearer public-api-contract-secret");
            }
            let reply = Box::pin(
                build_router(state.clone())
                    .oneshot(request.body(Body::from(raw.to_string())).unwrap()),
            )
            .await
            .unwrap();
            let status = reply.status();
            let body = reply.into_body().collect().await.unwrap().to_bytes();
            assert_eq!(
                status,
                expected,
                "{endpoint}: {}",
                String::from_utf8_lossy(&body)
            );
            let capture = seen.lock().unwrap();
            assert_eq!(
                capture.seen.len(),
                before + usize::from(status == StatusCode::OK)
            );
            if status == StatusCode::OK {
                let invocation = capture.seen.last().unwrap();
                assert_eq!(
                    invocation["cookie"],
                    "SAPISID=selected-signing-cookie; SID=selected-login-cookie"
                );
                let host_body = &invocation["body"];
                assert_eq!(host_body["contents"][0]["role"], "user");
                assert_eq!(
                    host_body["contents"][0]["parts"][1],
                    json!({"inlineData":{"mimeType":"application/pdf","data":data}})
                );
                assert_eq!(
                    host_body["contents"][0]["parts"][2],
                    json!({"inlineData":{"mimeType":"text/plain","data":"aGVsbG8="}})
                );
                assert!(!host_body.to_string().contains("private"));
                assert!(host_body.get("apiKey").is_none());
                assert!(String::from_utf8_lossy(&body).contains("合成回执"));
            }
        }
    }
    state.local_runtime.as_ref().unwrap().close().await;
}
