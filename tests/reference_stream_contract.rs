//! Reference-inspired stream contracts, using synthetic bytes only.
use bytes::Bytes;
use futures::{stream, Stream, StreamExt};
use neuro_gateway::protocol::openai::translate_openai_chat_sse_to_legacy_completions_with_error;
use neuro_gateway::protocol::responses::require_responses_completion;
use neuro_gateway::protocol::stream_error::StreamError;
use neuro_gateway::upstream::stream::{
    snapshot_tapped_completion_semantics, snapshot_tapped_usage,
    tap_sse_completion_semantics_with_error, tap_sse_usage_with_error,
};
use std::pin::Pin;
use std::sync::{
    atomic::{AtomicUsize, Ordering},
    Arc,
};
use std::task::{Context, Poll};

type Error = StreamError<std::io::Error>;
const DELTA: &str =
    "data: {\"choices\":[{\"delta\":{\"content\":\"partial\"},\"finish_reason\":null}]}\n\n";
const FINISH: &str = "data: {\"choices\":[{\"delta\":{},\"finish_reason\":\"length\"}]}\n\n";
const USAGE: &str = "data: {\"choices\":[],\"usage\":{\"prompt_tokens\":2,\"completion_tokens\":3,\"total_tokens\":5}}\n\n";

fn source(wire: &str) -> impl Stream<Item = Result<Bytes, Error>> + Send + 'static {
    stream::iter(vec![Ok(Bytes::copy_from_slice(wire.as_bytes()))])
}

fn output_text(chunks: &[Result<Bytes, Error>]) -> String {
    String::from_utf8(
        chunks
            .iter()
            .filter_map(|chunk| chunk.as_ref().ok())
            .flat_map(|chunk| chunk.iter().copied())
            .collect(),
    )
    .unwrap()
}

async fn translate(wire: &str) -> Vec<Result<Bytes, Error>> {
    translate_openai_chat_sse_to_legacy_completions_with_error(source(wire), "test".into())
        .collect()
        .await
}

#[tokio::test]
async fn clean_eof_without_finish_does_not_invent_success() {
    for wire in [
        String::new(),
        DELTA.into(),
        format!("{DELTA}{USAGE}"),
        "data: {\"choices\":[{\"delta\":{},\"finish_reason\":\" \"}]}\n\n".into(),
    ] {
        let chunks = translate(&wire).await;
        assert_eq!(
            chunks.iter().filter(|chunk| chunk.is_err()).count(),
            1,
            "{wire}"
        );
        let error = chunks.last().unwrap().as_ref().unwrap_err();
        assert!(matches!(error, Error::Protocol(_)));
        assert!(error.to_string().contains("openai_chat_stream_incomplete"));
        let text = output_text(&chunks);
        assert!(!text.contains("[DONE]"));
        assert!(!text.contains("\"finish_reason\":\"stop\""));
        if wire.contains("partial") {
            assert!(text.contains("partial"));
        }
    }
}

#[tokio::test]
async fn finish_without_done_is_valid_and_retains_later_usage() {
    for suffix in [FINISH.to_string(), format!("{FINISH}{USAGE}")] {
        let chunks = translate(&format!("{DELTA}{suffix}")).await;
        assert!(chunks.iter().all(Result::is_ok));
        let text = output_text(&chunks);
        assert_eq!(text.matches("[DONE]").count(), 1);
        assert!(text.contains("\"finish_reason\":\"length\""));
        if suffix.contains("usage") {
            assert!(text.contains("\"total_tokens\":5"));
        }
    }
}

#[tokio::test]
async fn native_legacy_eof_also_requires_a_terminal_signal() {
    let native = "data: {\"object\":\"text_completion.chunk\",\"choices\":[{\"text\":\"partial\",\"finish_reason\":null}]}\n\n";
    let chunks = translate(native).await;
    assert_eq!(chunks.len(), 2);
    assert!(output_text(&chunks).contains("partial"));
    assert!(matches!(chunks.last(), Some(Err(Error::Protocol(_)))));
    let complete = native.replace("null", "\"stop\"");
    let chunks = translate(&complete).await;
    assert!(chunks.iter().all(Result::is_ok));
    assert_eq!(output_text(&chunks), complete);
}

