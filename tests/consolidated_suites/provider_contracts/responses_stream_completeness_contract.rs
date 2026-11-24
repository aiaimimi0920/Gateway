//! Protocol completion is independent of a clean HTTP EOF; fixtures never call providers.
use bytes::Bytes;
use futures::{stream, StreamExt};
use neuro_gateway::protocol::responses::{
    accumulate_responses_sse_stream_with_error, require_responses_completion,
    translate_responses_sse_to_openai_chat_with_error,
};
use neuro_gateway::protocol::stream_error::StreamError;
use neuro_gateway::upstream::stream::{StreamTermination, TrackedStream};
use std::sync::{Arc, Mutex};

type Error = StreamError<std::io::Error>;
const PARTIAL: &str = "event: response.output_text.delta\ndata: {\"type\":\"response.output_text.delta\",\"delta\":\"partial\"}\n\n";
const COMPLETE: &str = "event: response.completed\ndata: {\"type\":\"response.completed\",\"response\":{\"status\":\"completed\",\"output\":[],\"usage\":{\"input_tokens\":2,\"output_tokens\":3,\"total_tokens\":5}}}\n\n";

fn source(wire: &str) -> impl futures::Stream<Item = Result<Bytes, Error>> + Send + 'static {
    stream::iter(vec![Ok(Bytes::copy_from_slice(wire.as_bytes()))])
}

#[tokio::test]
async fn translated_clean_eof_without_response_terminal_is_not_success() {
    for suffix in ["", "data: [DONE]\n\n"] {
        let wire = format!("{PARTIAL}{suffix}");
        let chunks =
            translate_responses_sse_to_openai_chat_with_error(source(&wire), "test".into())
                .collect::<Vec<_>>()
                .await;
        let output = chunks
            .iter()
            .filter_map(|chunk| chunk.as_ref().ok())
            .flat_map(|chunk| chunk.iter().copied())
            .collect::<Vec<_>>();
        let output = String::from_utf8(output).unwrap();
        assert_eq!(
            chunks.iter().filter(|chunk| chunk.is_err()).count(),
            1,
            "missing terminal was accepted: {output}"
        );
        assert!(!output.contains("[DONE]"));
        assert!(!output.contains("\"finish_reason\":\"stop\""));
    }
}

#[tokio::test]
async fn buffered_partial_text_without_response_terminal_is_not_success() {
    for suffix in ["", "data: [DONE]\n\n"] {
        let wire = format!("{PARTIAL}{suffix}");
        let error = accumulate_responses_sse_stream_with_error(Box::pin(source(&wire)), "test")
            .await
            .expect_err("partial text must not become a successful JSON response");
        assert_eq!(error.code.as_deref(), Some("responses_stream_incomplete"));
    }
}

#[tokio::test]
async fn tool_item_done_is_not_a_response_terminal() {
    let wire = "event: response.output_item.done\ndata: {\"type\":\"response.output_item.done\",\"output_index\":0,\"item\":{\"type\":\"function_call\",\"id\":\"f1\",\"call_id\":\"c1\",\"name\":\"weather\",\"arguments\":\"{}\"}}\n\n";
    assert!(
        accumulate_responses_sse_stream_with_error(Box::pin(source(wire)), "test")
            .await
            .is_err()
    );
    let chunks = translate_responses_sse_to_openai_chat_with_error(source(wire), "test".into())
        .collect::<Vec<_>>()
        .await;
    assert!(chunks.iter().any(Result::is_err));
}

#[tokio::test]
async fn valid_completion_survives_every_byte_split_and_preserves_usage() {
    let wire = format!("{PARTIAL}{COMPLETE}").replace('\n', "\r\n");
    for split in 0..=wire.len() {
        let chunks: Vec<Result<Bytes, Error>> = vec![
            Ok(Bytes::copy_from_slice(&wire.as_bytes()[..split])),
            Ok(Bytes::copy_from_slice(&wire.as_bytes()[split..])),
        ];
        let result =
            accumulate_responses_sse_stream_with_error(Box::pin(stream::iter(chunks)), "test")
                .await
                .unwrap();
        assert_eq!(result.text, "partial");
        assert_eq!(result.usage.unwrap().total_tokens, 5);
    }
}

