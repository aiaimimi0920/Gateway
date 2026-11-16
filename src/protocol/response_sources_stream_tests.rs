//! Public Chat/Responses stream conversion must preserve the final source list.
use super::response_sources_tests::{chat, sources};
use super::responses::{
    accumulate_responses_sse_stream_with_error, translate_openai_sse_to_responses_with_error,
    translate_responses_sse_to_openai_chat_with_error,
};
use super::sse_parse::format_sse_event;
use super::stream_error::ProtocolStreamError;
use bytes::Bytes;
use futures::{stream, StreamExt};
use serde_json::{json, Value};

fn chunks() -> Vec<Result<Bytes, ProtocolStreamError>> {
    let text = json!({"model":"unknown", "choices":[{"delta":{"content":"答案 [1]\n未改写。"},
        "finish_reason":null}]});
    let stop = json!({"model":"unknown", "choices":[{"delta":{}, "finish_reason":"stop"}],
        "usage":null, "gateway_sources":sources()});
    [text.to_string(), stop.to_string(), "[DONE]".into()]
        .into_iter()
        .map(|data| Ok(Bytes::from(format_sse_event(None, &data))))
        .collect()
}

fn payloads(bytes: &[u8]) -> Vec<Value> {
    String::from_utf8(bytes.to_vec())
        .unwrap()
        .lines()
        .filter_map(|line| line.strip_prefix("data: "))
        .filter_map(|data| serde_json::from_str(data).ok())
        .collect()
}

#[tokio::test]
async fn response_sources_chat_to_responses_terminal_preserves_sources_and_unknown_usage() {
    let output =
        translate_openai_sse_to_responses_with_error(stream::iter(chunks()), "alias".into());
    let bytes: Vec<u8> = output
        .collect::<Vec<_>>()
        .await
        .into_iter()
        .flat_map(|result| result.unwrap().to_vec())
        .collect();
    let events = payloads(&bytes);
    let final_event = events
        .iter()
        .find(|event| event["type"] == "response.completed")
        .unwrap();
    assert_eq!(final_event["response"]["gateway_sources"], sources());
    assert!(final_event["response"].get("usage").is_none());
    assert_eq!(
        events
            .iter()
            .filter(|event| event["response"].get("gateway_sources").is_some())
            .count(),
        1
    );
    assert!(events
        .iter()
        .all(|event| !event.to_string().contains("annotations")));
    let input = stream::iter(vec![Ok::<_, ProtocolStreamError>(Bytes::from(bytes))]);
    let canonical = accumulate_responses_sse_stream_with_error(Box::pin(input), "fallback")
        .await
        .unwrap();
    assert_eq!(canonical.text, "答案 [1]\n未改写。");
    assert!(canonical.usage.is_none());
    assert_eq!(
        serde_json::to_value(canonical).unwrap()["sources"],
        sources()
    );
}

#[tokio::test]
async fn response_sources_responses_back_to_chat_has_one_final_list() {
    let response_stream =
        translate_openai_sse_to_responses_with_error(stream::iter(chunks()), "alias".into());
    let chat_stream =
        translate_responses_sse_to_openai_chat_with_error(response_stream, "alias".into());
    let bytes: Vec<u8> = chat_stream
        .collect::<Vec<_>>()
        .await
        .into_iter()
        .flat_map(|result| result.unwrap().to_vec())
        .collect();
    let events = payloads(&bytes);
    let source_frames: Vec<_> = events
        .iter()
        .filter(|event| event.get("gateway_sources").is_some())
        .collect();
    assert_eq!(source_frames.len(), 1);
    assert_eq!(source_frames[0]["gateway_sources"], sources());
    assert_eq!(source_frames[0]["choices"][0]["finish_reason"], "stop");
    assert!(source_frames[0].get("usage").is_none());
    let text: String = events
        .iter()
        .filter_map(|event| event["choices"][0]["delta"]["content"].as_str())
        .collect();
    assert_eq!(
        text,
        chat()["choices"][0]["message"]["content"].as_str().unwrap()
    );
    assert!(String::from_utf8(bytes)
        .unwrap()
        .ends_with("data: [DONE]\n\n"));
}

