//! Public image requests exercise the configured SAP deployment, not a synthetic request plan alone.
use super::*;

const PNG: &str =
    "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAACklEQVR4nGNgAAIAAAUAAen63NgAAAAASUVORK5CYII=";

fn image_request(endpoint: &str, stream: bool) -> Value {
    let mut request = body(endpoint, stream);
    let url = format!("data:image/png;base64,{PNG}");
    match endpoint {
        "messages" => {
            request["messages"][0]["content"] = json!([
                {"type":"text","text":"describe"},
                {"type":"image","source":{"type":"base64","media_type":"image/png","data":PNG}}
            ])
        }
        "responses" => {
            request["input"] = json!([{"role":"user","content":[
                {"type":"input_text","text":"describe"}, {"type":"input_image","image_url":url,"detail":"high"}
            ]}])
        }
        _ => {
            request["messages"][0]["content"] = json!([
                {"type":"text","text":"describe"}, {"type":"image_url","image_url":{"url":url,"detail":"high"}}
            ])
        }
    }
    request
}

#[tokio::test]
async fn sap_aicore_inline_images_use_chat_wire_and_settle_json_and_sse() {
    Box::pin(async {
        for endpoint in ENDPOINTS {
            for stream in [false, true] {
                let peer = peer::Peer::new("chat", false, false, 200).await;
                let fixture = Box::pin(Fixture::with_images(&peer)).await;
                super::super::test_support::origin(peer.origin.clone(), async {
                    let response = fixture.open(endpoint, image_request(endpoint, stream)).await;
                    let status = response.status();
                    let text = drain(response.into_body()).await;
                    assert_eq!(status, StatusCode::OK, "{endpoint}/{stream}: {text}");
                    assert!(text.contains("合成答案"), "{text}");
                    if stream {
                        assert!(text.contains(match endpoint {
                            "messages" => "message_stop",
                            "responses" => "response.completed",
                            _ => "[DONE]",
                        }), "{text}");
                    }
                    assert_wire(&peer, 1, endpoint);
                    let mut expected = json!({"url":format!("data:image/png;base64,{PNG}")});
                    if endpoint != "messages" {
                        expected["detail"] = json!("high");
                    }
                    {
                        let calls = peer.calls.lock().unwrap();
                        let wire = &calls[0].2;
                        assert_eq!(wire["messages"], json!([{"role":"user","content":[
                            {"type":"text","text":"describe"}, {"type":"image_url","image_url":expected}
                        ]}]));
                        assert!(wire.get("sap_image_input").is_none());
                    }
                    let rows = fixture.audits(1).await;
                    assert_identity(&rows[0]);
                    assert!(!serde_json::to_string(&rows[0]).unwrap().contains(PNG));
                    assert_eq!(rows[0].status, "completed");
                    assert_eq!(rows[0].total_tokens, Some(5));
                    assert_eq!(fixture.remaining().await, 1);
                }).await;
                fixture.finish().await;
                peer.stop().await;
            }
        }
    }).await;
}

