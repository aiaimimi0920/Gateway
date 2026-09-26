use super::*;
use crate::protocol::canonical::{CanonicalMessage, ContentPart, MessageRole, ProtocolFamily};
use serde_json::json;
use std::collections::HashMap;

fn make_payload(adapter: &str, base_url: &str) -> ProviderAccountPayload {
    ProviderAccountPayload {
        adapter: adapter.to_string(),
        base_url: base_url.to_string(),
        api_key: "sk-test".to_string(),
        credential_id: None,
        expires_at: None,
        runtime_state_object_key: None,
        account_name: None,
        execution_mode: None,
        endpoint_execution_modes: None,
        default_model: None,
        headers: HashMap::new(),
        auth_mode: None,
        anthropic_version: None,
        beta_headers: None,
        auth_header_name: None,
        auth_token: None,
        responses_path: None,
        chat_completions_path: None,
        completions_path: None,
        embeddings_path: None,
        audio_transcriptions_path: None,
        audio_speech_path: None,
        messages_path: None,
        search_path: None,
        fetch_path: None,
        research_path: None,
        balance_path: None,
        search_query_field: None,
        fetch_urls_field: None,
        extra_body: None,
        session_auth: None,
        keepalive: None,
    }
}

fn make_request(protocol: ProtocolFamily, endpoint: EndpointKind) -> CanonicalRelayRequest {
    CanonicalRelayRequest {
        protocol_family: protocol,
        endpoint_kind: endpoint,
        requested_model: Some("gpt-4o".to_string()),
        stream: false,
        messages: vec![CanonicalMessage {
            role: MessageRole::User,
            content: vec![ContentPart::Text {
                text: "Hello".to_string(),
            }],
            name: None,
            tool_call_id: None,
            tool_calls: vec![],
        }],
        tools: vec![],
        tool_choice: None,
        reasoning: None,
        metadata: None,
        raw_body: json!({}),
        previous_response_id: None,
        explicit_session_key: None,
        extra: HashMap::new(),
    }
}

fn make_gemini_canvas_runtime_api_context(
    payload: ProviderAccountPayload,
    api_key_candidates: &[&str],
    page_referer: &str,
) -> GeminiCanvasRuntimeApiContext {
    GeminiCanvasRuntimeApiContext {
        payload,
        api_key_candidates: api_key_candidates
            .iter()
            .map(|candidate| (*candidate).to_string())
            .collect(),
        session: gemini_canvas::GeminiCanvasPureHttpSession {
            cookie_header: "SAPISID=session-cookie".to_string(),
            sapisid: "session-cookie".to_string(),
            auth_user: "0".to_string(),
        },
        page_origin: "https://gemini.google.com".to_string(),
        page_referer: page_referer.to_string(),
    }
}

fn unused_loopback_base_url() -> String {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("bind unused loopback port");
    let addr = listener.local_addr().expect("read loopback listener addr");
    drop(listener);
    format!("http://{addr}")
}

fn single_response_executor_base_url(
    status: u16,
    status_text: &'static str,
    body: &'static str,
) -> (String, std::thread::JoinHandle<()>) {
    let listener =
        std::net::TcpListener::bind("127.0.0.1:0").expect("bind executor response server");
    let addr = listener
        .local_addr()
        .expect("read executor response server addr");
    let handle = std::thread::spawn(move || {
        let (mut stream, _) = listener.accept().expect("accept executor request");
        let mut buffer = [0_u8; 4096];
        let _ = std::io::Read::read(&mut stream, &mut buffer);
        let response = format!(
                "HTTP/1.1 {status} {status_text}\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n{body}",
                body.len()
            );
        std::io::Write::write_all(&mut stream, response.as_bytes())
            .expect("write executor response");
    });
    (format!("http://{addr}"), handle)
}

#[path = "tests/fallback.rs"]
mod fallback;

#[path = "tests/continuation.rs"]
mod continuation;

#[path = "tests/browser_executor.rs"]
mod browser_executor;

#[path = "tests/provider_input.rs"]
mod provider_input;
