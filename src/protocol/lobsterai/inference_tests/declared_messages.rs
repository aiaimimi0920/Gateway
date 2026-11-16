//! Directory-selected Messages uses the same public pipeline without a virtual resolution call.
use super::super::test_support::{Peer, Reply};
use super::*;
pub(super) const DECLARED: &str = "synthetic-declared-messages";
pub(super) async fn fixture(peer: &Peer, cash: bool) -> Fixture {
    Box::pin(Fixture::with_quota(
        peer.builder()
            .retry(rquest::retry::Policy::never())
            .build()
            .unwrap(),
        peer.origin.clone(),
        DECLARED,
        json!([{"modelId":DECLARED,"apiFormat":"anthropic","supportsToolCalling":true}]),
        cash,
    ))
    .await
}
pub(super) fn reply(sse: bool, tools: bool) -> Reply {
    Reply::raw(200, native::reply("messages", tools, sse, true)).header(
        "content-type",
        if sse {
            "text/event-stream"
        } else {
            "application/json"
        },
    )
}
pub(super) fn assert_wire(peer: &Peer, count: usize) {
    peer.inspect(|calls| {
        assert_eq!(calls.len(), count, "no resolver call or hidden retry");
        for call in calls {
            assert_eq!(call.path, "/api/proxy/v1/messages");
            assert_eq!(call.body["model"], DECLARED);
            assert_eq!(call.headers["authorization"], format!("Bearer {KEY}"));
            assert_eq!(call.headers["anthropic-version"], "2023-06-01");
            assert_eq!(
                call.headers["x-lobsterai-client-version"],
                super::super::runtime::VERSION
            );
            for header in [
                "cookie",
                "x-api-key",
                "x-management-token",
                "x-lobsterai-client-capabilities",
            ] {
                assert!(!call.headers.contains_key(header), "{header}");
            }
        }
    });
}
#[tokio::test]
async fn lobsterai_declared_messages_public_protocol_matrix() {
    Box::pin(async {
        for endpoint in ENDPOINTS {
            for stream in [false, true] {
                for sse in [false, true] {
                    let peer = Peer::start(vec![reply(sse, false)]).await;
                    let fixture = fixture(&peer, false).await;
                    super::super::transport::TEST_ORIGIN
                        .scope(peer.origin.clone(), async {
                            let response = fixture.open(endpoint, body(endpoint, stream)).await;
                            let status = response.status();
                            let text = drain(response.into_body()).await;
                            assert_eq!(status, StatusCode::OK, "{endpoint}/{stream}/{sse}: {text}");
                            assert!(text.contains("合成答案") && text.contains(MODEL), "{text}");
                        })
                        .await;
                    let audits = fixture.audits(1).await;
                    let row = &audits[0];
                    assert_eq!(row.status, "completed");
                    assert_eq!(row.requested_model.as_deref(), Some(MODEL));
                    assert_eq!(row.resolved_model.as_deref(), Some(DECLARED));
                    assert_eq!(row.total_tokens, Some(5));
                    let trace = row.route_trace.as_ref().unwrap();
                    assert_eq!(
                        trace["selectedUpstreamTargetProtocolFamily"],
                        "anthropic_messages"
                    );
                    assert_eq!(
                        trace["upstreamModelResolution"],
                        json!({"source":"catalog","modelId":DECLARED,"apiFormat":"anthropic"})
                    );
                    assert_eq!(fixture.remaining().await, 1);
                    fixture.finish().await;
                    assert_wire(&peer, 1);
                }
            }
        }
    })
    .await;
}
#[tokio::test]
async fn lobsterai_declared_messages_tools_roundtrip() {
    Box::pin(async {
        for endpoint in ENDPOINTS {
            for stream in [false, true] {
                let peer = Peer::start(vec![reply(!stream, true), reply(!stream, false)]).await;
                let fixture = fixture(&peer, false).await;
                super::super::transport::TEST_ORIGIN
                    .scope(peer.origin.clone(), async {
                        let mut raw = public_wire::request_body(endpoint, stream);
                        raw["model"] = json!(MODEL);
                        raw["tool_choice"] = if endpoint == "messages" {
                            json!({"type":"auto"})
                        } else {
                            json!("auto")
                        };
                        let response = fixture.open(endpoint, raw).await;
                        let status = response.status();
                        let text = drain(response.into_body()).await;
                        assert_eq!(status, StatusCode::OK, "{text}");
                        let calls = public_wire::calls(&text, endpoint, stream);
                        assert_eq!(calls.len(), 2, "{text}");
                        for call in &calls {
                            assert_eq!(call.name, "lookup");
                            assert_eq!(call.args, native::args());
                        }
                        let mut followup = public_wire::followup(endpoint, stream, &calls);
                        followup["model"] = json!(MODEL);
                        let response = fixture.open(endpoint, followup).await;
                        let status = response.status();
                        let text = drain(response.into_body()).await;
                        assert_eq!(status, StatusCode::OK, "{text}");
                        assert!(text.contains("合成答案"));
                        peer.inspect(|seen| {
                            let captured = seen
                                .iter()
                                .map(|entry| {
                                    (
                                        entry.path.clone(),
                                        entry.headers.clone(),
                                        entry.body.clone(),
                                    )
                                })
                                .collect();
                            native::assert_tools(&captured, "messages", &calls);
                        });
                    })
                    .await;
                assert!(fixture
                    .audits(2)
                    .await
                    .iter()
                    .all(|row| row.status == "completed"));
                assert_eq!(fixture.remaining().await, 0);
                fixture.finish().await;
                assert_wire(&peer, 2);
            }
        }
    })
    .await;
}
#[tokio::test]
async fn lobsterai_declared_messages_sender_uses_planned_response_protocol() {
    Box::pin(async {
        for stream in [false, true] {
            let peer = Peer::start(vec![reply(!stream, false)]).await;
            let fixture = fixture(&peer, false).await;
            let snapshot = fixture.state.route_config.snapshot();
            let payload = snapshot
                .oauth_refresh_target("lobsterai-public", "exact")
                .unwrap()
                .1
                .payload
                .clone();
            assert!(super::super::metadata::read(&payload)
                .unwrap()
                .selection
                .is_none());
            let request = crate::protocol::openai::normalize_chat_completions(body(
                "chat/completions",
                stream,
            ))
            .unwrap();
            super::super::transport::TEST_ORIGIN
                .scope(peer.origin.clone(), async {
                    if stream {
                        let response = super::super::runtime::execute_stream(
                            &fixture.state.upstream_client,
                            &payload,
                            &request,
                            DECLARED,
                        )
                        .await
                        .unwrap();
                        let crate::upstream::response_types::UpstreamStreamingResponse::Bytes(
                            mut bytes,
                        ) = response
                        else {
                            panic!("JSON must become canonical frames")
                        };
                        use futures::StreamExt;
                        let mut out = Vec::new();
                        while let Some(chunk) = bytes.next().await {
                            out.extend_from_slice(&chunk.unwrap());
                        }
                        assert!(String::from_utf8(out).unwrap().contains("合成答案"));
                    } else {
                        let response = super::super::runtime::execute(
                            &fixture.state.upstream_client,
                            &payload,
                            &request,
                            DECLARED,
                        )
                        .await
                        .unwrap();
                        assert_eq!(response.text, "合成答案");
                        assert_eq!(response.usage.unwrap().total_tokens, 5);
                    }
                })
                .await;
            fixture.finish().await;
            assert_wire(&peer, 1);
        }
    })
    .await;
}
