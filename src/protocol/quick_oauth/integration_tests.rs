//! End-to-end local loopback tests for Amazon Quick OAuth PKCE (manual + automatic 302 loopback),
//! token refresh, and bidirectional WebSocket EventStream execution (buffered + streaming + cleanup).

use super::{
    eventstream::encode_test_eventstream_frame,
    login::{LoginStepOutcome, PendingLogin},
    metadata,
    runtime::{execute, execute_stream},
    transport::build_http_client,
    AUTH_MODE, PORTAL_BASE, TEST_DATA_PLANE_ORIGIN, TEST_OIDC_ORIGIN, TEST_WS_URL,
};
use crate::{
    protocol::canonical::{
        CanonicalMessage, CanonicalRelayRequest, ContentPart, EndpointKind, MessageRole,
        ProtocolFamily,
    },
    routing::candidate::ProviderAccountPayload,
    upstream::{client::UpstreamClient, response_types::UpstreamStreamingResponse},
};
use axum::{routing::post, Json, Router};
use base64::{engine::general_purpose::STANDARD, Engine as _};
use futures::{SinkExt, StreamExt};
use serde_json::{json, Value};
use std::{collections::HashMap, sync::Arc};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::{TcpListener, TcpStream},
    sync::Mutex,
};
use tokio_tungstenite::{accept_async, tungstenite::Message};

async fn spawn_mock_oidc_and_dataplane() -> (String, String, tokio::task::JoinHandle<()>) {
    let app = Router::new()
        .route(
            "/client/register",
            post(|Json(body): Json<Value>| async move {
                assert_eq!(body["clientType"], "public");
                assert_eq!(body["scopes"], json!(["quicksight:read_write"]));
                Json(json!({
                    "clientId": "registered-client-1",
                    "clientSecret": "registered-secret-1",
                    "clientSecretExpiresAt": 4_000_000_000i64,
                }))
            }),
        )
        .route(
            "/token",
            post(|Json(body): Json<Value>| async move {
                match body["grantType"].as_str() {
                    Some("authorization_code") => Json(json!({
                        "accessToken": "access-tok-1",
                        "refreshToken": "refresh-tok-1",
                        "idToken": "id-jwt-1",
                        "expiresIn": 3600,
                    })),
                    Some("refresh_token") => Json(json!({
                        "accessToken": "access-tok-2",
                        "refreshToken": "refresh-tok-2",
                        "idToken": "id-jwt-2",
                        "expiresIn": 3600,
                    })),
                    _ => panic!("unexpected grantType"),
                }
            }),
        )
        .route(
            "/integration/quick-work/user-info",
            post(|| async {
                Json(json!({
                    "userInfo": {
                        "userArn": "arn:aws:sts::111122223333:assumed-role/QuickUser/alice",
                        "awsAccountId": "111122223333",
                        "email": "alice@example.com",
                        "displayName": "Alice",
                    }
                }))
            }),
        );
    let listener = TcpListener::bind(("127.0.0.1", 0)).await.unwrap();
    let origin = format!("http://127.0.0.1:{}", listener.local_addr().unwrap().port());
    let handle = tokio::spawn(async move {
        let _ = axum::serve(listener, app).await;
    });
    (origin.clone(), origin, handle)
}

fn extract_surface_nonce(discovery_url: &str) -> String {
    let parsed = url::Url::parse(discovery_url).unwrap();
    let surface = parsed
        .query_pairs()
        .find(|(k, _)| k == "qwSurface")
        .unwrap()
        .1
        .into_owned();
    surface.split(':').nth(1).unwrap().to_owned()
}

#[tokio::test]
async fn quick_oauth_manual_paste_and_refresh_grant_end_to_end() {
    let (oidc_origin, dp_origin, server) = spawn_mock_oidc_and_dataplane().await;
    TEST_OIDC_ORIGIN
        .scope(
            oidc_origin,
            TEST_DATA_PLANE_ORIGIN.scope(dp_origin, async {
                let http = build_http_client().unwrap();
                let (pending, _listener) = PendingLogin::start(http.clone()).await.unwrap();
                let nonce = extract_surface_nonce(pending.discovery_url());
                let redirect = pending.redirect_uri().to_owned();

                let step1 = format!(
                    "{redirect}/callback?nonce={nonce}&issuerUrl=https%3A%2F%2Fd-999999.awsapps.com%2Fstart&idcRegion=us-east-1"
                );
                let LoginStepOutcome::AuthorizeRedirect { authorize_url } =
                    pending.submit_callback_url(&step1).await.unwrap()
                else {
                    panic!("expected AuthorizeRedirect on step 1");
                };
                let auth_parsed = url::Url::parse(&authorize_url).unwrap();
                let state = auth_parsed
                    .query_pairs()
                    .find(|(k, _)| k == "state")
                    .unwrap()
                    .1
                    .into_owned();

                let step2 = format!("{redirect}/?code=auth-code-xyz&state={state}");
                let LoginStepOutcome::Completed(authorized) =
                    pending.submit_callback_url(&step2).await.unwrap()
                else {
                    panic!("expected Completed on step 2");
                };
                assert_eq!(authorized.bearer_token, "id-jwt-1");
                assert_eq!(authorized.refresh_token, "refresh-tok-1");
                assert_eq!(authorized.context.email(), Some("alice@example.com"));

                let grant = authorized
                    .context
                    .refresh_grant(&authorized.refresh_token)
                    .unwrap();
                let refreshed = grant.execute(&http).await.unwrap();
                assert_eq!(refreshed.bearer_token, "id-jwt-2");
                assert_eq!(refreshed.refresh_token.as_deref(), Some("refresh-tok-2"));
            }),
        )
        .await;
    server.abort();
}