#[tokio::test]
async fn response_sources_invalid_stream_extension_aborts_without_completed_event() {
    let mut input = chunks();
    let invalid = json!({"choices":[{"delta":{}, "finish_reason":"stop"}],
        "gateway_sources":[{"url":"file:///private", "name":"", "snippet":""}]});
    input[1] = Ok(Bytes::from(format_sse_event(None, &invalid.to_string())));
    let results = translate_openai_sse_to_responses_with_error(stream::iter(input), "alias".into())
        .collect::<Vec<_>>()
        .await;
    assert!(results.iter().any(Result::is_err));
    let bytes: Vec<_> = results
        .into_iter()
        .filter_map(Result::ok)
        .flat_map(|chunk| chunk.to_vec())
        .collect();
    assert!(payloads(&bytes).iter().all(|event| !matches!(
        event["type"].as_str(),
        Some("response.completed" | "response.incomplete")
    )));
}

#[tokio::test]
async fn response_sources_invalid_reverse_stream_extension_is_not_swallowed() {
    let mut body = super::response_sources_tests::response();
    body["gateway_sources"] = json!([{"url":"file:///private", "name":"", "snippet":""}]);
    let event = json!({"type":"response.completed", "response":body});
    let input = stream::iter(vec![Ok::<_, ProtocolStreamError>(Bytes::from(
        format_sse_event(Some("response.completed"), &event.to_string()),
    ))]);
    let results = translate_responses_sse_to_openai_chat_with_error(input, "alias".into())
        .collect::<Vec<_>>()
        .await;
    assert!(results.iter().any(Result::is_err));
    let bytes: Vec<_> = results
        .into_iter()
        .filter_map(Result::ok)
        .flat_map(|chunk| chunk.to_vec())
        .collect();
    assert!(!String::from_utf8(bytes.clone()).unwrap().contains("[DONE]"));
    assert!(payloads(&bytes)
        .iter()
        .all(|chunk| chunk["choices"][0]["finish_reason"].is_null()));
}

#[tokio::test]
async fn response_sources_preterminal_duplicate_or_nonterminal_list_aborts() {
    let nonterminal =
        json!({"choices":[{"delta":{}, "finish_reason":null}], "gateway_sources":sources()});
    let empty_reason =
        json!({"choices":[{"delta":{}, "finish_reason":" "}], "gateway_sources":sources()});
    for extra in [
        nonterminal,
        empty_reason,
        json!({"choices":[{"delta":{}, "finish_reason":"stop"}],
        "gateway_sources":sources()}),
    ] {
        let mut input = chunks();
        input.insert(
            2,
            Ok(Bytes::from(format_sse_event(None, &extra.to_string()))),
        );
        let results =
            translate_openai_sse_to_responses_with_error(stream::iter(input), "alias".into())
                .collect::<Vec<_>>()
                .await;
        assert!(results.iter().any(Result::is_err));
        let bytes: Vec<_> = results
            .into_iter()
            .filter_map(Result::ok)
            .flat_map(|chunk| chunk.to_vec())
            .collect();
        assert!(payloads(&bytes).iter().all(|event| !matches!(
            event["type"].as_str(),
            Some("response.completed" | "response.incomplete")
        )));
    }
}

#[tokio::test]
async fn response_sources_known_usage_terminal_keeps_original_drain_semantics() {
    let mut input = chunks();
    let terminal = json!({"choices":[{"delta":{}, "finish_reason":"stop"}],
        "gateway_sources":sources(), "usage":{"prompt_tokens":1, "completion_tokens":2, "total_tokens":3}});
    input[1] = Ok(Bytes::from(format_sse_event(None, &terminal.to_string())));
    let trailing = json!({"choices":[{"delta":{}, "finish_reason":"stop"}], "gateway_sources":"ignored-after-terminal"});
    input.insert(
        2,
        Ok(Bytes::from(format_sse_event(None, &trailing.to_string()))),
    );
    let results = translate_openai_sse_to_responses_with_error(stream::iter(input), "alias".into())
        .collect::<Vec<_>>()
        .await;
    let bytes: Vec<_> = results
        .into_iter()
        .flat_map(|chunk| chunk.unwrap().to_vec())
        .collect();
    let events = payloads(&bytes);
    let finals: Vec<_> = events
        .iter()
        .filter(|event| event["type"] == "response.completed")
        .collect();
    assert_eq!(finals.len(), 1);
    assert_eq!(finals[0]["response"]["gateway_sources"], sources());
    assert_eq!(finals[0]["response"]["usage"]["total_tokens"], 3);
}

