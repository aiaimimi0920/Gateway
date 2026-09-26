use super::*;
use crate::protocol::canonical::{
    CanonicalMessage, ContentPart, EndpointKind, MessageRole, ProtocolFamily,
};

pub(super) fn make_payload() -> ProviderAccountPayload {
    ProviderAccountPayload {
        adapter: "freebuff_compatible".to_string(),
        base_url: "https://codebuff.com".to_string(),
        api_key: "fb-token".to_string(),
        credential_id: Some("cred-1".to_string()),
        expires_at: None,
        runtime_state_object_key: None,
        account_name: None,
        execution_mode: None,
        endpoint_execution_modes: None,
        default_model: Some("z-ai/glm-5.1".to_string()),
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
        extra_body: Some(
            [
                (
                    "freebuffModelAgentMap".to_string(),
                    json!({
                        "z-ai/glm-5.1": "base2-free",
                        "google/gemini-3.1-flash-lite-preview": ["basher", "researcher-web"]
                    }),
                ),
                ("freebuffRunRotationSecs".to_string(), json!(900)),
            ]
            .into_iter()
            .collect(),
        ),
        session_auth: None,
        keepalive: None,
    }
}

pub(super) fn make_request() -> CanonicalRelayRequest {
    CanonicalRelayRequest {
        protocol_family: ProtocolFamily::OpenAi,
        endpoint_kind: EndpointKind::ChatCompletions,
        requested_model: Some("z-ai/glm-5.1".to_string()),
        stream: false,
        messages: vec![CanonicalMessage {
            role: MessageRole::User,
            content: vec![ContentPart::Text {
                text: "hello".to_string(),
            }],
            name: None,
            tool_calls: Vec::new(),
            tool_call_id: None,
        }],
        tools: Vec::new(),
        tool_choice: None,
        reasoning: None,
        metadata: None,
        raw_body: json!({}),
        previous_response_id: None,
        explicit_session_key: None,
        extra: HashMap::new(),
    }
}

fn make_responses_request() -> CanonicalRelayRequest {
    let mut request = make_request();
    request.endpoint_kind = EndpointKind::Responses;
    request
}

fn assert_freebuff_chat_surface_plan(plan: &RequestPlan) {
    assert_eq!(plan.url, "https://www.codebuff.com/api/v1/chat/completions");
    assert_eq!(plan.response_kind, EndpointKind::ChatCompletions);
    assert_eq!(plan.body.as_ref().unwrap()["model"], "z-ai/glm-5.1");
    assert_eq!(plan.body.as_ref().unwrap()["stream"], true);
    assert!(plan.body.as_ref().unwrap().get("messages").is_some());
    assert!(plan.body.as_ref().unwrap().get("freebuffAgentId").is_none());
}

#[test]
fn reserves_runtime_extra_body_keys() {
    assert!(is_reserved_payload_extra_key("freebuffAgentId"));
    assert!(is_reserved_payload_extra_key("freebuffModelAgentMap"));
    assert!(is_reserved_payload_extra_key("freebuffRunRotationSecs"));
    assert!(is_reserved_payload_extra_key("freebuffSessionPath"));
    assert!(is_reserved_payload_extra_key(
        "freebuffSessionPollIntervalMs"
    ));
    assert!(is_reserved_payload_extra_key(
        "freebuffSessionPollTimeoutMs"
    ));
    assert!(!is_reserved_payload_extra_key("temperature"));
}

#[test]
fn normalizes_bare_codebuff_base_url_to_www() {
    assert_eq!(
        normalize_base_url("https://codebuff.com"),
        "https://www.codebuff.com"
    );
    assert_eq!(
        normalize_base_url("https://codebuff.com/api"),
        "https://www.codebuff.com/api"
    );
    assert_eq!(
        normalize_base_url("https://www.codebuff.com"),
        "https://www.codebuff.com"
    );
}

#[test]
fn freebuff_request_plan_normalizes_base_url_and_chat_surface() {
    let mut payload = make_payload();
    payload.chat_completions_path = Some("/api/v1/chat/completions".to_string());
    let req = make_responses_request();
    let plan = build_request_plan(&payload, &req, "z-ai/glm-5.1", true).unwrap();
    assert_eq!(plan.method, Method::POST);
    assert_eq!(plan.url, "https://www.codebuff.com/api/v1/chat/completions");
    assert_eq!(plan.response_kind, EndpointKind::ChatCompletions);
    assert_eq!(plan.body.as_ref().unwrap()["model"], "z-ai/glm-5.1");
    assert_eq!(plan.body.as_ref().unwrap()["stream"], true);
}

#[test]
fn plan_freebuff_rejects_unsupported_endpoint() {
    let mut req = make_request();
    req.endpoint_kind = EndpointKind::Embeddings;
    let err = build_request_plan(&make_payload(), &req, "z-ai/glm-5.1", false)
        .expect_err("freebuff should reject embeddings");
    assert_eq!(err.http_status, Some(400));
    assert_eq!(err.code.as_deref(), Some("unsupported_freebuff_endpoint"));
}

#[test]
fn plan_freebuff_compatible_url_and_body() {
    let mut payload = make_payload();
    payload.chat_completions_path = Some("/api/v1/chat/completions".to_string());
    payload.extra_body = Some(HashMap::from([(
        "freebuffAgentId".to_string(),
        json!("base2-free"),
    )]));
    let req = make_responses_request();
    let plan = build_request_plan(&payload, &req, "z-ai/glm-5.1", true).unwrap();
    assert_freebuff_chat_surface_plan(&plan);
}

