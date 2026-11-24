//! 取消退还预付次数；未知拒绝不重放；未实现入口不外发。
use super::*;
#[tokio::test]
async fn sap_aicore_sse_cancel_refunds_and_releases_source() {
    Box::pin(async {
        for endpoint in ENDPOINTS {
            let peer = peer::Peer::new(family(endpoint), false, true, 200).await;
            let fixture = Box::pin(Fixture::new(&peer)).await;
            super::super::test_support::origin(peer.origin.clone(), async {
                let response = fixture.open(endpoint, body(endpoint, true)).await;
                assert_eq!(response.status(), StatusCode::OK);
                let mut body = response.into_body();
                let mut seen = Vec::new();
                tokio::time::timeout(Duration::from_secs(3), async {
                    while !String::from_utf8_lossy(&seen).contains("合成答案") {
                        seen.extend_from_slice(
                            &body.frame().await.unwrap().unwrap().into_data().unwrap(),
                        );
                    }
                })
                .await
                .unwrap();
                drop(body);
                let rows = fixture.audits(1).await;
                assert_eq!(rows[0].status, "cancelled");
                assert_eq!(rows[0].upstream_status, Some(200));
                assert_identity(&rows[0]);
                assert!(rows[0].total_tokens.is_none());
                assert_eq!(fixture.remaining().await, 2);
                assert_wire(&peer, 1, endpoint);
                peer.wait_closed().await;
                let states = fixture
                    .state
                    .local_runtime
                    .as_ref()
                    .unwrap()
                    .list_model_states(db::CredentialModelStateFilters::default())
                    .await
                    .unwrap();
                assert!(states.iter().all(|s| s.failure_count == 0));
            })
            .await;
            fixture.finish().await;
            peer.stop().await;
        }
    })
    .await;
}
#[tokio::test]
async fn sap_aicore_unknown_rejection_is_not_replayed_or_charged() {
    Box::pin(async {
        for endpoint in ENDPOINTS {
            let peer = peer::Peer::new(family(endpoint), false, false, 400).await;
            let fixture = Box::pin(Fixture::new(&peer)).await;
            super::super::test_support::origin(peer.origin.clone(), async {
                let response = fixture.open(endpoint, body(endpoint, false)).await;
                let status = response.status();
                let text = drain(response.into_body()).await;
                assert!(!status.is_success(), "{text}");
                assert!(!text.contains(KEY));
                let rows = fixture.audits(1).await;
                assert_identity(&rows[0]);
                assert_ne!(rows[0].status, "completed");
                assert_eq!(fixture.remaining().await, 2);
                assert_wire(&peer, 1, endpoint);
            })
            .await;
            fixture.finish().await;
            peer.stop().await;
        }
    })
    .await;
}
#[tokio::test]
async fn sap_aicore_unimplemented_endpoints_fail_before_network() {
    Box::pin(async {
        for (endpoint, body) in [
            ("embeddings", json!({"model":MODEL,"input":"hello"})),
            ("completions", json!({"model":MODEL,"prompt":"hello"})),
            ("responses", json!({"model":MODEL,"input":"hello","reasoning":{"effort":"high"}})),
            ("responses", json!({"model":MODEL,"input":[{"type":"reasoning","encrypted_content":"opaque"},{"role":"user","content":"hello"}]})),
            ("responses", json!({"model":MODEL,"input":[{"type":"custom_tool_call","name":"shell","input":"pwd","call_id":"call-1"}]})),
            ("responses", json!({"model":MODEL,"input":[{"role":"user","content":[{"type":"input_text","text":"read"},{"type":"input_file","file_id":"file-1"}]}]})),
            ("messages", json!({"model":MODEL,"max_tokens":32,"messages":[{"role":"user","content":"hello"}],"thinking":{"type":"enabled","budget_tokens":16}})),
            (
                "responses",
                json!({"model":MODEL,"input":"hello","store":true}),
            ),
            (
                "responses",
                json!({"model":MODEL,"input":"hello","previous_response_id":"resp_old"}),
            ),
        ] {
            let peer = peer::Peer::new("chat", false, false, 200).await;
            let fixture = Box::pin(Fixture::new(&peer)).await;
            super::super::test_support::origin(peer.origin.clone(), async {
                let response = fixture.open(endpoint, body).await;
                let status = response.status();
                let text = drain(response.into_body()).await;
                assert!(!status.is_success(), "{text}");
                assert!(peer.calls.lock().unwrap().is_empty());
                assert!(peer.oauth.lock().unwrap().is_empty());
                assert_ne!(fixture.audits(1).await[0].status, "completed");
                assert_eq!(fixture.remaining().await, 2);
            })
            .await;
            fixture.finish().await;
            peer.stop().await;
        }
    })
    .await;
}

#[tokio::test]
async fn sap_aicore_generic_connectivity_probes_never_send() {
    Box::pin(async {
        let peer = peer::Peer::new("chat", false, false, 200).await;
        let fixture = Box::pin(Fixture::new(&peer)).await;
        let target = fixture
            .state
            .route_config
            .snapshot()
            .select_credential_probe_target("exact")
            .unwrap();
        for options in [None] {
            let (_, report) = crate::provider_runtime::probe_console_target_with_request(
                &fixture.state,
                &target,
                options,
            )
            .await;
            assert!(matches!(
                report.status,
                crate::provider_runtime::ProviderPayloadProbeStatus::Unsupported
            ));
        }
        let report = crate::provider_runtime::probe_provider_payload_for_console(
            &peer.client(),
            &target.payload,
        )
        .await;
        assert!(!matches!(
            report.status,
            crate::provider_runtime::ProviderPayloadProbeStatus::Passed
        ));
        assert!(report.message.contains("Invalid SAP AI Core"));
        assert!(peer.calls.lock().unwrap().is_empty());
        assert!(peer.oauth.lock().unwrap().is_empty());
        fixture.finish().await;
        peer.stop().await;
    })
    .await;
}
