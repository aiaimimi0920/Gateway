//! Hosted Orchestration reuses the account's fixed model across native tool turns.
use super::*;
use crate::protocol::sap_aicore::wire_tests::orchestration_tools as tools;

#[test]
#[ignore = "Requires isolated Hosted PostgreSQL schema and Redis; retains scoped fixture data"]
fn sap_aicore_server_hosted_orchestration_v2_tools() {
    run_hosted(hosted);
}

async fn hosted() {
    let scope = format!("sap-orchestration-tools-{}", uuid::Uuid::new_v4().simple());
    let peer = peer::Peer::raw_sequence(
        [false, true]
            .into_iter()
            .flat_map(|stream| [true, false].map(|first| tools::reply(first, stream)))
            .collect(),
    )
    .await;
    let writer = build(&scope, "writer", &peer).await;
    let (account, credential) = provision(
        &writer,
        &scope,
        json!({"sap_orchestration_model":"gpt-4.1-mini"}),
        json!({"api_key":KEY,"extra_body":{"sap_orchestration_model":"credential-must-not-win"}}),
    )
    .await;
    let reader = build(&scope, "reader", &peer).await;
    let route = "/v1/chat/completions";
    test_support::origin(peer.origin.clone(), async {
        let mut initial = tools::request(false);
        initial["model"] = json!(scope);
        let (status, _) = call(&reader, route, "wrong-public-key", initial.clone()).await;
        assert_eq!(status, StatusCode::UNAUTHORIZED);
        let mut unsupported = initial.clone();
        unsupported["tool_choice"] = json!("required");
        let (status, text) = call(&reader, route, PUBLIC, unsupported).await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "{text}");
        assert!(text.contains("invalid_sap_aicore"));
        assert!(peer.calls.lock().unwrap().is_empty());
        assert!(peer.oauth.lock().unwrap().is_empty());

        for stream in [false, true] {
            initial["stream"] = json!(stream);
            let (status, first) = call(&reader, route, PUBLIC, initial.clone()).await;
            assert_eq!(status, StatusCode::OK, "{first}");
            let mut next = initial.clone();
            next["messages"].as_array_mut().unwrap().extend([
                json!({"role":"assistant","content":null,"tool_calls":[tools::call()]}),
                json!({"role":"tool","tool_call_id":"call_sap","content":tools::RESULT}),
            ]);
            let (status, second) = call(&reader, route, PUBLIC, next).await;
            assert_eq!(status, StatusCode::OK, "{second}");
            assert_public(&scope, stream, &first, &second);
        }
    })
    .await;

    assert_eq!(peer.oauth.lock().unwrap().len(), 1);
    {
        let sent = peer.calls.lock().unwrap();
        assert_eq!(sent.len(), 4);
        for (index, (path, headers, body)) in sent.iter().enumerate() {
            assert_eq!(path, "/v2/inference/deployments/deployment1/v2/completion");
            assert_eq!(headers["authorization"], "Bearer synthetic-sap-access");
            assert_eq!(headers["AI-Resource-Group"], "group-a");
            assert!(!headers.contains_key("cookie") && !headers.contains_key("x-management-token"));
            let templating = &body["config"]["modules"]["prompt_templating"];
            assert_eq!(templating["model"]["name"], "gpt-4.1-mini");
            assert_eq!(
                templating["prompt"]["tools"],
                tools::request(false)["tools"]
            );
            if index >= 2 {
                assert_eq!(headers["accept"], "text/event-stream");
                assert_eq!(body["config"]["stream"], json!({"enabled":true}));
            }
            if index % 2 == 1 {
                assert_eq!(body["placeholder_values"]["gateway_content"], tools::RESULT);
                assert_eq!(
                    templating["prompt"]["template"],
                    json!([
                        {"role":"tool","tool_call_id":"call_sap","content":"{{?gateway_content}}"}
                    ])
                );
            }
        }
    }
    assert_audits(&writer, &account, &credential, &scope, 4).await;
    finish(writer, reader, peer).await;
}

fn assert_public(alias: &str, stream: bool, first: &str, second: &str) {
    for text in [first, second] {
        assert!(!text.contains("hidden-module") && !text.contains("tenant-internal-model"));
    }
    if stream {
        let first_frames = tools::frames(first);
        let second_frames = tools::frames(second);
        for (text, frames) in [(first, &first_frames), (second, &second_frames)] {
            assert_eq!(text.matches("data: [DONE]").count(), 1);
            assert!(frames.iter().all(|frame| frame["model"] == alias));
            assert_eq!(
                frames
                    .iter()
                    .filter(|frame| frame["usage"]["total_tokens"] == 5)
                    .count(),
                1
            );
        }
        let arguments: String = first_frames
            .iter()
            .filter_map(|frame| {
                frame
                    .pointer("/choices/0/delta/tool_calls/0/function/arguments")?
                    .as_str()
            })
            .collect();
        assert_eq!(arguments, tools::ARGS);
        assert!(first_frames
            .iter()
            .any(|frame| frame["choices"][0]["finish_reason"] == "tool_calls"));
        let content: String = second_frames
            .iter()
            .filter_map(|frame| frame.pointer("/choices/0/delta/content")?.as_str())
            .collect();
        assert_eq!(content, "已收到结果");
    } else {
        let first: Value = serde_json::from_str(first).unwrap();
        let second: Value = serde_json::from_str(second).unwrap();
        for reply in [&first, &second] {
            assert_eq!(reply["model"], alias);
            assert_eq!(reply["usage"]["total_tokens"], 5);
        }
        assert_eq!(
            first["choices"][0]["message"]["tool_calls"],
            json!([tools::call()])
        );
        assert_eq!(second["choices"][0]["message"]["content"], "已收到结果");
    }
}
