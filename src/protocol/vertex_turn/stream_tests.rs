//! Conflicting state cannot produce successful completion in either public translator direction.
use super::*;
use crate::protocol::{
    anthropic, responses, sse_parse::format_sse_event, stream_error::ProtocolStreamError,
};
use bytes::Bytes;
use futures::{stream, StreamExt};

fn turn(text: &str) -> Value {
    json!({"source":"vertex_express_api","model":"model","parts":[{"text":text,"thought_signature":"YWJj"}]})
}
fn chunk(raw: Value) -> Result<Bytes, ProtocolStreamError> {
    Ok(Bytes::from(format_sse_event(None, &raw.to_string())))
}
fn chat(text: &str) -> Value {
    json!({"choices":[{"delta":{"content":text},"finish_reason":null}]})
}
fn terminal(text: &str) -> Value {
    json!({"choices":[{"delta":{},"finish_reason":"stop"}],"gateway_vertex_turn":turn(text)})
}
async fn forward(
    input: Vec<Result<Bytes, ProtocolStreamError>>,
    messages: bool,
) -> Vec<Result<Bytes, ProtocolStreamError>> {
    if messages {
        anthropic::translate_openai_sse_to_anthropic_with_error(stream::iter(input), "alias".into())
            .collect()
            .await
    } else {
        responses::translate_openai_sse_to_responses_with_error(stream::iter(input), "alias".into())
            .collect()
            .await
    }
}
fn failed(results: Vec<Result<Bytes, ProtocolStreamError>>) {
    assert!(results.iter().any(Result::is_err));
    let text = String::from_utf8(
        results
            .into_iter()
            .filter_map(Result::ok)
            .flat_map(|b| b.to_vec())
            .collect(),
    )
    .unwrap();
    for success in ["response.completed", "message_stop", "[DONE]"] {
        assert!(!text.contains(success), "{text}");
    }
}
#[tokio::test]
async fn vertex_express_signed_stream_projection_mismatch_aborts_both_forward_translators() {
    for messages in [false, true] {
        failed(
            forward(
                vec![
                    chunk(chat("A")),
                    chunk(terminal("B")),
                    chunk(
                        json!({"usage":{"prompt_tokens":1,"completion_tokens":1,"total_tokens":2}}),
                    ),
                ],
                messages,
            )
            .await,
        );
    }
}
#[tokio::test]
async fn vertex_express_signed_stream_late_content_without_extension_aborts() {
    for messages in [false, true] {
        failed(
            forward(
                vec![
                    chunk(chat("A")),
                    chunk(terminal("A")),
                    chunk(chat("B")),
                    chunk(
                        json!({"usage":{"prompt_tokens":1,"completion_tokens":1,"total_tokens":2}}),
                    ),
                ],
                messages,
            )
            .await,
        );
    }
}
#[tokio::test]
async fn vertex_express_signed_stream_reverse_snapshot_must_match_emitted_text() {
    let response = json!({"id":"r","model":"model","status":"completed","output":[{"type":"message","role":"assistant","content":[{"type":"output_text","text":"B"}]}],"gateway_vertex_turn":turn("B")});
    let input = vec![
        chunk(json!({"type":"response.output_text.delta","delta":"A"})),
        chunk(json!({"type":"response.completed","response":response})),
    ];
    failed(
        responses::translate_responses_sse_to_openai_chat_with_error(
            stream::iter(input),
            "alias".into(),
        )
        .collect()
        .await,
    );
}

#[tokio::test]
async fn vertex_express_signed_stream_valid_text_and_fragmented_tools_survive_both_directions() {
    let tool_turn = json!({"source":"vertex_express_api","model":"model","parts":[
        {"function_call":{"name":"lookup","args":{"s":"&amp;"}},"public_id":"c1","thought_signature":"YWJj"}]});
    for tool in [false, true] {
        let mut end = terminal("A");
        let mut input = vec![chunk(chat("A"))];
        if tool {
            end["choices"][0]["finish_reason"] = json!("tool_calls");
            end["gateway_vertex_turn"] = tool_turn.clone();
            input = vec![
                chunk(
                    json!({"choices":[{"delta":{"tool_calls":[{"index":0,"id":"c1","type":"function","function":{"name":"lookup","arguments":"{\"s\":"}}]}}]}),
                ),
                chunk(
                    json!({"choices":[{"delta":{"tool_calls":[{"index":0,"function":{"arguments":" \"&amp;\"}"}}]}}]}),
                ),
            ];
        }
        end["usage"] = json!({"prompt_tokens":1,"completion_tokens":2,"total_tokens":3});
        input.push(chunk(end));
        for messages in [false, true] {
            let copied = input
                .iter()
                .map(|v| Ok(v.as_ref().unwrap().clone()))
                .collect();
            let results = forward(copied, messages).await;
            assert!(results.iter().all(Result::is_ok));
            let bytes: Vec<_> = results
                .into_iter()
                .map(Result::unwrap)
                .flat_map(|b| b.to_vec())
                .collect();
            assert!(String::from_utf8(bytes.clone())
                .unwrap()
                .contains("gateway_vertex_turn"));
            if !messages {
                let reverse = responses::translate_responses_sse_to_openai_chat_with_error(
                    stream::iter(vec![Ok::<_, ProtocolStreamError>(Bytes::from(bytes))]),
                    "alias".into(),
                )
                .collect::<Vec<_>>()
                .await;
                assert!(reverse.iter().all(Result::is_ok));
                assert!(reverse
                    .iter()
                    .any(|v| String::from_utf8_lossy(v.as_ref().unwrap())
                        .contains("gateway_vertex_turn")));
            }
        }
    }
}

#[tokio::test]
async fn vertex_express_signed_stream_reverse_rejects_preterminal_or_wrong_level_state() {
    for raw in [
        json!({"type":"response.created","response":{"gateway_vertex_turn":turn("A")}}),
        json!({"type":"response.in_progress","gateway_vertex_turn":turn("A")}),
    ] {
        failed(
            responses::translate_responses_sse_to_openai_chat_with_error(
                stream::iter(vec![chunk(raw)]),
                "alias".into(),
            )
            .collect()
            .await,
        );
    }
}