#[tokio::test]
async fn response_sources_usage_null_is_unknown_without_changing_object_semantics() {
    for (usage, total) in [
        (Value::Null, None),
        (json!({}), Some(0)),
        (json!({"input_tokens":3, "output_tokens":4}), Some(7)),
    ] {
        let mut input = chunks();
        let stop = json!({"choices":[{"delta":{}, "finish_reason":"stop"}], "usage":usage});
        input[1] = Ok(Bytes::from(format_sse_event(None, &stop.to_string())));
        let results =
            translate_openai_sse_to_responses_with_error(stream::iter(input), "alias".into())
                .collect::<Vec<_>>()
                .await;
        let bytes: Vec<_> = results
            .into_iter()
            .flat_map(|chunk| chunk.unwrap().to_vec())
            .collect();
        let events = payloads(&bytes);
        let final_body = &events
            .iter()
            .find(|event| event["type"] == "response.completed")
            .unwrap()["response"];
        assert_eq!(final_body["usage"]["total_tokens"].as_u64(), total);
        assert!(final_body.get("gateway_sources").is_none());
    }
}

#[tokio::test]
async fn response_sources_native_responses_passthrough_does_not_repack() {
    let event =
        json!({"type":"response.completed", "response":super::response_sources_tests::response()});
    let wire = format_sse_event(Some("response.completed"), &event.to_string());
    let input = stream::iter(vec![Ok::<_, ProtocolStreamError>(Bytes::from(
        wire.clone(),
    ))]);
    let bytes: Vec<_> =
        translate_openai_sse_to_responses_with_error(input, "different-model".into())
            .collect::<Vec<_>>()
            .await
            .into_iter()
            .flat_map(|result| result.unwrap().to_vec())
            .collect();
    assert_eq!(bytes, wire.as_bytes());
}

#[tokio::test]
async fn response_sources_frozen_answer_rejects_later_content_or_tool_delta() {
    for delta in [
        json!({"content":"must-not-append"}),
        json!({"tool_calls":[
            {"index":0, "function":{"name":"must-not-add", "arguments":"{}"}}
        ]}),
    ] {
        let mut input = chunks();
        let changed = json!({"choices":[{"delta":delta, "finish_reason":null}]});
        input.insert(
            2,
            Ok(Bytes::from(format_sse_event(None, &changed.to_string()))),
        );
        let results =
            translate_openai_sse_to_responses_with_error(stream::iter(input), "alias".into())
                .collect::<Vec<_>>()
                .await;
        assert!(results.iter().any(Result::is_err));
        let bytes: Vec<_> = results
            .into_iter()
            .filter_map(Result::ok)
            .flat_map(|part| part.to_vec())
            .collect();
        assert!(!String::from_utf8_lossy(&bytes).contains("must-not-"));
        assert!(payloads(&bytes).iter().all(|event| !matches!(
            event["type"].as_str(),
            Some("response.completed" | "response.incomplete")
        )));
    }
}

#[tokio::test]
async fn response_sources_finish_frame_delta_and_usage_only_tail_are_supported() {
    let mut input = chunks();
    let finish = json!({"choices":[{"delta":{"content":"最后一段"}, "finish_reason":"stop"}],
        "gateway_sources":sources(), "usage":null});
    input[1] = Ok(Bytes::from(format_sse_event(None, &finish.to_string())));
    let usage =
        json!({"choices":[], "usage":{"prompt_tokens":1, "completion_tokens":2, "total_tokens":3}});
    input.insert(
        2,
        Ok(Bytes::from(format_sse_event(None, &usage.to_string()))),
    );
    let output = translate_openai_sse_to_responses_with_error(stream::iter(input), "alias".into());
    let result = accumulate_responses_sse_stream_with_error(Box::pin(output), "fallback")
        .await
        .unwrap();
    assert_eq!(result.text, "答案 [1]\n未改写。最后一段");
    assert_eq!(result.usage.as_ref().unwrap().total_tokens, 3);
    assert_eq!(serde_json::to_value(result).unwrap()["sources"], sources());
}