#[tokio::test]
async fn sap_aicore_native_responses_inline_images_preserve_wire_and_usage() {
    Box::pin(async {
        let peer = peer::Peer::responses_with_unique_ids("sap-inline".into()).await;
        let fixture = Box::pin(Fixture::configured(
            &peer,
            false,
            super::super::test_support::BASE,
            KEY,
            json!({"sap_api_version":"2024-10-21","sap_native_responses":true,
                "sap_native_responses_image_input":true}),
        ))
        .await;
        super::super::test_support::origin(peer.origin.clone(), async {
            for stream in [false, true] {
                let req = image_request("responses", stream);
                let res = fixture.open("responses", req.clone()).await;
                let status = res.status();
                let text = drain(res.into_body()).await;
                assert_eq!(status, StatusCode::OK, "stream={stream}: {text}");
                assert!(text.contains("合成答案") && text.contains(MODEL), "{text}");
                if stream {
                    assert!(text.contains("response.completed"), "{text}");
                }
                let calls = peer.calls.lock().unwrap();
                assert_eq!(calls.len(), if stream { 2 } else { 1 });
                let (path, headers, sent) = calls.last().unwrap();
                assert_eq!(path, "/v2/inference/deployments/deployment1/responses");
                assert_eq!(headers["AI-Resource-Group"], "group-a");
                assert_eq!(sent["input"], req["input"]);
                assert_eq!(sent["store"], false);
                assert!(sent.get("sap_native_responses_image_input").is_none());
            }
            assert_eq!(peer.oauth.lock().unwrap().len(), 1);
            for audit in fixture.audits(2).await {
                assert_identity(&audit);
                assert_eq!(audit.status, "completed");
                assert_eq!(audit.total_tokens, Some(5));
                assert!(!serde_json::to_string(&audit).unwrap().contains(PNG));
            }
            assert_eq!(fixture.remaining().await, 0);
        })
        .await;
        fixture.finish().await;
        peer.stop().await;
    })
    .await;
}

#[tokio::test]
async fn sap_aicore_native_responses_inline_images_reject_before_oauth() {
    Box::pin(async {
        let peer = peer::Peer::new("responses", false, false, 200).await;
        for enabled in [false, true] {
            let fixture = Box::pin(Fixture::configured(
                &peer,
                true,
                super::super::test_support::BASE,
                KEY,
                json!({"sap_api_version":"2024-10-21", "sap_image_input":true,
                    "sap_native_responses":true, "sap_native_responses_image_input":enabled}),
            )).await;
            let original = image_request("responses", false);
            let mut requests = vec![];
            if enabled {
                for image in [
                    json!({"type":"input_image","image_url":"https://image.invalid/picture.png"}),
                    json!({"type":"input_image","file_id":"file-remote"}),
                    json!({"type":"input_image","image_url":"data:image/gif;base64,YQ=="}),
                    json!({"type":"input_image","image_url":"data:image/png;base64,%%%"}),
                    json!({"type":"input_image","image_url":original["input"][0]["content"][1]["image_url"],"detail":"original"}),
                ] {
                    let mut request = original.clone();
                    request["input"][0]["content"][1] = image;
                    requests.push(request);
                }
                let mut request = original.clone();
                request["input"][0]["role"] = json!("assistant");
                requests.push(request);
                let mut request = original.clone();
                request["input"] = json!(vec![original["input"][0].clone(); 11]);
                requests.push(request);
            } else {
                // The older Chat vision flag is not proof of native Responses vision support.
                requests.push(original.clone());
                let mut request = original;
                request["sap_native_responses_image_input"] = json!(true);
                requests.push(request);
            }
            super::super::test_support::origin(peer.origin.clone(), async {
                for request in requests {
                    let response = fixture.open("responses", request).await;
                    let status = response.status();
                    let _ = drain(response.into_body()).await;
                    assert!(!status.is_success());
                    assert_eq!(fixture.remaining().await, 2);
                    assert!(peer.oauth.lock().unwrap().is_empty());
                    assert!(peer.calls.lock().unwrap().is_empty());
                }
            }).await;
            fixture.finish().await;
        }
        peer.stop().await;
    }).await;
}