#[test]
fn resolves_agent_from_model_map() {
    let payload = make_payload();
    let config = FreeBuffRuntimeConfig::from_payload(&payload, "z-ai/glm-5.1").unwrap();
    assert_eq!(config.agent_id, "base2-free");
    assert_eq!(config.rotation_interval, Duration::from_secs(900));
}

#[test]
fn request_body_injects_codebuff_metadata() {
    let request = make_request();
    let body = build_chat_request_body(
        &request,
        "z-ai/glm-5.1",
        true,
        "run-123",
        "free",
        Some("instance-123"),
    );
    assert_eq!(body["model"], "z-ai/glm-5.1");
    assert_eq!(body["stream"], true);
    assert_eq!(body["codebuff_metadata"]["run_id"], "run-123");
    assert_eq!(body["codebuff_metadata"]["cost_mode"], "free");
    assert_eq!(
        body["codebuff_metadata"]["freebuff_instance_id"],
        "instance-123"
    );
    assert!(body["codebuff_metadata"]["client_id"]
        .as_str()
        .is_some_and(|value| value.len() == 13));
}

#[test]
fn request_body_flattens_json_parts_for_freebuff_chat_surface() {
    let mut request = make_request();
    request.messages.push(CanonicalMessage {
        role: MessageRole::Tool,
        content: vec![ContentPart::Json {
            value: json!({
                "city": "Hangzhou",
                "condition": "sunny"
            }),
        }],
        name: None,
        tool_calls: Vec::new(),
        tool_call_id: Some("toolu_weather".to_string()),
    });

    let body = build_chat_request_body(
        &request,
        "z-ai/glm-5.1",
        false,
        "run-123",
        "free",
        Some("instance-123"),
    );
    assert_eq!(
        body["messages"][1]["content"],
        json!("{\"city\":\"Hangzhou\",\"condition\":\"sunny\"}")
    );
}

#[test]
fn parses_active_freebuff_session_snapshot() {
    let config = FreeBuffRuntimeConfig::from_payload(&make_payload(), "z-ai/glm-5.1").unwrap();
    let snapshot = parse_free_session_snapshot(
        &json!({
            "status": "active",
            "instanceId": "inst-active",
            "remainingMs": 3600000,
            "admittedAt": "2026-04-20T00:00:00.000Z",
            "expiresAt": "2026-04-20T01:00:00.000Z"
        }),
        &config,
    )
    .unwrap();
    assert!(matches!(snapshot.state, FreeBuffSessionState::Active));
    assert_eq!(snapshot.instance_id.as_deref(), Some("inst-active"));
    assert!(snapshot.is_fresh());
}

#[test]
fn parses_queued_freebuff_session_snapshot() {
    let config = FreeBuffRuntimeConfig::from_payload(&make_payload(), "z-ai/glm-5.1").unwrap();
    let snapshot = parse_free_session_snapshot(
        &json!({
            "status": "queued",
            "instanceId": "inst-queued",
            "position": 3,
            "queueDepth": 10,
            "estimatedWaitMs": 4200
        }),
        &config,
    )
    .unwrap();
    assert!(matches!(snapshot.state, FreeBuffSessionState::Queued));
    assert_eq!(snapshot.instance_id.as_deref(), Some("inst-queued"));
    assert_eq!(snapshot.queue_position, Some(3));
    assert_eq!(snapshot.queue_depth, Some(10));
    assert_eq!(snapshot.estimated_wait_ms, Some(4_200));
}

#[test]
fn classifies_waiting_room_rejections() {
    assert_eq!(
        classify_waiting_room_rejection(
            426,
            r#"{"error":{"code":"freebuff_update_required","message":"missing instance"}}"#
        ),
        Some(FreeBuffWaitingRoomRejection::MissingInstance)
    );
    assert_eq!(
        classify_waiting_room_rejection(
            428,
            r#"{"error":{"code":"waiting_room_required","message":"waiting room required"}}"#
        ),
        Some(FreeBuffWaitingRoomRejection::WaitingRoomRequired)
    );
    assert_eq!(
        classify_waiting_room_rejection(
            429,
            r#"{"error":{"code":"waiting_room_queued","message":"queued"}}"#
        ),
        Some(FreeBuffWaitingRoomRejection::WaitingRoomQueued)
    );
    assert_eq!(
        classify_waiting_room_rejection(
            409,
            r#"{"error":{"code":"session_superseded","message":"superseded"}}"#
        ),
        Some(FreeBuffWaitingRoomRejection::SessionSuperseded)
    );
    assert_eq!(
        classify_waiting_room_rejection(
            410,
            r#"{"error":{"code":"session_expired","message":"expired"}}"#
        ),
        Some(FreeBuffWaitingRoomRejection::SessionExpired)
    );
}

#[test]
fn detects_invalid_run_errors() {
    assert!(should_retry_with_fresh_run(
        400,
        r#"{"message":"runId not found"}"#
    ));
    assert!(should_retry_with_fresh_run(
        400,
        r#"{"error":{"message":"runId not running"}}"#
    ));
    assert!(!should_retry_with_fresh_run(429, "runId not found"));
}

#[test]
fn auth_error_is_marked_as_token_invalidated() {
    let error = classify_auth_error("Unauthorized");
    assert_eq!(error.http_status, Some(401));
    assert!(error.message.contains("token_invalidated"));
    assert_eq!(error.code.as_deref(), Some("token_invalidated"));
}
