use super::*;

mod body_limits;
mod body_targets;
mod body_wire;
mod stream;

use std::sync::{Arc, Mutex};

use axum::{extract::State, http::HeaderMap as AxumHeaderMap, routing::post, Json, Router};
use serde_json::{json, Value};
use tokio::net::TcpListener;

use crate::credential_runtime::SessionAuthConfig;
use crate::protocol::canonical::{
    CanonicalMessage, ContentPart, EndpointKind, MessageRole, ProtocolFamily,
};

#[derive(Clone, Default)]
struct RequestState {
    seen: Arc<Mutex<Vec<SeenRequest>>>,
}

#[derive(Clone, Debug)]
struct SeenRequest {
    path: String,
    headers: HashMap<String, String>,
    body: String,
}

async fn record_requirements(
    State(state): State<RequestState>,
    headers: AxumHeaderMap,
    body: String,
) -> Json<Value> {
    record(
        &state,
        "/backend-api/sentinel/chat-requirements",
        headers,
        body,
    );
    Json(json!({
        "token": "requirements-token",
        "proofofwork": { "required": false },
        "turnstile": { "required": false, "token": "turnstile-token" }
    }))
}

async fn record_prepare(
    State(state): State<RequestState>,
    headers: AxumHeaderMap,
    body: String,
) -> Json<Value> {
    record(&state, "/backend-api/f/conversation/prepare", headers, body);
    Json(json!({ "ok": true }))
}

async fn record_conversation(
    State(state): State<RequestState>,
    headers: AxumHeaderMap,
    body: String,
) -> ([(&'static str, &'static str); 1], &'static str) {
    record(&state, "/backend-api/f/conversation", headers, body);
    (
        [("content-type", "text/event-stream")],
        concat!(
            "data: {\"p\":\"/message/content/parts/0\",\"o\":\"append\",\"v\":\"Paris\"}\n\n",
            "data: [DONE]\n\n"
        ),
    )
}

async fn record_dynamic_conversation(
    State(state): State<RequestState>,
    headers: AxumHeaderMap,
    body: String,
) -> ([(&'static str, &'static str); 1], &'static str) {
    record(&state, "/backend-api/conversation", headers, body);
    (
        [("content-type", "text/event-stream")],
        concat!(
            "data: {\"p\":\"/message/content/parts/0\",\"o\":\"append\",\"v\":\"Paris\"}\n\n",
            "data: [DONE]\n\n"
        ),
    )
}

fn record(state: &RequestState, path: &str, headers: AxumHeaderMap, body: String) {
    let headers = headers
        .iter()
        .filter_map(|(name, value)| {
            value
                .to_str()
                .ok()
                .map(|value| (name.as_str().to_ascii_lowercase(), value.to_string()))
        })
        .collect::<HashMap<_, _>>();
    state.seen.lock().unwrap().push(SeenRequest {
        path: path.to_string(),
        headers,
        body,
    });
}

async fn spawn_chatgpt_f_conversation_server(state: RequestState) -> String {
    let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind");
    let address = listener.local_addr().expect("addr");
    let app = Router::new()
        .route(
            "/backend-api/sentinel/chat-requirements",
            post(record_requirements),
        )
        .route("/backend-api/f/conversation/prepare", post(record_prepare))
        .route("/backend-api/f/conversation", post(record_conversation))
        .route(
            "/backend-api/conversation",
            post(record_dynamic_conversation),
        )
        .with_state(state);
    tokio::spawn(async move {
        axum::serve(listener, app).await.expect("serve");
    });
    format!("http://{}", address)
}

