//! Actual loopback Upgrade, masking, session sequencing, bounded failure and socket cancellation.
use super::{test_support as wire, *};
use crate::upstream::{client::UpstreamClient, response_types::UpstreamStreamingResponse};
use futures::StreamExt;
use serde_json::json;
use std::time::Duration;

fn client(server: &wire::Server) -> UpstreamClient {
    let mut client = UpstreamClient::new(5);
    client.http = server.client();
    client
}
#[tokio::test]
async fn grok_web_ws_dispatch_buffered_is_current_mode_exact_account_without_usage() {
    for attached_first in [false, true] {
        let server = wire::Server::start(wire::Reply {
            attached_first,
            ..Default::default()
        })
        .await;
        let client = client(&server);
        let caller = std::collections::HashMap::from([
            ("cookie".into(), "foreign=caller-secret".into()),
            ("authorization".into(), "Bearer caller-secret".into()),
        ]);
        let answer = wire::within(
            &server.origin,
            client.execute_with_provider_account_id(
                "exact",
                &wire::payload(),
                &wire::request(),
                "grok-chat-fast",
                Some(&caller),
            ),
        )
        .await
        .unwrap();
        assert_eq!(answer.text, "合成答案");
        assert!(answer.usage.is_none() && answer.model_declaration.is_none());
        assert_eq!(answer.finish_reason.as_deref(), Some("stop"));
        let calls = server.calls();
        assert_eq!(calls.len(), 2);
        assert!(calls[0].uri.ends_with("/api/auth/session"));
        assert!(calls[1]
            .uri
            .ends_with("/ws/mgw/?uid=00000000-0000-4000-8000-000000000001"));
        for call in calls {
            assert_eq!(
                call.headers["cookie"],
                "sso=synthetic-exact; sso-rw=synthetic-exact; cf_clearance=synthetic-clearance"
            );
            assert_eq!(call.headers["user-agent"], "exact-UA");
            assert!(!call.headers.contains_key("authorization"));
        }
        let events = server.events();
        assert_eq!(events.len(), 3);
        assert_eq!(events[0]["event"]["type"], "session.create");
        assert_eq!(events[0]["event"]["session"]["model"], "fast");
        assert_eq!(
            events[1]["event"]["item"]["x_grok"]["input_chunks"][0]["text"]["text"],
            "合成问题"
        );
        assert_eq!(events[2]["event"]["type"], "response.create");
        server.finish().await;
    }
}
#[tokio::test]
async fn grok_web_ws_stream_reports_terminal_only_after_completed_and_drop_closes_socket() {
    for cancel in [false, true] {
        let mut reply = wire::Reply::default();
        if cancel {
            reply.frames.pop();
        }
        let server = wire::Server::start(reply).await;
        let client = client(&server);
        let response = wire::within(
            &server.origin,
            client.execute_stream(&wire::payload(), &wire::request(), "grok-chat-auto", None),
        )
        .await
        .unwrap();
        let UpstreamStreamingResponse::NativeEvents(mut stream) = response else {
            panic!("Dedicated typed WS stream required")
        };
        let first = stream.next().await.unwrap().unwrap();
        assert!(String::from_utf8_lossy(&first).contains("合成答案"));
        assert!(!String::from_utf8_lossy(&first).contains("[DONE]"));
        if !cancel {
            let terminal = stream.next().await.unwrap().unwrap();
            assert!(String::from_utf8_lossy(&terminal).contains("[DONE]"));
            assert!(stream.next().await.is_none());
        }
        drop(stream);
        server.finish().await;
    }
}
#[tokio::test]
async fn grok_web_ws_bad_json_eof_failed_status_and_error_are_not_success() {
    for ending in [
        "<close>".into(), "{bad".into(),
        json!({"event":{"type":"error","message":"synthetic-private-error"}}).to_string(),
        json!({"event":{"type":"response.done","response":{"id":"r","status":"failed"}}}).to_string(),
        json!({"event":{"type":"response.done","response":{"id":"r"}}}).to_string(),
        json!({"event":{"type":"response.grok.output","output":{"stream_error":{"message":"synthetic-private-error"}}}}).to_string(),
        "x".repeat(machine::FRAME_LIMIT + 1),
    ] {
        let mut reply = wire::Reply::default();
        reply.frames[1] = ending;
        let server = wire::Server::start(reply).await;
        let error = wire::within(&server.origin, client(&server).execute(
            &wire::payload(), &wire::request(), "fast", None
        )).await.unwrap_err();
        assert!(error.code.as_deref().unwrap().starts_with("grok_web_"));
        assert!(!error.to_string().contains("synthetic-private"));
        assert!(!error.retryable);
        assert_eq!(server.calls().len(), 2);
        server.finish().await;
    }
}
#[tokio::test]
async fn grok_web_ws_partial_then_failure_yields_typed_error_without_done() {
    let mut reply = wire::Reply::default();
    reply.frames[1] = "<close>".into();
    let server = wire::Server::start(reply).await;
    let response = wire::within(
        &server.origin,
        client(&server).execute_stream(&wire::payload(), &wire::request(), "fast", None),
    )
    .await
    .unwrap();
    let UpstreamStreamingResponse::NativeEvents(mut stream) = response else {
        panic!()
    };
    assert!(stream.next().await.unwrap().is_ok());
    assert!(stream.next().await.unwrap().is_err());
    assert!(stream.next().await.is_none());
    server.finish().await;
}
#[tokio::test]
async fn grok_web_ws_total_deadline_covers_waiting_without_terminal() {
    let mut reply = wire::Reply::default();
    reply.frames.clear();
    let server = wire::Server::start(reply).await;
    let error = wire::within(
        &server.origin,
        execute(
            &server.client(),
            Duration::from_millis(150),
            &wire::payload(),
            &wire::request(),
            "fast",
        ),
    )
    .await
    .unwrap_err();
    assert_eq!(error.code.as_deref(), Some("grok_web_timeout"));
    server.finish().await;
}
#[tokio::test]
async fn grok_web_ws_rejects_unsupported_inputs_and_audience_before_network() {
    let server = wire::Server::start(Default::default()).await;
    for model in ["grok-3", "grok-4", "sonar"] {
        assert!(client(&server)
            .execute(&wire::payload(), &wire::request(), model, None)
            .await
            .is_err());
    }
    let mut payload = wire::payload();
    payload.base_url = "https://example.invalid".into();
    assert!(client(&server)
        .execute(&payload, &wire::request(), "fast", None)
        .await
        .is_err());
    let mut request = wire::request();
    request.raw_body["temperature"] = json!(0.2);
    assert!(client(&server)
        .execute(&wire::payload(), &request, "fast", None)
        .await
        .is_err());
    assert!(server.calls().is_empty());
}
