//! Exercise actual body transport failure, not a successful EOF with invalid protocol frames.
use super::*;

#[tokio::test]
async fn sap_aicore_stream_terminal_records_once_and_transport_failure_is_neutral() {
    Box::pin(async {
        for (hold, override_profile) in [(false, false), (true, false), (true, true)] {
            let peer = peer::Peer::new("chat", false, hold, 200).await;
            let f = Fixture::new(&peer).await;
            if override_profile {
                let mut document = f.state.route_config.snapshot().document().clone();
                document.providers[0].protocol_profile = Some("chatgpt_official_api".into());
                f.state.route_config.replace_document(document).unwrap();
            }
            super::super::super::test_support::origin(peer.origin.clone(), async {
                let before = metric("gateway_provider_requests_total", None);
                let response = f
                    .open("chat/completions", body("chat/completions", true))
                    .await;
                assert_eq!(response.status(), StatusCode::OK);
                let mut stream = response.into_body();
                let mut output = Vec::new();
                // Observing public data proves the source was handed off before transport loss.
                let first = tokio::time::timeout(Duration::from_secs(3), stream.frame())
                    .await
                    .unwrap()
                    .unwrap()
                    .unwrap()
                    .into_data()
                    .unwrap();
                output.extend_from_slice(&first);
                if hold {
                    peer.disconnect.notify_one();
                }
                tokio::time::timeout(Duration::from_secs(5), async {
                    while let Some(frame) = stream.frame().await {
                        match frame {
                            Ok(frame) => {
                                if let Ok(bytes) = frame.into_data() {
                                    output.extend_from_slice(&bytes);
                                }
                            }
                            Err(_) => break,
                        }
                    }
                })
                .await
                .unwrap();
                drop(stream);
                let rows = f.audits(1).await;
                assert_identity(&rows[0]);
                assert_eq!(rows[0].upstream_status, Some(200));
                let trace = rows[0].route_trace.as_ref().unwrap();
                assert_eq!(trace["routeAttemptCount"], 1);
                assert_eq!(rows[0].status, if hold { "failed" } else { "completed" });
                if override_profile {
                    assert_eq!(trace["selectedProtocolProfile"], "chatgpt_official_api");
                }
                assert_eq!(metric("gateway_provider_requests_total", None), before + 1);
                assert_eq!(peer.calls.lock().unwrap().len(), 1);
                assert_eq!(f.remaining().await, if hold { 2 } else { 1 });
                if hold {
                    assert_eq!(trace["terminationReason"], "upstream_interrupted");
                    assert_eq!(
                        rows[0].error_summary.as_deref(),
                        Some(crate::protocol::sap_aicore::errors::STREAM_UNVERIFIED)
                    );
                    assert!(
                        states(&f).await.is_empty(),
                        "transport loss is not model failure"
                    );
                    assert!(!String::from_utf8(output).unwrap().contains("[DONE]"));
                }
            })
            .await;
            f.finish().await;
            peer.stop().await;
        }
    })
    .await;
}

#[tokio::test]
async fn sap_aicore_connection_loss_after_oauth_is_one_neutral_attempt() {
    Box::pin(async {
        for stream in [false, true] {
            let peer = peer::Peer::new("chat", false, false, 200).await;
            let f = Fixture::new(&peer).await;
            let origin = peer.origin.clone();
            let calls = peer.calls.clone();
            let target = f
                .state
                .route_config
                .snapshot()
                .select_credential_probe_target("exact")
                .unwrap();
            super::super::super::test_support::origin(origin.clone(), async {
                f.state
                    .upstream_client
                    .sap_aicore
                    .get(f.state.upstream_client.client(), &target.payload)
                    .await
                    .unwrap();
            })
            .await;
            peer.stop().await;
            super::super::super::test_support::origin(origin, async {
                let before = metric("gateway_provider_requests_total", None);
                let response = f
                    .open("chat/completions", body("chat/completions", stream))
                    .await;
                assert_eq!(response.status(), StatusCode::BAD_GATEWAY);
                let _ = drain(response.into_body()).await;
                let rows = f.audits(1).await;
                assert_identity(&rows[0]);
                assert_eq!(rows[0].upstream_status, None);
                assert_eq!(
                    rows[0].route_trace.as_ref().unwrap()["routeAttemptCount"],
                    1
                );
                assert_eq!(metric("gateway_provider_requests_total", None), before + 1);
                assert!(states(&f).await.is_empty());
                assert_eq!(f.remaining().await, 2);
                assert!(
                    calls.lock().unwrap().is_empty(),
                    "closed peer received no inference"
                );
            })
            .await;
            f.finish().await;
        }
    })
    .await;
}