fn make_payload(base_url: &str) -> ProviderAccountPayload {
    let mut headers = HashMap::new();
    headers.insert("X-OAI-IS".to_string(), "is-material".to_string());
    headers.insert(
        "X-Conduit-Token".to_string(),
        "conduit-material".to_string(),
    );
    ProviderAccountPayload {
        discovered_protocols: Vec::new(),
        adapter: "chatgpt_web_reverse_compatible".to_string(),
        base_url: base_url.to_string(),
        api_key: "session-token".to_string(),
        credential_id: None,
        expires_at: None,
        runtime_state_object_key: None,
        account_name: None,
        execution_mode: None,
        endpoint_execution_modes: None,
        default_model: None,
        headers,
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
        extra_body: Some(HashMap::from([
            (
                "chatgptPowSources".to_string(),
                json!(["https://chatgpt.com/backend-api/sentinel/sdk.js"]),
            ),
            ("chatgptPowDataBuild".to_string(), json!("prod-test-build")),
        ])),
        session_auth: Some(SessionAuthConfig {
            transport: "bearer".to_string(),
            primary_cookie_name: None,
            secondary_cookie_name: None,
            header_name: Some("authorization".to_string()),
            expires_at: None,
        }),
        keepalive: None,
    }
}

fn make_request() -> CanonicalRelayRequest {
    CanonicalRelayRequest {
        protocol_family: ProtocolFamily::OpenAi,
        endpoint_kind: EndpointKind::ChatCompletions,
        requested_model: Some("auto".to_string()),
        stream: false,
        messages: vec![CanonicalMessage {
            role: MessageRole::User,
            content: vec![ContentPart::Text {
                text: "What is the capital of France?".to_string(),
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

#[tokio::test]
async fn execute_uses_browserless_f_conversation_prepare_contract() {
    let state = RequestState::default();
    let base_url = spawn_chatgpt_f_conversation_server(state.clone()).await;
    let payload = make_payload(&base_url);
    let response = execute(
        &Client::new(),
        Duration::from_secs(5),
        &payload,
        &make_request(),
        "auto",
        None,
    )
    .await
    .expect("f conversation response");

    assert_eq!(response.text, "Paris");

    let seen = state.seen.lock().unwrap().clone();
    assert_eq!(
        seen.iter()
            .map(|item| item.path.as_str())
            .collect::<Vec<_>>(),
        vec![
            "/backend-api/sentinel/chat-requirements",
            "/backend-api/f/conversation/prepare",
            "/backend-api/f/conversation",
        ]
    );

    let prepare = &seen[1];
    let conversation = &seen[2];
    assert!(prepare.body.is_empty());
    assert_eq!(
        prepare
            .headers
            .get("x-openai-target-path")
            .map(String::as_str),
        Some("/backend-api/f/conversation/prepare")
    );
    assert_eq!(
        conversation
            .headers
            .get("x-openai-target-path")
            .map(String::as_str),
        Some("/backend-api/f/conversation")
    );

    let trace_id = prepare
        .headers
        .get("x-oai-turn-trace-id")
        .expect("prepare trace id");
    uuid::Uuid::parse_str(trace_id).expect("trace id uuid");
    assert_eq!(
        conversation
            .headers
            .get("x-oai-turn-trace-id")
            .map(String::as_str),
        Some(trace_id.as_str())
    );
    let requirements = &seen[0];
    assert!(!requirements.headers.contains_key("x-oai-is"));
    assert!(!requirements.headers.contains_key("x-conduit-token"));
    assert_eq!(
        conversation
            .headers
            .get("openai-sentinel-chat-requirements-token")
            .map(String::as_str),
        Some("requirements-token")
    );
    assert_eq!(
        conversation
            .headers
            .get("openai-sentinel-turnstile-token")
            .map(String::as_str),
        Some("turnstile-token")
    );
    assert_eq!(
        conversation.headers.get("x-oai-is").map(String::as_str),
        Some("is-material")
    );
    assert_eq!(
        conversation
            .headers
            .get("x-conduit-token")
            .map(String::as_str),
        Some("conduit-material")
    );

    let body: Value = serde_json::from_str(&conversation.body).expect("conversation body json");
    assert_eq!(body["client_prepare_state"], "success");
    assert_eq!(body["parent_message_id"], "client-created-root");
}

#[tokio::test]
async fn execute_uses_cached_f_conversation_sentinel_material_without_preflight() {
    let state = RequestState::default();
    let base_url = spawn_chatgpt_f_conversation_server(state.clone()).await;
    let mut payload = make_payload(&base_url);
    let extra_body = payload.extra_body.as_mut().expect("extra body");
    extra_body.insert(
        "chatgptWebSentinelChatRequirementsToken".to_string(),
        json!("stored-requirements-token"),
    );
    extra_body.insert(
        "chatgptWebSentinelProofToken".to_string(),
        json!("stored-proof-token"),
    );
    extra_body.insert(
        "chatgptWebSentinelTurnstileToken".to_string(),
        json!("stored-turnstile-token"),
    );
    extra_body.insert(
        "chatgptWebTurnTraceId".to_string(),
        json!("11111111-1111-4111-8111-111111111111"),
    );
    payload
        .headers
        .insert("Cookie".to_string(), "session-cookie=present".to_string());

    let response = execute(
        &Client::new(),
        Duration::from_secs(5),
        &payload,
        &make_request(),
        "auto",
        None,
    )
    .await
    .expect("cached f conversation response");

    assert_eq!(response.text, "Paris");

    let seen = state.seen.lock().unwrap().clone();
    assert_eq!(
        seen.iter()
            .map(|item| item.path.as_str())
            .collect::<Vec<_>>(),
        vec!["/backend-api/f/conversation"]
    );

    let conversation = &seen[0];
    assert_eq!(
        conversation
            .headers
            .get("openai-sentinel-chat-requirements-token")
            .map(String::as_str),
        Some("stored-requirements-token")
    );
    assert_eq!(
        conversation
            .headers
            .get("openai-sentinel-proof-token")
            .map(String::as_str),
        Some("stored-proof-token")
    );
    assert_eq!(
        conversation
            .headers
            .get("openai-sentinel-turnstile-token")
            .map(String::as_str),
        Some("stored-turnstile-token")
    );
    assert_eq!(
        conversation
            .headers
            .get("x-oai-turn-trace-id")
            .map(String::as_str),
        Some("11111111-1111-4111-8111-111111111111")
    );
    assert!(!conversation.headers.contains_key("cookie"));
    assert!(!conversation.headers.contains_key("origin"));
    for name in [
        "cache-control",
        "pragma",
        "priority",
        "sec-fetch-dest",
        "sec-fetch-mode",
        "sec-fetch-site",
        "sec-ch-ua-arch",
        "sec-ch-ua-bitness",
        "sec-ch-ua-full-version",
        "sec-ch-ua-full-version-list",
        "sec-ch-ua-model",
        "sec-ch-ua-platform-version",
    ] {
        assert!(
            !conversation.headers.contains_key(name),
            "unexpected f-conversation header: {name}"
        );
    }
}

#[tokio::test]
async fn execute_uses_dynamic_backend_conversation_when_mode_requests_it() {
    let state = RequestState::default();
    let base_url = spawn_chatgpt_f_conversation_server(state.clone()).await;
    let mut payload = make_payload(&base_url);
    let extra_body = payload.extra_body.as_mut().expect("extra body");
    extra_body.insert(
        "chatgptWebConversationMode".to_string(),
        json!("dynamic_backend_conversation"),
    );

    let response = execute(
        &Client::new(),
        Duration::from_secs(5),
        &payload,
        &make_request(),
        "auto",
        None,
    )
    .await
    .expect("dynamic backend conversation response");

    assert_eq!(response.text, "Paris");

    let seen = state.seen.lock().unwrap().clone();
    assert_eq!(
        seen.iter()
            .map(|item| item.path.as_str())
            .collect::<Vec<_>>(),
        vec![
            "/backend-api/sentinel/chat-requirements",
            "/backend-api/conversation",
        ]
    );

    let conversation = &seen[1];
    assert_eq!(
        conversation
            .headers
            .get("x-openai-target-path")
            .map(String::as_str),
        Some("/backend-api/conversation")
    );
    assert_eq!(
        conversation
            .headers
            .get("openai-sentinel-chat-requirements-token")
            .map(String::as_str),
        Some("requirements-token")
    );
    assert!(!conversation.headers.contains_key("x-oai-is"));
    assert!(!conversation.headers.contains_key("x-conduit-token"));
}
