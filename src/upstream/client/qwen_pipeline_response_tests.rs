//! Malformed upstream success bodies must not acquire downstream success or usage evidence.
use super::*;
#[test]
fn qwen_pipeline_error_envelope_is_not_success() {
    run(error_envelope);
}
async fn error_envelope() {
    let mut body = wire::completed();
    body["error"] = json!({"message":"private-upstream-detail"});
    rejects(body.to_string(), false).await;
}
#[test]
fn qwen_pipeline_missing_terminal_is_not_success() {
    run(missing_terminal);
}
async fn missing_terminal() {
    let mut body = wire::completed();
    body["choices"][0]
        .as_object_mut()
        .unwrap()
        .remove("finish_reason");
    rejects(body.to_string(), false).await;
}
#[test]
fn qwen_pipeline_invalid_message_is_not_success() {
    run(invalid_message);
}
async fn invalid_message() {
    let mut body = wire::completed();
    body["choices"][0]["message"] = json!({});
    rejects(body.to_string(), false).await;
}
#[test]
fn qwen_pipeline_bounded_body_rejects_chunked_oversize() {
    run(oversize);
}
async fn oversize() {
    rejects(String::new(), true).await;
}
async fn rejects(reply: String, oversized: bool) {
    let server = wire::Server::start(reply, 200, oversized).await;
    let fixture = Fixture::new(&server).await;
    let (status, body) = fixture.request(&server, "chat/completions", true).await;
    assert!(
        !status.is_success(),
        "Malformed body accepted with HTTP {status}; oversized={oversized}"
    );
    assert!(!body.to_string().contains("private-upstream-detail"));
    assert_eq!(
        body["error"]["code"],
        if oversized {
            "upstream_body_too_large"
        } else {
            "invalid_qwen_workspace_response"
        }
    );
    assert_wire(&server);
    let audits = fixture.audits().await;
    assert_eq!(audits.len(), 1);
    assert_ne!(audits[0].status, "completed");
    assert_ne!(audits[0].status, "started");
    assert!(audits[0].total_tokens.is_none());
    fixture.finish().await;
    server.stop().await;
}

#[test]
fn qwen_pipeline_invalid_public_key_never_reaches_upstream() {
    run(unauthorized);
}
async fn unauthorized() {
    let server = wire::Server::start(wire::completed().to_string(), 200, false).await;
    let fixture = Fixture::new(&server).await;
    let (status, _) = fixture.request(&server, "responses", false).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    assert_eq!(server.attempts(), 0);
    fixture.finish().await;
    server.stop().await;
}

#[test]
fn qwen_pipeline_fixture_blocks_https_when_transport_seam_is_absent() {
    run(blocks_https);
}
async fn blocks_https() {
    let server = wire::Server::start(Vec::new(), 200, false).await;
    assert!(server.client().get(BASE).send().await.is_err());
    assert_eq!(
        server.attempts(),
        1,
        "The local proxy must observe and reject CONNECT"
    );
    assert!(server.calls().is_empty());
    server.stop().await;
}

#[test]
fn qwen_pipeline_tools_length_and_filter_keep_terminal_and_usage_semantics() {
    run(terminals);
}
async fn terminals() {
    for endpoint in ["chat/completions", "responses"] {
        for terminal in ["length", "content_filter", "tool_calls"] {
            let mut reply = wire::completed();
            reply["choices"][0]["finish_reason"] = json!(terminal);
            reply["choices"][0]["message"]["content"] = Value::Null;
            if terminal == "tool_calls" {
                reply["choices"][0]["message"]["tool_calls"] = json!([{"id":"call-1","type":"function",
                    "function":{"name":"lookup","arguments":"{\"q\":\"x\"}"}}]);
            }
            let server = wire::Server::start(reply.to_string(), 200, false).await;
            let fixture = Fixture::new(&server).await;
            let (status, body) = fixture.request(&server, endpoint, true).await;
            assert_eq!(status, StatusCode::OK, "{body}");
            assert_wire(&server);
            if endpoint == "responses" {
                assert_eq!(
                    body["status"],
                    if terminal == "tool_calls" {
                        "completed"
                    } else {
                        "incomplete"
                    }
                );
                assert_eq!(body["usage"]["output_tokens"], 2);
                if terminal == "tool_calls" {
                    assert_eq!(body["output"][0]["type"], "function_call");
                    assert_eq!(body["output"][0]["name"], "lookup");
                    assert_eq!(body["output"][0]["arguments"], "{\"q\":\"x\"}");
                }
            } else {
                assert_eq!(body["choices"][0]["finish_reason"], terminal);
                assert_eq!(body["usage"]["completion_tokens"], 2);
                if terminal == "tool_calls" {
                    assert_eq!(
                        body["choices"][0]["message"]["tool_calls"][0]["function"]["name"],
                        "lookup"
                    );
                }
            }
            let audits = fixture.audits().await;
            assert_eq!(audits.len(), 1);
            assert_eq!(audits[0].total_tokens, Some(10));
            assert_eq!(
                audits[0].route_trace.as_ref().unwrap()["realCredentialRef"],
                "exact"
            );
            fixture.finish().await;
            server.stop().await;
        }
    }
}

#[test]
fn qwen_pipeline_invalid_utf8_and_json_are_rejected_without_body_echo() {
    run(invalid_encoding);
}
async fn invalid_encoding() {
    for bytes in [
        vec![0xff, 0xfe],
        b"{\"private-upstream-detail\":\"unfinished".to_vec(),
    ] {
        let server = wire::Server::start(bytes, 200, false).await;
        let fixture = Fixture::new(&server).await;
        let (status, body) = fixture.request(&server, "responses", true).await;
        assert!(!status.is_success());
        assert_eq!(body["error"]["code"], "invalid_qwen_workspace_response");
        assert!(!body.to_string().contains("private-upstream-detail"));
        assert_wire(&server);
        let audits = fixture.audits().await;
        assert_ne!(audits[0].status, "completed");
        fixture.finish().await;
        server.stop().await;
    }
}
