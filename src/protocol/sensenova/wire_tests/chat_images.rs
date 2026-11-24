use super::*;

fn image_request(role: &str, url: &str, stream: bool) -> Value {
    json!({"model":MODEL,"stream":stream,"messages":[{"role":role,"content":[
        {"type":"text","text":"describe"},{"type":"image_url","image_url":{"url":url}}
    ]}]})
}

fn document(model: &str) -> Value {
    json!({"providers":[{"id":"sensenova-public","preset":"sensenova-token-plan",
        "base_url":super::super::BASE,"explicit_credentials_only":true,
        "supported_models":[MODEL],"model_map":{MODEL:model},
        "credentials":[{"id":"exact","api_key":KEY,"supported_models":[MODEL]}]}],"model_routes":[]})
}

#[tokio::test]
async fn sensenova_chat_images_preserve_wire_and_use_resolved_model() {
    Box::pin(async {
        for (model, url) in [
            ("sensenova-6.8-flash-lite", "https://image.invalid/img.png"),
            ("deepseek-flash", "data:image/gif;base64,R0lGODlh"),
            ("kimi-k3", "data:image/heic;base64,AAAB"),
        ] {
            let peer = peer::Peer::new("chat", false, false, 200).await;
            let fixture = Box::pin(Fixture::with_document(&peer, document(model), 2)).await;
            super::super::test_support::origin(peer.origin.clone(), async {
                for stream in [false, true] {
                    let reply = fixture
                        .open("chat/completions", image_request("user", url, stream))
                        .await;
                    assert_eq!(reply.status(), StatusCode::OK);
                    let output = drain(reply.into_body()).await;
                    assert!(output.contains("合成答案"), "{output}");
                    if stream {
                        assert!(output.contains("[DONE]"));
                    }
                }
                let rows = fixture.audits(2).await;
                assert!(rows.iter().all(|r| r.status == "completed"
                    && r.total_tokens == Some(5)
                    && r.requested_model.as_deref() == Some(MODEL)
                    && r.resolved_model.as_deref() == Some(model)));
                assert_eq!(fixture.remaining().await, 0);
                let calls = peer.calls.lock().unwrap();
                assert_eq!(calls.len(), 2);
                for (path, headers, wire) in calls.iter() {
                    assert_eq!(path, "/v1/chat/completions");
                    assert_eq!(headers["authorization"], format!("Bearer {KEY}"));
                    assert_eq!(wire["model"], model);
                    assert_eq!(wire["messages"][0]["content"][1]["image_url"]["url"], url);
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
async fn sensenova_chat_images_reject_invalid_source_before_upstream_send() {
    Box::pin(async {
        for (model, role, url, code) in [
            (
                "kimi-k3",
                "user",
                "https://image.invalid/img.png",
                "sensenova_chat_image_unsupported",
            ),
            (
                "sensenova-6.8-flash-lite",
                "user",
                "data:image/gif;base64,R0lGODlh",
                "sensenova_chat_image_unsupported",
            ),
            (
                "sensenova-6.8-flash-lite",
                "user",
                "file:///img.png",
                "sensenova_chat_image_unsupported",
            ),
            (
                "sensenova-6.8-flash-lite",
                "system",
                "https://image.invalid/img.png",
                "sensenova_chat_image_unsupported",
            ),
            (
                "glm-5.2",
                "user",
                "https://image.invalid/img.png",
                "sensenova_chat_image_model_unsupported",
            ),
        ] {
            let peer = peer::Peer::new("chat", false, false, 200).await;
            let fixture = Box::pin(Fixture::with_document(&peer, document(model), 2)).await;
            super::super::test_support::origin(peer.origin.clone(), async {
                let reply = fixture
                    .open("chat/completions", image_request(role, url, false))
                    .await;
                let status = reply.status();
                let output = drain(reply.into_body()).await;
                assert_eq!(status, StatusCode::BAD_REQUEST, "{output}");
                assert!(output.contains(code), "{output}");
                let rows = fixture.audits(1).await;
                assert_eq!(rows[0].status, "failed");
                assert!(rows[0].provider_account_id.is_none());
                let trace = rows[0].route_trace.as_ref().unwrap();
                assert_eq!(trace["routeAttemptCount"], 0);
                assert!(trace["realCredentialRef"].is_null());
                assert_eq!(trace["attemptedProviderIds"], json!([]));
                assert_eq!(trace["rejectedCandidates"][0]["reason"], code);
                assert_eq!(fixture.remaining().await, 2);
                assert!(peer.calls.lock().unwrap().is_empty());
            })
            .await;
            fixture.finish().await;
            peer.stop().await;
        }
    })
    .await;
}