struct DropSource {
    wire: Option<Bytes>,
    drops: Arc<AtomicUsize>,
}
impl Stream for DropSource {
    type Item = Result<Bytes, Error>;
    fn poll_next(mut self: Pin<&mut Self>, _: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        Poll::Ready(self.wire.take().map(Ok))
    }
}
impl Drop for DropSource {
    fn drop(&mut self) {
        self.drops.fetch_add(1, Ordering::SeqCst);
    }
}

#[tokio::test]
async fn eof_error_releases_source_before_reporting_and_is_emitted_once() {
    let drops = Arc::new(AtomicUsize::new(0));
    let source = DropSource {
        wire: Some(Bytes::from_static(DELTA.as_bytes())),
        drops: drops.clone(),
    };
    let mut output = Box::pin(translate_openai_chat_sse_to_legacy_completions_with_error(
        source,
        "test".into(),
    ));
    assert!(output.next().await.unwrap().is_ok());
    assert!(output.next().await.unwrap().is_err());
    assert_eq!(drops.load(Ordering::SeqCst), 1);
    assert!(output.next().await.is_none());
    assert!(output.next().await.is_none());
}

#[tokio::test]
async fn transport_error_is_not_replaced_by_eof_error() {
    let chunks = vec![
        Ok(Bytes::from_static(DELTA.as_bytes())),
        Err(Error::Transport(std::io::Error::new(
            std::io::ErrorKind::BrokenPipe,
            "original-marker",
        ))),
    ];
    let output = translate_openai_chat_sse_to_legacy_completions_with_error(
        stream::iter(chunks),
        "test".into(),
    )
    .collect::<Vec<_>>()
    .await;
    assert_eq!(output.len(), 2);
    assert!(
        matches!(&output[1], Err(Error::Transport(error)) if error.to_string() == "original-marker")
    );
    assert!(!output_text(&output).contains("[DONE]"));
}

#[tokio::test]
async fn done_remains_terminal_without_waiting_for_a_trailer() {
    let chunks = source(&format!("{DELTA}data: [DONE]\n\n")).chain(stream::pending());
    let output = tokio::time::timeout(
        std::time::Duration::from_secs(1),
        translate_openai_chat_sse_to_legacy_completions_with_error(chunks, "test".into())
            .collect::<Vec<_>>(),
    )
    .await
    .unwrap();
    assert!(output.iter().all(Result::is_ok));
    assert_eq!(output_text(&output).matches("[DONE]").count(), 1);
}

#[tokio::test]
async fn initial_bom_is_accepted_by_the_shared_decoder_at_every_byte_split() {
    let wire = format!("\u{feff}{DELTA}{FINISH}{USAGE}");
    for split in 0..=wire.len() {
        let chunks: Vec<Result<Bytes, Error>> = vec![
            Ok(Bytes::copy_from_slice(&wire.as_bytes()[..split])),
            Ok(Bytes::copy_from_slice(&wire.as_bytes()[split..])),
        ];
        let output = translate_openai_chat_sse_to_legacy_completions_with_error(
            stream::iter(chunks),
            "test".into(),
        )
        .collect::<Vec<_>>()
        .await;
        assert!(output.iter().all(Result::is_ok), "split={split}");
        assert!(output_text(&output).contains("partial"), "split={split}");
    }
}

#[tokio::test]
async fn initial_bom_does_not_hide_a_responses_terminal_or_change_native_bytes() {
    let wire = "\u{feff}data: {\"type\":\"response.completed\",\"response\":{\"status\":\"completed\",\"output\":[]}}\n\n";
    let chunks = wire
        .as_bytes()
        .iter()
        .map(|byte| Bytes::copy_from_slice(&[*byte]))
        .collect::<Vec<_>>();
    let input = stream::iter(chunks.clone().into_iter().map(Ok::<_, Error>));
    let output = require_responses_completion(input)
        .collect::<Vec<_>>()
        .await;
    assert!(output.iter().all(Result::is_ok));
    assert_eq!(output.len(), chunks.len());
    assert_eq!(output_text(&output), wire);
    for (before, after) in chunks.iter().zip(&output) {
        assert_eq!(before, after.as_ref().unwrap());
    }
}

