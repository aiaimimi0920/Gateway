//! Native Responses must reach the real public route and settle against authoritative usage.
use super::*;

async fn fixture(peer: &peer::Peer, fallback: bool) -> Fixture {
    Box::pin(Fixture::configured(
        peer,
        fallback,
        super::super::test_support::BASE,
        KEY,
        json!({"sap_api_version":"2024-10-21","sap_native_responses":true}),
    ))
    .await
}

#[tokio::test]
async fn sap_aicore_native_responses_public_json_and_sse() {
    Box::pin(async {
        for stream in [false, true] {
            let peer = peer::Peer::new("responses", true, false, 200).await;
            let fixture = fixture(&peer, false).await;
            super::super::test_support::origin(peer.origin.clone(), async {
                let mut req = public_wire::request_body("responses", stream);
                req["model"] = json!(MODEL);
                req["reasoning"] = json!({"effort":"low"});
                req.as_object_mut().unwrap().remove("store");
                let response = fixture.open("responses", req.clone()).await;
                let status = response.status();
                let text = drain(response.into_body()).await;
                assert_eq!(status, StatusCode::OK, "{text}");
                let calls = public_wire::calls(&text, "responses", stream);
                assert_eq!(calls.len(), 2, "{text}");
                let mut next = public_wire::followup("responses", stream, &calls);
                next["model"] = json!(MODEL);
                // This flow returns results in call order; a separate shared pipeline test owns reordering.
                next["input"].as_array_mut().unwrap()[1 + calls.len()..].reverse();
                let response = fixture.open("responses", next.clone()).await;
                let status = response.status();
                let text = drain(response.into_body()).await;
                assert_eq!(status, StatusCode::OK, "{text}");
                assert!(text.contains("合成答案") && text.contains(MODEL), "{text}");
                assert!(!text.contains("opaque-model"), "{text}");
                if stream {
                    assert!(text.contains("response.completed"), "{text}");
                }
                let rows = fixture.audits(2).await;
                for row in rows {
                    assert_eq!(row.status, "completed");
                    assert_eq!(
                        (row.prompt_tokens, row.completion_tokens, row.total_tokens),
                        (Some(2), Some(3), Some(5))
                    );
                    assert_identity(&row);
                }
                assert_eq!(fixture.remaining().await, 0);
                assert_eq!(peer.oauth.lock().unwrap().len(), 1);
                let wire = peer.calls.lock().unwrap();
                assert_eq!(wire.len(), 2);
                for ((path, headers, sent), original) in wire.iter().zip([req, next]) {
                    assert_eq!(path, "/v2/inference/deployments/deployment1/responses");
                    assert_eq!(headers["authorization"], "Bearer synthetic-sap-access");
                    assert_eq!(headers["AI-Resource-Group"], "group-a");
                    assert!(!headers.contains_key("cookie"));
                    assert!(sent.get("model").is_none());
                    assert!(sent.get("sap_api_version").is_none());
                    assert_eq!(sent["store"], false);
                    assert_eq!(sent["stream"], stream);
                    assert_eq!(sent["input"], original["input"]);
                    assert_eq!(sent["tools"], original["tools"]);
                    assert_eq!(sent["tool_choice"], original["tool_choice"]);
                    assert_eq!(sent["reasoning"], original["reasoning"]);
                }
            })
            .await;
            fixture.finish().await;
            peer.stop().await;
        }
    })
    .await;
}