#[tokio::test]
async fn sap_aicore_image_admission_rejects_before_oauth_and_refunds() {
    Box::pin(async {
        let peer = peer::Peer::new("chat", false, false, 200).await;
        let fixture = Box::pin(Fixture::new(&peer)).await;
        let mut false_override = image_request("chat/completions", false);
        false_override["sap_image_input"] = json!(true);
        assert_rejected(&fixture, &peer, "chat/completions", false_override, 1).await;
        fixture.finish().await;

        let fixture = Box::pin(Fixture::with_images(&peer)).await;
        let image_path = "/messages/0/content/1/image_url";
        let mut requests = Vec::new();
        for image in [
            json!({"url":"https://image.invalid/picture.png"}),
            json!({"url":"data:image/gif;base64,YQ=="}),
            json!({"url":"data:image/png;base64,%%%"}),
            json!({"url":format!("data:image/png;base64,{PNG}"), "detail": 2}),
            json!({"url":format!("data:image/png;base64,{PNG}"), "detail": "extreme"}),
        ] {
            let mut request = image_request("chat/completions", false);
            *request.pointer_mut(image_path).unwrap() = image;
            requests.push(("chat/completions", request));
        }
        let mut chat = image_request("chat/completions", false);
        chat["messages"][0]["role"] = json!("assistant");
        requests.push(("chat/completions", chat));
        let mut chat = image_request("chat/completions", false);
        let image = chat["messages"][0]["content"][1].clone();
        chat["messages"][0]["content"] = json!(vec![image; 11]);
        requests.push(("chat/completions", chat));
        let mut messages = image_request("messages", false);
        messages["messages"][0]["content"][1]["cache_control"] = json!({"type":"ephemeral"});
        requests.push(("messages", messages));
        let mut messages = body("messages", false);
        messages["system"] = json!([image_request("messages", false)["messages"][0]["content"][1]]);
        requests.push(("messages", messages));
        let mut responses = image_request("responses", false);
        responses["input"][0]["role"] = json!("developer");
        requests.push(("responses", responses));
        let mut responses = image_request("responses", false);
        responses["input"][0]["content"][1] = json!({"type":"input_image","file_id":"file-1"});
        requests.push(("responses", responses));
        let mut chat = image_request("chat/completions", false);
        chat["modalities"] = json!(["text", "audio"]);
        requests.push(("chat/completions", chat));

        for (i, (endpoint, request)) in requests.into_iter().enumerate() {
            assert_rejected(&fixture, &peer, endpoint, request, i + 1).await;
        }
        fixture.finish().await;
        peer.stop().await;
    })
    .await;
}

async fn assert_rejected(
    fixture: &Fixture,
    peer: &peer::Peer,
    endpoint: &str,
    request: Value,
    audit_count: usize,
) {
    super::super::test_support::origin(peer.origin.clone(), async {
        let response = fixture.open(endpoint, request).await;
        let status = response.status();
        let text = drain(response.into_body()).await;
        assert!(!status.is_success(), "{endpoint}: {text}");
        assert!(peer.calls.lock().unwrap().is_empty());
        assert!(peer.oauth.lock().unwrap().is_empty());
        let audits = fixture.audits(audit_count).await;
        assert!(audits.iter().all(|row| row.status != "completed"));
        assert_eq!(fixture.remaining().await, 2);
    })
    .await;
}

#[test]
fn sap_aicore_image_config_requires_stable_version_and_deployment_scope() {
    use crate::routing::config::RouteConfigStore;
    for (version, enabled, credential_override, valid) in [
        ("2024-10-21", json!(true), false, true),
        ("2024-10-21", json!(false), false, true),
        ("2023-05-15", json!(false), false, true),
        ("2023-05-15", json!(true), false, false),
        ("2024-10-21-preview", json!(true), false, false),
        ("2024-10-21", json!("true"), false, false),
        ("2024-10-21", json!(true), true, false),
    ] {
        let mut doc = super::super::tests::document();
        let provider = &mut doc["providers"][0];
        provider["extra_body"]["sap_api_version"] = json!(version);
        if credential_override {
            provider["credentials"][0]["extra_body"] = json!({"sap_image_input": enabled});
        } else {
            provider["extra_body"]["sap_image_input"] = enabled;
        }
        assert_eq!(
            RouteConfigStore::from_document(serde_json::from_value(doc).unwrap()).is_ok(),
            valid,
            "{version}, credential_override={credential_override}"
        );
    }
}
