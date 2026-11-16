//! Three rounds traverse both real sockets and preserve opaque native order across JSON/SSE.
use super::{
    outbound, requests, run,
    runtime::Fixture,
    upstream::{Mode, DEADLINE},
    wire,
};
use neuro_gateway::upstream::bedrock_converse_official_api_common::accumulate_stream;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use tokio::time::timeout;

async fn verify(protocol: u8, starts_streaming: bool) {
    let fixture = Fixture::start(Mode::Complete).await;
    let mut turns = Vec::new();
    for round in 0..3 {
        let stream = if round == 2 {
            !starts_streaming
        } else {
            starts_streaming
        };
        let response = fixture
            .post(
                requests::path(protocol, stream),
                requests::request(protocol, &turns, stream),
            )
            .await;
        assert_eq!(response.status().as_u16(), 200);
        let content_type = response.headers()["content-type"]
            .to_str()
            .unwrap()
            .to_owned();
        assert!(content_type.starts_with(if stream {
            if protocol == 3 {
                "application/vnd.amazon.eventstream"
            } else {
                "text/event-stream"
            }
        } else {
            "application/json"
        }));
        let bytes = timeout(DEADLINE, response.bytes()).await.unwrap().unwrap();
        let turn = if protocol < 3 {
            outbound::public(protocol, &bytes, stream)
        } else if stream {
            assert_eq!(
                outbound::native(&bytes),
                outbound::native(&super::fixture::sequence().concat())
            );
            let response = accumulate_stream(wire::response(vec![bytes.to_vec()]), "fixture-model")
                .await
                .unwrap();
            json!(response.bedrock_turn.unwrap())
        } else {
            let value: Value = serde_json::from_slice(&bytes).unwrap();
            assert_eq!(value["usage"]["totalTokens"], 7);
            assert_eq!(value["stopReason"], "tool_use");
            json!({"content":value["output"]["message"]["content"]})
        };
        assert_eq!(turn["content"], super::fixture::content());
        let calls = fixture.upstream.calls.lock().unwrap();
        assert_eq!(calls.len(), round + 1);
        let (path, body) = &calls[round];
        // Production intentionally accumulates ConverseStream even for public JSON callers.
        assert_eq!(path, "/model/fixture-model/converse-stream");
        assert_eq!(body["system"], json!([{"text":"policy-secret"}]));
        assert_eq!(body["messages"].as_array().unwrap().len(), 2 * round + 1);
        assert_eq!(body["toolConfig"]["tools"][0]["toolSpec"]["name"], "lookup");
        for index in 0..round {
            assert_eq!(
                body["messages"][index * 2 + 1]["content"],
                super::fixture::content()
            );
            assert_eq!(
                body["messages"][index * 2 + 2]["content"][0]["toolResult"],
                json!({"toolUseId":"call-a","content":[{"text":"done"}]})
            );
        }
        if protocol < 3 {
            assert_eq!(turn["continuation"]["source"], "aws_bedrock_converse");
            assert_eq!(turn["continuation"]["model"], "fixture-model");
            let prefix = json!({"messages":body["messages"],"system":body.get("system")});
            assert_eq!(
                turn["continuation"]["request_sha256"],
                format!("{:x}", Sha256::digest(serde_json::to_vec(&prefix).unwrap()))
            );
        }
        drop(calls);
        let rows = fixture.settled(round + 1, "completed").await;
        for row in rows {
            assert_eq!(
                (row.prompt_tokens, row.completion_tokens, row.total_tokens),
                (Some(3), Some(4), Some(7))
            );
        }
        turns.push(turn);
    }
    if protocol < 3 {
        let mut bad = requests::request(protocol, &turns, true);
        let key = if protocol == 2 { "input" } else { "messages" };
        bad[key][0]["content"] = json!("changed old user");
        assert_eq!(
            fixture
                .post(requests::path(protocol, true), bad)
                .await
                .status()
                .as_u16(),
            400
        );
        assert_eq!(fixture.upstream.calls.lock().unwrap().len(), 3);
        assert_eq!(fixture.balance().await, 7);
        assert_eq!(
            fixture.successes(),
            3,
            "local invalid history must not reset provider health"
        );
    }
    fixture.close().await;
}
macro_rules! success {
    ($name:ident, $protocol:expr, $stream:expr) => {
        #[test]
        fn $name() {
            run(|| verify($protocol, $stream));
        }
    };
}
success!(
    chat_json_tcp_tools_signed_history_and_local_settlement,
    0,
    false
);
success!(
    chat_sse_tcp_tools_signed_history_and_local_settlement,
    0,
    true
);
success!(
    messages_json_tcp_tools_signed_history_and_local_settlement,
    1,
    false
);
success!(
    messages_sse_tcp_tools_signed_history_and_local_settlement,
    1,
    true
);
success!(
    responses_json_tcp_tools_signed_history_and_local_settlement,
    2,
    false
);
success!(
    responses_sse_tcp_tools_signed_history_and_local_settlement,
    2,
    true
);
success!(
    native_json_tcp_ordered_tools_signed_history_and_local_settlement,
    3,
    false
);
success!(
    native_eventstream_tcp_ordered_tools_signed_history_and_local_settlement,
    3,
    true
);