#[tokio::test]
async fn initial_bom_observers_preserve_bytes_and_capture_finish_and_usage() {
    let wire = "\u{feff}data: {\"choices\":[{\"finish_reason\":\"stop\"}],\"usage\":{\"prompt_tokens\":2,\"completion_tokens\":3,\"total_tokens\":5}}\r\n\r\n";
    for split in 0..=wire.len() {
        let chunks: Vec<Result<Bytes, Error>> = vec![
            Ok(Bytes::copy_from_slice(&wire.as_bytes()[..split])),
            Ok(Bytes::copy_from_slice(&wire.as_bytes()[split..])),
        ];
        let (usage_stream, usage) = tap_sse_usage_with_error(stream::iter(chunks));
        let (observed, completion) = tap_sse_completion_semantics_with_error(usage_stream);
        let output = observed.collect::<Vec<_>>().await;
        assert_eq!(output.len(), 2);
        assert_eq!(output_text(&output), wire);
        assert_eq!(
            snapshot_tapped_usage(&usage).map(|usage| usage.total_tokens),
            Some(5),
            "split={split}"
        );
        assert_eq!(
            snapshot_tapped_completion_semantics(&completion).as_deref(),
            Some("stop"),
            "split={split}"
        );
    }
}

#[tokio::test]
async fn bom_after_the_first_line_is_not_treated_as_a_new_stream() {
    let wire = "\n\u{feff}data: {\"choices\":[{\"finish_reason\":\"stop\"}],\"usage\":{\"total_tokens\":5}}\n\n";
    let (usage_stream, usage) = tap_sse_usage_with_error(source(wire));
    let (observed, completion) = tap_sse_completion_semantics_with_error(usage_stream);
    assert_eq!(output_text(&observed.collect::<Vec<_>>().await), wire);
    assert!(snapshot_tapped_usage(&usage).is_none());
    assert!(snapshot_tapped_completion_semantics(&completion).is_none());
}

#[tokio::test]
async fn eof_flush_preserves_parsed_delta_before_reporting_incomplete() {
    let chunks = translate(DELTA.trim_end_matches('\n').to_string().as_str()).await;
    assert_eq!(
        chunks.len(),
        1,
        "unterminated last line retains historical ignore behavior"
    );
    assert!(chunks[0].is_err());
    let chunks = translate(DELTA.strip_suffix('\n').unwrap()).await;
    assert_eq!(chunks.len(), 2);
    assert!(output_text(&chunks).contains("partial"));
    assert!(chunks[1].is_err());
}

#[tokio::test]
async fn existing_early_terminal_paths_do_not_poll_a_pending_trailer() {
    for wire in ["data: {\"choices\":[{\"delta\":{},\"finish_reason\":\"stop\"}],\"usage\":{\"total_tokens\":5}}\n\n".to_string(),
        "data: {\"object\":\"text_completion.chunk\",\"choices\":[{\"text\":\"ok\"}]}\n\ndata: [DONE]\n\n".into()] {
        let input = source(&wire).chain(stream::pending());
        let output = tokio::time::timeout(std::time::Duration::from_secs(1),
            translate_openai_chat_sse_to_legacy_completions_with_error(input, "test".into()).collect::<Vec<_>>())
            .await.unwrap();
        assert!(output.iter().all(Result::is_ok));
        assert!(output_text(&output).contains("[DONE]"));
    }
}

#[tokio::test]
async fn downstream_drop_after_a_delta_releases_the_source_without_repolling() {
    let drops = Arc::new(AtomicUsize::new(0));
    let source = DropSource {
        wire: Some(Bytes::from_static(DELTA.as_bytes())),
        drops: drops.clone(),
    };
    let mut output = Box::pin(translate_openai_chat_sse_to_legacy_completions_with_error(
        source,
        "test".into(),
    ));
    assert!(output.next().await.unwrap().is_ok());
    assert_eq!(drops.load(Ordering::SeqCst), 0);
    drop(output);
    assert_eq!(drops.load(Ordering::SeqCst), 1);
}