#[tokio::test]
async fn quick_oauth_automatic_loopback_302_and_200_completion() {
    let (oidc_origin, dp_origin, server) = spawn_mock_oidc_and_dataplane().await;
    TEST_OIDC_ORIGIN
        .scope(
            oidc_origin,
            TEST_DATA_PLANE_ORIGIN.scope(dp_origin, async {
                let http = build_http_client().unwrap();
                let (pending, listener) = PendingLogin::start(http).await.unwrap();
                let nonce = extract_surface_nonce(pending.discovery_url());
                let port = url::Url::parse(pending.redirect_uri())
                    .unwrap()
                    .port()
                    .unwrap();

                let pending_clone = pending.clone();
                let loop_task =
                    tokio::spawn(TEST_OIDC_ORIGIN.scope(
                        TEST_OIDC_ORIGIN.with(|v| v.clone()),
                        TEST_DATA_PLANE_ORIGIN.scope(
                            TEST_DATA_PLANE_ORIGIN.with(|v| v.clone()),
                            async move { pending_clone.run_loopback(listener, |_| {}).await },
                        ),
                    ));

                let mut s1 = TcpStream::connect(("127.0.0.1", port)).await.unwrap();
                let req1 = format!(
                    "GET /callback?nonce={nonce}&issuerUrl=https%3A%2F%2Fd-123456.awsapps.com%2Fstart HTTP/1.1\r\nHost: 127.0.0.1:{port}\r\n\r\n"
                );
                s1.write_all(req1.as_bytes()).await.unwrap();
                let mut resp1 = Vec::new();
                s1.read_to_end(&mut resp1).await.unwrap();
                let resp1_text = String::from_utf8(resp1).unwrap();
                assert!(resp1_text.starts_with("HTTP/1.1 302 Found\r\n"));
                let location = resp1_text
                    .lines()
                    .find_map(|l| l.strip_prefix("Location: "))
                    .unwrap();
                let state = url::Url::parse(location)
                    .unwrap()
                    .query_pairs()
                    .find(|(k, _)| k == "state")
                    .unwrap()
                    .1
                    .into_owned();

                let mut s2 = TcpStream::connect(("127.0.0.1", port)).await.unwrap();
                let req2 = format!(
                    "GET /?code=loopback-code-1&state={state} HTTP/1.1\r\nHost: 127.0.0.1:{port}\r\n\r\n"
                );
                s2.write_all(req2.as_bytes()).await.unwrap();
                let mut resp2 = Vec::new();
                s2.read_to_end(&mut resp2).await.unwrap();
                assert!(String::from_utf8(resp2)
                    .unwrap()
                    .starts_with("HTTP/1.1 200 OK\r\n"));

                let session = loop_task.await.unwrap().unwrap();
                assert_eq!(session.bearer_token, "id-jwt-1");
            }),
        )
        .await;
    server.abort();
}