#[tokio::test]
async fn sap_aicore_native_responses_denies_remote_state_before_oauth() {
    Box::pin(async {
        let peer = peer::Peer::new("responses", false, false, 200).await;
        let fixture = fixture(&peer, true).await;
        super::super::test_support::origin(peer.origin.clone(), async {
            for (key, value) in [
                ("store", json!(true)),
                ("store", json!("false")),
                ("background", json!(true)),
                ("previous_response_id", json!(77)),
                ("conversation", json!({"id":"conv-remote"})),
                ("tools", json!([{"type":"web_search"}])),
                (
                    "input",
                    json!([{"type":"item_reference","id":"msg-remote"}]),
                ),
                (
                    "input",
                    json!([{"role":"user","content":[{"type":"input_file","file_id":"remote"}]}]),
                ),
            ] {
                let mut request = body("responses", false);
                // The valid but unsupported state request reaches the audit owner;
                // malformed fields can be rejected before an audit is opened.
                let request_is_persistent = key == "store" && value == true;
                request[key] = value;
                let res = fixture.open("responses", request).await;
                let status = res.status();
                // Drain errors too: the HTTP body retains the runtime owner until consumed.
                let text = drain(res.into_body()).await;
                assert!(!status.is_success(), "{key}: {text}");
                if request_is_persistent {
                    assert_ne!(fixture.audits(1).await[0].status, "completed");
                }
                assert_eq!(fixture.remaining().await, 2);
                assert!(peer.oauth.lock().unwrap().is_empty(), "{key}");
                assert!(peer.calls.lock().unwrap().is_empty(), "{key}");
            }
            let rows = fixture
                .state
                .local_runtime
                .as_ref()
                .unwrap()
                .list_audits(&db::RequestAuditFilters::default())
                .await
                .unwrap();
            assert!(rows.iter().all(|row| row.status != "completed"));
        })
        .await;
        fixture.finish().await;
        peer.stop().await;
    })
    .await;
}

#[tokio::test]
async fn sap_aicore_native_responses_failure_does_not_charge_or_replay() {
    Box::pin(async {
        let terminal = native::reply("responses", false, true, true);
        let wrong_id = String::from_utf8(terminal.clone()).unwrap().replacen(
            "native-resp",
            "conflicting-id",
            1,
        );
        for (stream, raw) in [
            (
                false,
                br#"{"error":{"message":"synthetic-private-error"}}"#.to_vec(),
            ),
            (
                false,
                String::from_utf8(native::reply("responses", false, false, true))
                    .unwrap()
                    .replace("\"total_tokens\":5", "\"total_tokens\":99")
                    .into_bytes(),
            ),
            (true, native::reply("responses", false, true, false)),
            (true, wrong_id.into_bytes()),
            (true, [terminal, b"data: {bad}\n\n".to_vec()].concat()),
        ] {
            let peer = peer::Peer::raw(raw).await;
            let fixture = fixture(&peer, true).await;
            super::super::test_support::origin(peer.origin.clone(), async {
                let res = fixture.open("responses", body("responses", stream)).await;
                let mut body = res.into_body();
                let mut output = Vec::new();
                tokio::time::timeout(Duration::from_secs(5), async {
                    while let Some(frame) = body.frame().await {
                        let Ok(frame) = frame else { break };
                        if let Ok(data) = frame.into_data() {
                            output.extend_from_slice(&data);
                        }
                    }
                })
                .await
                .unwrap();
                drop(body);
                let text = String::from_utf8(output).unwrap();
                assert!(
                    !text.contains("response.completed")
                        && !text.contains("synthetic-private-error"),
                    "{text}"
                );
                assert_ne!(fixture.audits(1).await[0].status, "completed");
                assert_eq!(fixture.remaining().await, 2);
                assert_eq!(peer.oauth.lock().unwrap().len(), 1);
                assert_eq!(peer.calls.lock().unwrap().len(), 1);
                peer.wait_closed().await;
            })
            .await;
            fixture.finish().await;
            peer.stop().await;
        }
    })
    .await;
}

#[tokio::test]
async fn sap_aicore_native_responses_cancel_releases_upstream_and_refunds() {
    Box::pin(async {
        let peer = peer::Peer::new("responses", false, true, 200).await;
        let fixture = fixture(&peer, true).await;
        super::super::test_support::origin(peer.origin.clone(), async {
            let res = fixture.open("responses", body("responses", true)).await;
            assert_eq!(res.status(), StatusCode::OK);
            let mut body = res.into_body();
            tokio::time::timeout(Duration::from_secs(3), async {
                loop {
                    let data = body.frame().await.unwrap().unwrap().into_data().unwrap();
                    if String::from_utf8_lossy(&data).contains("response.output_text.delta") {
                        break;
                    }
                }
            })
            .await
            .unwrap();
            drop(body);
            assert_eq!(fixture.audits(1).await[0].status, "cancelled");
            assert_eq!(fixture.remaining().await, 2);
            assert_eq!(peer.calls.lock().unwrap().len(), 1);
            peer.wait_closed().await;
        })
        .await;
        fixture.finish().await;
        peer.stop().await;
    })
    .await;
}
