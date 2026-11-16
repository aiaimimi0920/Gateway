//! Only the lossless single-string legacy subset is bridged; unsupported sampling never reaches HTTP.
use super::*;
use crate::{routing::candidate::ProviderAccountPayload, upstream::client::UpstreamClient};

#[tokio::test]
async fn nous_public_legacy_completions_both_targets_json_sse() {
    Box::pin(async {
        for target in TARGETS {
            for stream in [false, true] {
                let peer = peer::Peer::new("chat", false, false, 200).await;
                let fixture = Box::pin(Fixture::with_target(&peer, target)).await;
                super::super::test_support::origin(peer.origin.clone(), async {
                    let request = json!({"model":MODEL,"prompt":"合成问题","stream":stream,
                        "max_tokens":32,"temperature":0.2,"top_p":0.8,"stop":["终止"],
                        "presence_penalty":0.1,"frequency_penalty":0.3,"seed":42,
                        "n":1,"best_of":1,"echo":false,"logprobs":null,"suffix":null,"logit_bias":{}});
                    let response = fixture.open("completions", request.clone()).await;
                    let status = response.status();
                    let text = drain(response.into_body()).await;
                    assert_eq!(status, StatusCode::OK, "{target:?}/{stream}: {text}");
                    let replies: Vec<Value> = if stream {
                        assert_eq!(text.matches("data: [DONE]").count(), 1, "{text}");
                        text.lines().filter_map(|line| line.strip_prefix("data: "))
                            .filter(|data| *data != "[DONE]")
                            .map(|data| serde_json::from_str(data).unwrap()).collect()
                    } else { vec![serde_json::from_str(&text).unwrap()] };
                    let mut content = String::new();
                    let mut finished = false;
                    for reply in replies {
                        assert_eq!(reply["object"], "text_completion");
                        assert_eq!(reply["model"], MODEL);
                        assert!(reply["choices"][0].get("message").is_none());
                        content.push_str(reply["choices"][0]["text"].as_str().unwrap_or_default());
                        finished |= reply["choices"][0]["finish_reason"] == "stop";
                    }
                    assert_eq!(content, "合成答案");
                    assert!(finished);
                    let rows = fixture.audits(1).await;
                    assert_identity(&rows[0]);
                    assert_eq!(rows[0].status, "completed");
                    assert_eq!(rows[0].total_tokens, Some(5));
                    assert_eq!(fixture.remaining().await, 1);
                    assert_wire(&peer, 1);
                    let calls = peer.calls.lock().unwrap();
                    let raw = &calls[0].2;
                    assert_eq!(raw["messages"][0]["content"], "合成问题");
                    for key in ["max_tokens", "temperature", "top_p", "stop", "presence_penalty", "frequency_penalty", "seed"] {
                        assert_eq!(raw[key], request[key], "{key}");
                    }
                }).await;
                fixture.finish().await;
                peer.stop().await;
            }
        }
    }).await;
}

#[test]
fn nous_legacy_bridge_rejects_unrepresentable_controls_and_keeps_generic_owner() {
    let mut payload: ProviderAccountPayload = serde_json::from_value(json!({
        "adapter":"openai_compatible","auth_mode":"nous_portal_oauth",
        "base_url":"https://inference-api.nousresearch.com/v1","api_key":"synthetic-access"
    }))
    .unwrap();
    for patch in [
        json!({"prompt":["a","b"]}),
        json!({"prompt":[1,2]}),
        json!({"prompt":[[1],[2]]}),
        json!({"prompt":{}}),
        json!({"echo":true}),
        json!({"logprobs":0}),
        json!({"suffix":"end"}),
        json!({"n":2}),
        json!({"best_of":2}),
        json!({"logit_bias":{"123":1}}),
    ] {
        let mut body = json!({"model":MODEL,"prompt":"hello"});
        body.as_object_mut()
            .unwrap()
            .extend(patch.as_object().unwrap().clone());
        let req = crate::protocol::openai::normalize_legacy_completions(body).unwrap();
        let error =
            UpstreamClient::build_request_plan(&payload, &req, UPSTREAM_MODEL, false).unwrap_err();
        assert_eq!(
            error.code.as_deref(),
            Some("nous_legacy_completions_unsupported"),
            "{patch}"
        );
        payload.auth_mode = None;
        assert!(super::super::runtime::validate_request(&payload, &req).is_ok());
        payload.auth_mode = Some("nous_portal_oauth".into());
    }
}

#[tokio::test]
async fn nous_public_legacy_unsupported_request_never_sends_or_charges() {
    Box::pin(async {
        for target in TARGETS {
            let peer = peer::Peer::new("chat", false, false, 200).await;
            let fixture = Box::pin(Fixture::with_target(&peer, target)).await;
            super::super::test_support::origin(peer.origin.clone(), async {
                let response = fixture
                    .open("completions", json!({"model":MODEL,"prompt":["a","b"]}))
                    .await;
                let status = response.status();
                let text = drain(response.into_body()).await;
                assert_eq!(status, StatusCode::BAD_REQUEST, "{text}");
                assert!(
                    text.contains("nous_legacy_completions_unsupported"),
                    "{text}"
                );
                assert_ne!(fixture.audits(1).await[0].status, "completed");
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
