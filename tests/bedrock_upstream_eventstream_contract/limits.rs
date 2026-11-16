//! Retained tool/block budgets and complete JSON use the same native semantic owner.
use super::fixtures::{event, frame, response, sequence};
use neuro_gateway::{
    protocol::bedrock_converse::build_converse_success,
    upstream::bedrock_converse_official_api_common::{accumulate_stream, unpack_response},
};
use serde_json::json;

#[tokio::test]
async fn bedrock_upstream_block_and_tool_argument_budgets_are_enforced() {
    let mut frames = vec![sequence()[0].clone()];
    for index in 0..4097 {
        frames.push(event(
            "contentBlockDelta",
            json!({"contentBlockIndex":index,"delta":{"text":""}}),
        ));
        frames.push(event(
            "contentBlockStop",
            json!({"contentBlockIndex":index}),
        ));
    }
    assert!(accumulate_stream(response(frames), "fixture")
        .await
        .unwrap_err()
        .message
        .contains("block limit"));
    let mut frames = sequence();
    frames[4] = event(
        "contentBlockDelta",
        json!({"contentBlockIndex":1,"delta":{"toolUse":{"input":"x".repeat(4*1024*1024+1)}}}),
    );
    assert!(accumulate_stream(response(frames), "fixture")
        .await
        .unwrap_err()
        .message
        .contains("retained-byte limit"));
}

#[test]
fn bedrock_upstream_native_json_keeps_usage_and_rejects_unsupported_content() {
    let body = json!({"model":"fixture","output":{"message":{"role":"assistant","content":[{"text":"answer"},{"toolUse":{"toolUseId":"call-1","name":"f","input":{"n":1}}}]}},"stopReason":"tool_use","usage":{"inputTokens":1,"outputTokens":2,"totalTokens":3}});
    let result = unpack_response(&body).unwrap();
    assert_eq!(result.text, "answer");
    assert_eq!(result.model, "fixture");
    assert_eq!(result.tool_calls[0].arguments.as_deref(), Some("{\"n\":1}"));
    assert_eq!(result.usage.unwrap().total_tokens, 3);
    assert_eq!(result.finish_reason.as_deref(), Some("tool_calls"));
    for reason in [
        "malformed_tool_use",
        "malformed_model_output",
        "private-fixture-secret",
    ] {
        let mut changed = body.clone();
        changed["stopReason"] = json!(reason);
        let error = unpack_response(&changed).unwrap_err();
        assert!(!error.message.contains("private-fixture-secret"));
    }
    let mut changed = body;
    changed["output"]["message"]["content"] = json!([{"reasoningContent":{"text":"private-fixture-secret","signature":"private-fixture-secret"}}]);
    assert!(unpack_response(&changed).is_err());
    let empty = build_converse_success("fixture", "", None, &[], Some("stop"));
    assert!(unpack_response(&empty).unwrap().usage.is_none());
}

#[tokio::test]
async fn bedrock_upstream_client_does_not_enforce_service_only_header_size_restrictions() {
    let padding = "x".repeat(60000);
    let first = frame(
        &[
            (":message-type", "event"),
            (":event-type", "messageStart"),
            ("x-one", &padding),
            ("x-two", &padding),
            ("x-three", &padding),
        ],
        &json!({"role":"assistant"}),
    );
    let mut frames = sequence();
    frames[0] = first;
    assert_eq!(
        accumulate_stream(response(frames), "fixture")
            .await
            .unwrap()
            .text,
        "你好"
    );
}

#[tokio::test]
async fn bedrock_upstream_client_does_not_enforce_service_only_payload_size_restrictions() {
    // Smithy clients MUST NOT enforce the service-only 24 MiB limit. Gateway still has a 64 MiB byte budget.
    let mut frames = sequence();
    frames[0] = event(
        "messageStart",
        json!({"role":"assistant","ignoredOptionalField":"x".repeat(24*1024*1024)}),
    );
    assert_eq!(
        accumulate_stream(response(frames), "fixture")
            .await
            .unwrap()
            .text,
        "你好"
    );
}

#[tokio::test]
async fn bedrock_upstream_gateway_header_count_budget_limits_decoded_map_overhead() {
    let mut fields = vec![
        (":message-type".to_owned(), "event".to_owned()),
        (":event-type".to_owned(), "messageStart".to_owned()),
    ];
    fields.extend((0..4096).map(|n| (format!("x-fixture-{n}"), "v".to_owned())));
    let fields: Vec<_> = fields
        .iter()
        .map(|(name, value)| (name.as_str(), value.as_str()))
        .collect();
    let error = accumulate_stream(
        response(vec![frame(&fields, &json!({"role":"assistant"}))]),
        "fixture",
    )
    .await
    .unwrap_err();
    assert!(error.message.contains("header count budget"));
}