#[tokio::test]
async fn quick_oauth_websocket_execute_and_stream_emit_deltas_usage_and_session_delete() {
    let ws_listener = TcpListener::bind(("127.0.0.1", 0)).await.unwrap();
    let ws_url = format!(
        "ws://127.0.0.1:{}",
        ws_listener.local_addr().unwrap().port()
    );
    let seen_types: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));
    let seen_clone = seen_types.clone();

    let ws_server = tokio::spawn(async move {
        for _ in 0..2 {
            let (tcp, _) = ws_listener.accept().await.unwrap();
            let mut ws = accept_async(tcp).await.unwrap();
            for _ in 0..4 {
                let Some(Ok(Message::Binary(raw))) = ws.next().await else {
                    panic!("expected outbound binary frame");
                };
                let outer: Value = serde_json::from_slice(&raw).unwrap();
                let inner_b64 = outer["payload"]["payload"].as_str().unwrap();
                let inner: Value =
                    serde_json::from_slice(&STANDARD.decode(inner_b64).unwrap()).unwrap();
                seen_clone
                    .lock()
                    .await
                    .push(inner["type"].as_str().unwrap().to_owned());
            }
            for ev in [
                json!({"type": "text_delta", "payload": {"delta": "Hello "}}),
                json!({"type": "text_delta", "payload": {"delta": "Quick!"}}),
                json!({"type": "turn_usage", "payload": {"input_tokens": 12, "output_tokens": 5, "total_tokens": 17, "cache_read_tokens": 4}}),
                json!({"type": "turn_completed", "payload": {}}),
            ] {
                let outer = json!({ "payload": STANDARD.encode(serde_json::to_vec(&ev).unwrap()) });
                let bin = encode_test_eventstream_frame(
                    &[(":message-type", "event")],
                    &serde_json::to_vec(&outer).unwrap(),
                );
                ws.send(Message::Binary(bin)).await.unwrap();
            }
            if let Some(Ok(Message::Binary(del_raw))) = ws.next().await {
                let outer: Value = serde_json::from_slice(&del_raw).unwrap();
                let inner_b64 = outer["payload"]["payload"].as_str().unwrap();
                let inner: Value =
                    serde_json::from_slice(&STANDARD.decode(inner_b64).unwrap()).unwrap();
                seen_clone
                    .lock()
                    .await
                    .push(inner["type"].as_str().unwrap().to_owned());
            }
        }
    });

    let mut payload: ProviderAccountPayload = serde_json::from_value(json!({
        "adapter": "openai_compatible",
        "base_url": PORTAL_BASE,
        "api_key": "id-jwt-1",
        "auth_mode": AUTH_MODE,
    }))
    .unwrap();
    let ctx = super::AuthorizationContext {
        idc_region: "us-east-1".into(),
        issuer_url: "https://d-123456.awsapps.com/start".into(),
        tenant_url: None,
        ws_host: "127.0.0.1".into(),
        client_id: "cid".into(),
        client_secret: Some("csec".into()),
        client_secret_expires_at: None,
        actor_arn: Some("arn:aws:sts::111122223333:assumed-role/U".into()),
        aws_account_id: None,
        email: None,
        display_name: None,
        account_name: None,
    };
    metadata::attach(&mut payload, &ctx);

    let req = CanonicalRelayRequest {
        protocol_family: ProtocolFamily::OpenAi,
        endpoint_kind: EndpointKind::ChatCompletions,
        requested_model: Some("smart".into()),
        stream: false,
        messages: vec![CanonicalMessage {
            role: MessageRole::User,
            content: vec![ContentPart::Text {
                text: "Say hello".into(),
            }],
            name: None,
            tool_call_id: None,
            tool_calls: Vec::new(),
        }],
        tools: Vec::new(),
        tool_choice: None,
        reasoning: None,
        metadata: None,
        raw_body: json!({}),
        previous_response_id: None,
        explicit_session_key: None,
        extra: HashMap::new(),
    };
    let client = UpstreamClient::new(15);

    TEST_WS_URL
        .scope(ws_url, async {
            let buffered = execute(&client, &payload, &req, "smart").await.unwrap();
            assert_eq!(buffered.text, "Hello Quick!");
            assert_eq!(buffered.usage.as_ref().unwrap().total_tokens, 17);
            assert_eq!(
                buffered.usage.as_ref().unwrap().cache_read_input_tokens,
                Some(4)
            );

            let mut stream_req = req.clone();
            stream_req.stream = true;
            let UpstreamStreamingResponse::NativeEvents(mut rx) =
                execute_stream(&client, &payload, &stream_req, "smart")
                    .await
                    .unwrap()
            else {
                panic!("expected NativeEvents stream");
            };
            let mut sse = String::new();
            while let Some(chunk) = rx.next().await {
                sse.push_str(std::str::from_utf8(&chunk.unwrap()).unwrap());
            }
            assert!(sse.contains("Hello "));
            assert!(sse.contains("Quick!"));
            assert!(sse.contains("\"cached_tokens\":4"));
            assert!(sse.contains("[DONE]"));
        })
        .await;

    ws_server.await.unwrap();
    let recorded = seen_types.lock().await.clone();
    assert_eq!(
        recorded,
        vec![
            "session_connect",
            "federate_status",
            "session_create",
            "user_message",
            "session_delete",
            "session_connect",
            "federate_status",
            "session_create",
            "user_message",
            "session_delete",
        ]
    );
}