#[tokio::test]
async fn incomplete_is_a_valid_terminal_not_a_network_truncation() {
    let wire = format!(
        "{PARTIAL}{}",
        COMPLETE
            .replace("response.completed", "response.incomplete")
            .replace("\"completed\"", "\"incomplete\"")
    );
    let chunks = translate_responses_sse_to_openai_chat_with_error(source(&wire), "test".into())
        .collect::<Vec<_>>()
        .await;
    assert!(chunks.iter().all(Result::is_ok));
    let output = chunks
        .into_iter()
        .flat_map(|chunk| chunk.unwrap().to_vec())
        .collect::<Vec<_>>();
    let output = String::from_utf8(output).unwrap();
    assert!(output.contains("\"finish_reason\":\"length\""));
    assert!(output.contains("\"total_tokens\":5"));
    assert!(output.contains("[DONE]"));
}

#[tokio::test]
async fn malformed_or_conflicting_terminal_does_not_complete() {
    for terminal in [
        "event: response.completed\ndata: {\"type\":\"response.completed\"}\n\n",
        "event: response.completed\ndata: {\"type\":\"response.completed\",\"response\":{\"status\":\"failed\"}}\n\n",
        "event: response.completed\ndata: {\"type\":\"response.failed\",\"response\":{\"status\":\"completed\"}}\n\n",
        "event: response.completed\ndata: {broken-json}\n\n",
    ] {
        let wire = format!("{PARTIAL}{terminal}");
        assert!(accumulate_responses_sse_stream_with_error(Box::pin(source(&wire)), "test").await.is_err());
        let chunks = translate_responses_sse_to_openai_chat_with_error(source(&wire), "test".into())
            .collect::<Vec<_>>().await;
        assert_eq!(chunks.iter().filter(|chunk| chunk.is_err()).count(), 1);
    }
}

#[tokio::test]
async fn explicit_failure_is_typed_sanitized_and_never_reports_success() {
    for event in ["error", "response.failed", "response.cancelled"] {
        let wire = format!("{PARTIAL}event: {event}\ndata: {{\"type\":\"{event}\",\"message\":\"private-token-marker\"}}\n\n");
        let translated =
            translate_responses_sse_to_openai_chat_with_error(source(&wire), "test".into());
        let observed = Arc::new(Mutex::new(Vec::new()));
        let capture = Arc::clone(&observed);
        let chunks = TrackedStream::new_with_error(translated, move |metrics, success| {
            capture.lock().unwrap().push((metrics.termination, success));
        })
        .collect::<Vec<_>>()
        .await;
        let errors: Vec<_> = chunks
            .iter()
            .filter_map(|chunk| chunk.as_ref().err())
            .collect();
        assert_eq!(errors.len(), 1);
        assert!(matches!(errors[0], Error::Protocol(_)));
        assert!(!errors[0].to_string().contains("private-token-marker"));
        assert_eq!(
            &*observed.lock().unwrap(),
            &[(Some(StreamTermination::Interrupted), false)]
        );
    }
}

#[tokio::test]
async fn legacy_tool_status_requires_actual_tool_output_and_preserves_finish_reason() {
    for (output, accepted) in [
        (
            serde_json::json!([{"type":"function_call","call_id":"c1","name":"weather","arguments":"{}"}]),
            true,
        ),
        (serde_json::json!([]), false),
    ] {
        let payload = serde_json::json!({"type":"response.completed",
            "response":{"status":"tool_calls","output":output}});
        let wire = format!("event: response.completed\ndata: {payload}\n\n");
        let native = require_responses_completion(source(&wire))
            .collect::<Vec<_>>()
            .await;
        assert_eq!(native.iter().all(Result::is_ok), accepted);
        let buffered =
            accumulate_responses_sse_stream_with_error(Box::pin(source(&wire)), "test").await;
        assert_eq!(buffered.is_ok(), accepted);
        let translated =
            translate_responses_sse_to_openai_chat_with_error(source(&wire), "test".into())
                .collect::<Vec<_>>()
                .await;
        assert_eq!(translated.iter().all(Result::is_ok), accepted);
        if accepted {
            assert_eq!(
                buffered.unwrap().finish_reason.as_deref(),
                Some("tool_calls")
            );
            let bytes: Vec<_> = translated
                .into_iter()
                .flat_map(|item| item.unwrap().to_vec())
                .collect();
            assert!(String::from_utf8_lossy(&bytes).contains("\"finish_reason\":\"tool_calls\""));
        }
    }
}
