//! Resolve per public request, preserve the authorized billing alias, dispatch the returned protocol.
use super::super::test_support::{Peer, Reply};
use super::*;
pub(super) fn resolution(family: &str) -> Reply {
    Reply::json(
        json!({"code":0,"data":{"modelId":UPSTREAM_MODEL,"apiFormat":family,"supportsToolCalling":true}}),
    )
}
pub(super) async fn fixture(peer: &Peer, preset: &str) -> Fixture {
    Box::pin(Fixture::with_catalog(
        peer.builder()
            .retry(rquest::retry::Policy::never())
            .build()
            .unwrap(),
        peer.origin.clone(),
        preset,
        json!([{"modelId":preset,"apiFormat":"openai","accessible":true}]),
    ))
    .await
}
fn reply(family: &str, sse: bool, tools: bool) -> Reply {
    Reply::raw(
        200,
        native::reply(
            if family == "anthropic" {
                "messages"
            } else {
                "chat"
            },
            tools,
            sse,
            true,
        ),
    )
    .header(
        "content-type",
        if sse {
            "text/event-stream"
        } else {
            "application/json"
        },
    )
}
fn assert_wire(
    calls: &[super::super::test_support::Seen],
    preset: &str,
    family: &str,
    expected: usize,
) {
    assert_eq!(calls.len(), expected);
    for pair in calls.chunks_exact(2) {
        assert_eq!(pair[0].path, format!("/api/models/{preset}/resolve"));
        assert_eq!(pair[0].method, "POST");
        assert_eq!(pair[0].body["requiresImage"], false);
        assert!(uuid::Uuid::parse_str(pair[0].body["sessionId"].as_str().unwrap()).is_ok());
        assert_eq!(
            pair[1].path,
            if family == "anthropic" {
                "/api/proxy/v1/messages"
            } else {
                "/api/proxy/v1/chat/completions"
            }
        );
        assert_eq!(pair[1].body["model"], UPSTREAM_MODEL);
        for call in pair {
            assert_eq!(call.headers["authorization"], format!("Bearer {KEY}"));
            assert_eq!(
                call.headers["x-lobsterai-client-capabilities"],
                "virtual-models-v1"
            );
            assert!(!call.headers.contains_key("cookie"));
            assert!(!call.body.to_string().contains("gateway_lobsterai_dispatch"));
        }
    }
}
#[tokio::test]
async fn lobsterai_virtual_models_resolve_then_public_inference() {
    Box::pin(async {
        for preset in ["yd-fast", "yd-ultra"] {
            for family in ["openai", "anthropic"] {
                for endpoint in ENDPOINTS {
                    for stream in [false, true] {
                        let peer =
                            Peer::start(vec![resolution(family), reply(family, stream, false)])
                                .await;
                        let fixture = Box::pin(fixture(&peer, preset)).await;
                        let (status, text) = super::super::transport::TEST_ORIGIN
                            .scope(peer.origin.clone(), async {
                                let response = fixture.open(endpoint, body(endpoint, stream)).await;
                                (response.status(), drain(response.into_body()).await)
                            })
                            .await;
                        let audits = fixture.audits(1).await;
                        let balance = fixture.remaining().await;
                        fixture.finish().await;
                        assert_eq!(
                            status,
                            StatusCode::OK,
                            "{family}/{endpoint}/{stream}: {text}"
                        );
                        assert!(text.contains("合成答案"), "{text}");
                        assert!(text.contains(MODEL), "public model alias: {text}");
                        assert_eq!(audits[0].requested_model.as_deref(), Some(MODEL));
                        assert_eq!(audits[0].resolved_model.as_deref(), Some(preset));
                        let trace = audits[0].route_trace.as_ref().unwrap();
                        assert_eq!(
                            trace["upstreamModelResolution"],
                            json!({"preset":preset,"modelId":UPSTREAM_MODEL,"apiFormat":family})
                        );
                        assert_eq!(
                            trace["selectedUpstreamTargetProtocolFamily"],
                            if family == "anthropic" {
                                "anthropic_messages"
                            } else {
                                "openai_chat"
                            }
                        );
                        assert_eq!(audits[0].total_tokens, Some(5));
                        assert_eq!(audits[0].status, "completed");
                        assert_eq!(balance, 1);
                        peer.inspect(|calls| assert_wire(calls, preset, family, 2));
                    }
                }
            }
        }
    })
    .await;
}
#[tokio::test]
async fn lobsterai_virtual_tools_two_turns_and_response_type_selection() {
    Box::pin(async {
        for family in ["openai", "anthropic"] {
            for endpoint in ENDPOINTS {
                for stream in [false, true] {
                    // Intentionally choose a response type opposite to the requested stream flag.
                    let peer = Peer::start(vec![
                        resolution(family),
                        reply(family, !stream, true),
                        resolution(family),
                        reply(family, !stream, false),
                    ])
                    .await;
                    let fixture = Box::pin(fixture(&peer, "yd-fast")).await;
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
                            assert_eq!(
                                status,
                                StatusCode::OK,
                                "{family}/{endpoint}/{stream}: {text}"
                            );
                            let calls = public_wire::calls(&text, endpoint, stream);
                            assert_eq!(calls.len(), 2, "{text}");
                            for call in &calls {
                                assert_eq!(call.name, "lookup");
                                assert_eq!(call.args, native::args());
                            }
                            let mut raw = public_wire::followup(endpoint, stream, &calls);
                            raw["model"] = json!(MODEL);
                            let response = fixture.open(endpoint, raw).await;
                            let status = response.status();
                            let text = drain(response.into_body()).await;
                            assert_eq!(status, StatusCode::OK, "{text}");
                            assert!(text.contains("合成答案"));
                        })
                        .await;
                    assert!(fixture
                        .audits(2)
                        .await
                        .iter()
                        .all(|a| a.status == "completed"));
                    assert_eq!(fixture.remaining().await, 0);
                    fixture.finish().await;
                    peer.inspect(|calls| {
                        assert_wire(calls, "yd-fast", family, 4);
                        assert_ne!(calls[0].body["sessionId"], calls[2].body["sessionId"]);
                        assert!(calls[3].body["messages"].to_string().contains("lookup"));
                    });
                }
            }
        }
    })
    .await;
}
