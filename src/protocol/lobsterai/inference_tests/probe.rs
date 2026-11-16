//! Console text generation must use the catalog-bound product sender, not generic HTTP.
use super::super::test_support::{Peer, Reply};
use super::*;
use crate::provider_runtime::{
    probe_console_target_with_request, ConsoleProbeRequest, ProviderPayloadProbeStatus,
};

#[tokio::test]
async fn lobsterai_console_probe_catalog_and_virtual_chat_messages_json_sse() {
    Box::pin(async {
        for virtual_model in [false, true] {
            for family in ["openai", "anthropic"] {
                for sse in [false, true] {
                    let mut replies = vec![];
                    if virtual_model {
                        replies.push(virtual_models::resolution(family));
                    }
                    replies.push(
                        Reply::raw(
                            200,
                            native::reply(
                                if family == "anthropic" {
                                    "messages"
                                } else {
                                    "chat"
                                },
                                false,
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
                        ),
                    );
                    let peer = Peer::start(replies).await;
                    let selected = if virtual_model {
                        "yd-fast"
                    } else {
                        UPSTREAM_MODEL
                    };
                    let f = Box::pin(Fixture::with_catalog(
                        peer.builder()
                            .retry(rquest::retry::Policy::never())
                            .build()
                            .unwrap(),
                        peer.origin.clone(),
                        selected,
                        json!([{"modelId":selected,"apiFormat":family,"supportsToolCalling":true}]),
                    ))
                    .await;
                    let report = super::super::transport::TEST_ORIGIN
                        .scope(peer.origin.clone(), Box::pin(call(&f)))
                        .await;
                    assert_eq!(
                        report.status,
                        ProviderPayloadProbeStatus::Passed,
                        "{virtual_model}/{family}/{sse}: {report:?}"
                    );
                    assert_eq!(report.answer.as_deref(), Some("合成答案"));
                    peer.inspect(|calls| {
                        assert_eq!(calls.len(), if virtual_model { 2 } else { 1 });
                        if virtual_model {
                            assert_eq!(calls[0].path, "/api/models/yd-fast/resolve");
                            assert_eq!(calls[0].body["requiresImage"], false);
                            assert!(uuid::Uuid::parse_str(
                                calls[0].body["sessionId"].as_str().unwrap()
                            )
                            .is_ok());
                        }
                        let sent = calls.last().unwrap();
                        assert_eq!(
                            sent.path,
                            if family == "anthropic" {
                                "/api/proxy/v1/messages"
                            } else {
                                "/api/proxy/v1/chat/completions"
                            }
                        );
                        assert_eq!(sent.body["model"], UPSTREAM_MODEL);
                        assert_eq!(sent.body["stream"], false);
                        assert_eq!(sent.body["max_tokens"], 256);
                        for call in calls {
                            assert_eq!(call.headers["authorization"], format!("Bearer {KEY}"));
                            assert_eq!(
                                call.headers.contains_key("x-lobsterai-client-capabilities"),
                                virtual_model
                            );
                            for header in ["cookie", "x-api-key", "x-account-mode"] {
                                assert!(!call.headers.contains_key(header));
                            }
                        }
                    });
                    let rows = f.audits(1).await;
                    assert_eq!(rows[0].status, "completed");
                    assert!(rows[0].total_tokens.is_some());
                    let trace = rows[0].route_trace.as_ref().unwrap();
                    assert_eq!(trace["realCredentialRef"], "exact");
                    if virtual_model || family == "anthropic" {
                        assert_eq!(trace["upstreamModelResolution"]["modelId"], UPSTREAM_MODEL);
                        assert_eq!(
                            trace["selectedUpstreamTargetProtocolFamily"],
                            if family == "anthropic" {
                                "anthropic_messages"
                            } else {
                                "openai_chat"
                            }
                        );
                    }
                    assert_eq!(f.remaining().await, 2);
                    f.finish().await;
                }
            }
        }
    })
    .await;
}

#[tokio::test]
async fn lobsterai_console_probe_401_wakes_owner_but_only_next_call_uses_new_token() {
    Box::pin(async {
        let peer = Peer::start(vec![
            Reply::raw(401, KEY.as_bytes().to_vec()),
            Reply::json(json!({"code":0,"data":{"accessToken":"new-access"}})),
            Reply::json(chat()),
        ])
        .await;
        let f = Box::pin(Fixture::with_http(
            peer.builder()
                .retry(rquest::retry::Policy::never())
                .build()
                .unwrap(),
            peer.origin.clone(),
        ))
        .await;
        super::super::transport::TEST_ORIGIN
            .scope(peer.origin.clone(), async {
                let report = Box::pin(call(&f)).await;
                assert_eq!(report.status, ProviderPayloadProbeStatus::Failed);
                assert!(!format!("{report:?}").contains(KEY));
                tokio::time::timeout(Duration::from_secs(5), async {
                    loop {
                        if f.state.route_config.snapshot().document().providers[0].credentials[0]
                            .api_key
                            .as_deref()
                            == Some("new-access")
                        {
                            break;
                        }
                        tokio::task::yield_now().await;
                    }
                })
                .await
                .unwrap();
                peer.inspect(|calls| assert_eq!(calls.len(), 2, "no implicit inference replay"));
                assert_eq!(
                    Box::pin(call(&f)).await.status,
                    ProviderPayloadProbeStatus::Passed
                );
            })
            .await;
        peer.inspect(|calls| {
            assert_eq!(calls.len(), 3);
            assert_eq!(calls[0].headers["authorization"], format!("Bearer {KEY}"));
            assert_eq!(calls[1].path, "/api/auth/refresh");
            assert_eq!(calls[2].headers["authorization"], "Bearer new-access");
        });
        let rows = f.audits(2).await;
        assert_eq!(rows.iter().filter(|r| r.status == "completed").count(), 1);
        assert_eq!(
            rows.iter()
                .filter(|r| r.status == "failed" && r.upstream_status == Some(401))
                .count(),
            1
        );
        assert_eq!(f.remaining().await, 2);
        f.finish().await;
    })
    .await;
}

pub(super) fn chat() -> Value {
    json!({"id":"probe","model":UPSTREAM_MODEL,"choices":[{"index":0,
        "message":{"role":"assistant","content":"OK"},"finish_reason":"stop"}],
        "usage":{"prompt_tokens":2,"completion_tokens":3,"total_tokens":5}})
}
pub(super) async fn call(f: &Fixture) -> crate::provider_runtime::ProviderPayloadProbeReport {
    let target = f
        .state
        .route_config
        .snapshot()
        .select_credential_probe_target("exact")
        .unwrap();
    let options: ConsoleProbeRequest =
        serde_json::from_value(json!({"model":MODEL,"prompt":"only OK"})).unwrap();
    probe_console_target_with_request(&f.state, &target, Some(&options))
        .await
        .1
}
#[tokio::test]
async fn lobsterai_console_probe_uses_product_sender_and_audit() {
    Box::pin(async {
        let peer = Peer::start(vec![Reply::json(chat())]).await;
        let f = Box::pin(Fixture::with_http(
            peer.builder().build().unwrap(),
            peer.origin.clone(),
        ))
        .await;
        let result = super::super::transport::TEST_ORIGIN
            .scope(peer.origin.clone(), Box::pin(call(&f)))
            .await;
        assert_eq!(
            result.status,
            ProviderPayloadProbeStatus::Passed,
            "{result:?}"
        );
        assert_eq!(result.answer.as_deref(), Some("OK"));
        peer.inspect(|calls| {
            assert_eq!(calls.len(), 1);
            assert_eq!(calls[0].path, "/api/proxy/v1/chat/completions");
            assert_eq!(calls[0].headers["authorization"], format!("Bearer {KEY}"));
            assert!(!calls[0].headers.contains_key("cookie"));
            assert_eq!(calls[0].body["model"], UPSTREAM_MODEL);
        });
        let rows = f.audits(1).await;
        assert_eq!(rows[0].status, "completed");
        assert_eq!(rows[0].total_tokens, Some(5));
        assert_eq!(f.remaining().await, 2);
        f.finish().await;
    })
    .await;
}
