//! Current endpoint, exact materials, explicit terminals, bounded failures and cancellation.
use super::{test_support as wire, *};
use crate::upstream::{client::UpstreamClient, response_types::UpstreamStreamingResponse};
use bytes::Bytes;
use futures::StreamExt;
use serde_json::json;
fn client(server: &wire::Server) -> UpstreamClient {
    let mut client = UpstreamClient::new(5);
    client.http = server.client();
    client
}
#[tokio::test]
async fn qwen_web_current_buffered_uses_fixed_origin_exact_material_and_reported_usage() {
    let server = wire::Server::start(Default::default()).await;
    let caller = std::collections::HashMap::from([
        ("authorization".into(), "Bearer foreign".into()),
        ("cookie".into(), "foreign=caller".into()),
    ]);
    let answer = wire::within(
        &server.origin,
        client(&server).execute(
            &wire::payload(),
            &wire::request(),
            "qwen3.7-plus",
            Some(&caller),
        ),
    )
    .await
    .unwrap();
    assert_eq!(answer.text, "合成答案");
    assert_eq!(answer.usage.unwrap().total_tokens, 5);
    let calls = server.calls();
    assert_eq!(calls.len(), 2);
    assert_eq!(calls[0].uri, "/api/v2/chats/new");
    assert_eq!(
        calls[1].uri,
        "/api/v2/chat/completions?chat_id=synthetic-chat"
    );
    for call in &calls {
        assert_eq!(call.headers["authorization"], "Bearer synthetic-exact");
        assert_eq!(
            call.headers["cookie"],
            "token=synthetic-exact; cf_clearance=synthetic-clearance"
        );
        assert_eq!(call.headers["user-agent"], "exact-UA");
    }
    assert_eq!(calls[0].body["models"][0], "qwen3.7-plus");
    assert_eq!(calls[1].body["messages"][0]["content"], " 合成问题 ");
    assert!(calls[1].body["parent_id"].is_null());
    assert_eq!(
        calls[1].body["messages"][0]["feature_config"]["auto_search"],
        false
    );
    server.finish().await;
}
#[tokio::test]
async fn qwen_web_current_stream_requires_explicit_terminal_and_drop_releases_body() {
    for cancel in [false, true] {
        let mut reply = wire::Reply::default();
        if cancel {
            reply.chunks.pop();
            reply.hold = true;
        }
        let server = wire::Server::start(reply).await;
        let response = wire::within(
            &server.origin,
            client(&server).execute_stream(
                &wire::payload(),
                &wire::request(),
                "qwen3.7-plus",
                None,
            ),
        )
        .await
        .unwrap();
        let UpstreamStreamingResponse::NativeEvents(mut stream) = response else {
            panic!("Typed current stream required")
        };
        let first = stream.next().await.unwrap().unwrap();
        assert!(String::from_utf8_lossy(&first).contains("合成答案"));
        assert!(!String::from_utf8_lossy(&first).contains("[DONE]"));
        if !cancel {
            assert!(
                String::from_utf8_lossy(&stream.next().await.unwrap().unwrap()).contains("[DONE]")
            );
            assert!(stream.next().await.is_none());
        }
        drop(stream);
        server.finish().await;
    }
}
#[tokio::test]
async fn qwen_web_current_partial_eof_errors_unknown_and_stopped_never_complete_or_replay() {
    for ending in [
        "",
        "data: {bad}\n\n",
        "data: {\"error\":\"synthetic-private\",\"done\":true}\n\n",
        "data: {\"response.stopped\":{}}\n\n",
        "data: {\"unknown\":true}\n\n",
    ] {
        let mut reply = wire::Reply::default();
        reply.chunks[1] = Bytes::from(ending.to_owned());
        let server = wire::Server::start(reply).await;
        let error = wire::within(
            &server.origin,
            client(&server).execute(&wire::payload(), &wire::request(), "qwen3.7-plus", None),
        )
        .await
        .unwrap_err();
        assert!(error
            .code
            .as_deref()
            .unwrap()
            .starts_with("qwen_web_current_"));
        assert!(!error.to_string().contains("synthetic-private"));
        assert!(!error.retryable);
        assert_eq!(server.calls().len(), 2);
        server.finish().await;
    }
}
#[tokio::test]
async fn qwen_web_current_auth_rate_redirect_and_invalid_create_do_not_replay() {
    for (create, status) in [
        (401, 200),
        (403, 200),
        (429, 200),
        (302, 200),
        (200, 401),
        (200, 429),
        (200, 302),
    ] {
        let server = wire::Server::start(wire::Reply {
            create_status: create,
            status,
            ..Default::default()
        })
        .await;
        assert!(wire::within(
            &server.origin,
            client(&server).execute(&wire::payload(), &wire::request(), "qwen3.7-plus", None)
        )
        .await
        .is_err());
        assert_eq!(server.calls().len(), if create == 200 { 2 } else { 1 });
        server.finish().await;
    }
    let server = wire::Server::start(wire::Reply {
        create: json!({"success":false,"data":{"id":"synthetic-chat"}}),
        ..Default::default()
    })
    .await;
    assert!(wire::within(
        &server.origin,
        client(&server).execute(&wire::payload(), &wire::request(), "qwen3.7-plus", None)
    )
    .await
    .is_err());
    assert_eq!(server.calls().len(), 1);
    server.finish().await;
}
#[tokio::test]
async fn qwen_web_current_input_audience_expiry_seed_and_cookie_reject_before_create() {
    let server = wire::Server::start(Default::default()).await;
    let mut request = wire::request();
    request.raw_body["temperature"] = json!(0.5);
    assert!(client(&server)
        .execute(&wire::payload(), &request, "qwen3.7-plus", None)
        .await
        .is_err());
    for field in ["base_url", "expires_at", "extra_body", "headers"] {
        let mut payload = wire::payload();
        match field {
            "base_url" => payload.base_url = "https://example.invalid".into(),
            "expires_at" => payload.expires_at = Some("2000-01-01T00:00:00Z".into()),
            "extra_body" => {
                payload.extra_body = Some(std::collections::HashMap::from([(
                    "authSeed".into(),
                    json!({"email":"synthetic"}),
                )]))
            }
            _ => {
                payload.headers.insert("Cookie".into(), "a=1; a=2".into());
            }
        }
        assert!(client(&server)
            .execute(&payload, &wire::request(), "qwen3.7-plus", None)
            .await
            .is_err());
    }
    assert!(server.calls().is_empty());
    server.finish().await;
}
#[tokio::test]
async fn qwen_web_current_total_deadline_includes_pending_body() {
    let mut reply = wire::Reply::default();
    reply.chunks.clear();
    reply.hold = true;
    let server = wire::Server::start(reply).await;
    let error = wire::within(
        &server.origin,
        execute(
            &server.client(),
            std::time::Duration::from_millis(150),
            &wire::payload(),
            &wire::request(),
            "qwen3.7-plus",
        ),
    )
    .await
    .unwrap_err();
    assert_eq!(error.code.as_deref(), Some("qwen_web_current_timeout"));
    server.finish().await;
}
