//! Inbound AWS EventStream, distinct from OpenAI-to-Bedrock output contracts.
#![cfg(feature = "family-bedrock-converse-official-api")]

#[path = "bedrock_upstream_eventstream_contract/adversarial.rs"]
mod adversarial;
#[path = "bedrock_upstream_eventstream_contract/fixtures.rs"]
mod fixtures;
#[path = "bedrock_upstream_eventstream_contract/limits.rs"]
mod limits;
#[path = "bedrock_upstream_eventstream_contract/stream.rs"]
mod stream;

use fixtures::{event, frame, response, sequence};
use neuro_gateway::upstream::bedrock_converse_official_api_common::accumulate_stream;
use serde_json::json;

#[tokio::test]
async fn bedrock_upstream_real_headers_preserve_text_tools_and_post_stop_usage() {
    let result = accumulate_stream(response(sequence()), "fixture-model")
        .await
        .unwrap();
    assert_eq!(result.text, "你好");
    assert_eq!(result.model, "fixture-model");
    assert_eq!(result.finish_reason.as_deref(), Some("tool_calls"));
    assert_eq!(result.tool_calls.len(), 1);
    assert_eq!(result.tool_calls[0].id.as_deref(), Some("call-1"));
    assert_eq!(result.tool_calls[0].name.as_deref(), Some("lookup"));
    assert_eq!(
        result.tool_calls[0].arguments.as_deref(),
        Some("{\"city\": \"北京\"}")
    );
    let usage = result.usage.unwrap();
    assert_eq!(
        (
            usage.prompt_tokens,
            usage.completion_tokens,
            usage.total_tokens
        ),
        (5, 8, 13)
    );
    assert_eq!(usage.cache_read_input_tokens, Some(2));
    assert_eq!(usage.cache_creation_input_tokens, Some(1));
}

#[tokio::test]
async fn bedrock_upstream_crc_corruption_is_not_an_empty_success() {
    for prelude in [true, false] {
        let mut frames = sequence();
        let position = if prelude { 8 } else { frames[1].len() - 1 };
        frames[1][position] ^= 1;
        assert!(accumulate_stream(response(frames), "fixture")
            .await
            .is_err());
    }
}

#[tokio::test]
async fn bedrock_upstream_eof_requires_stop_metadata_and_a_complete_frame() {
    let frames = sequence();
    for mut incomplete in [frames[..2].to_vec(), frames[..8].to_vec(), frames.clone()] {
        if incomplete.len() == frames.len() {
            incomplete.last_mut().unwrap().pop();
        }
        assert!(accumulate_stream(response(incomplete), "fixture")
            .await
            .is_err());
    }
}

#[tokio::test]
async fn bedrock_upstream_modeled_and_unmodeled_failures_never_return_partial_success() {
    for kind in [
        "internalServerException",
        "modelStreamErrorException",
        "validationException",
        "throttlingException",
        "serviceUnavailableException",
        "unknownFutureException",
    ] {
        let mut frames = sequence()[..2].to_vec();
        frames.push(frame(
            &[(":message-type", "exception"), (":exception-type", kind)],
            &json!({"message":"private-fixture-secret"}),
        ));
        let error = accumulate_stream(response(frames), "fixture")
            .await
            .unwrap_err();
        assert!(!error.message.contains("private-fixture-secret"));
    }
    let error = accumulate_stream(
        response(vec![frame(
            &[
                (":message-type", "error"),
                (":error-code", "InternalError"),
                (":error-message", "private-fixture-secret"),
            ],
            &json!({}),
        )]),
        "fixture",
    )
    .await
    .unwrap_err();
    assert!(!error.message.contains("private-fixture-secret"));
}

#[tokio::test]
async fn bedrock_upstream_unknown_delta_and_malformed_stop_are_not_success() {
    let mut frames = vec![
        event("messageStart", json!({"role":"assistant"})),
        event(
            "contentBlockDelta",
            json!({"contentBlockIndex":0,"delta":{"futurePrivateField":"private-fixture-secret"}}),
        ),
    ];
    assert!(accumulate_stream(response(frames.clone()), "fixture")
        .await
        .is_err());
    frames = sequence();
    frames[7] = event("messageStop", json!({"stopReason":"malformed_tool_use"}));
    assert!(accumulate_stream(response(frames), "fixture")
        .await
        .is_err());
}
