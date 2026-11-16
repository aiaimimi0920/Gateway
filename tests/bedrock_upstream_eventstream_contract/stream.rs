//! The public streaming owner must preserve event order and release its source on failure/drop.
use super::fixtures::{event, sequence};
use bytes::Bytes;
use futures::{Stream, StreamExt};
use neuro_gateway::protocol::{
    bedrock_converse::translate_bedrock_eventstream_to_openai_with_error as translate,
    stream_error::StreamError,
};
use serde_json::{json, Value};
use std::{
    collections::VecDeque,
    pin::Pin,
    sync::{
        atomic::{AtomicUsize, Ordering},
        Arc,
    },
    task::{Context, Poll},
};

type Error = StreamError<std::io::Error>;

async fn outputs(chunks: Vec<Bytes>) -> (Vec<Value>, Vec<String>, usize) {
    let stream = translate(
        futures::stream::iter(chunks.into_iter().map(Ok::<_, Error>)),
        "fixture".into(),
    );
    futures::pin_mut!(stream);
    let (mut values, mut errors, mut done) = (Vec::new(), Vec::new(), 0);
    while let Some(item) = stream.next().await {
        match item {
            Err(error) => errors.push(error.to_string()),
            Ok(bytes) => {
                for line in std::str::from_utf8(&bytes).unwrap().lines() {
                    if line == "data: [DONE]" {
                        done += 1;
                    } else if let Some(body) = line.strip_prefix("data: ") {
                        values.push(serde_json::from_str(body).unwrap());
                    }
                }
            }
        }
    }
    (values, errors, done)
}

#[tokio::test]
async fn bedrock_upstream_stream_handles_every_byte_boundary_and_one_final_usage() {
    let wire = sequence().concat();
    for chunks in [
        vec![Bytes::from(wire.clone())],
        wire.iter().map(|b| Bytes::copy_from_slice(&[*b])).collect(),
    ] {
        let (values, errors, done) = outputs(chunks).await;
        assert!(errors.is_empty(), "{errors:?}");
        assert_eq!(done, 1);
        assert_eq!(values[0]["choices"][0]["delta"]["role"], "assistant");
        let text: String = values
            .iter()
            .filter_map(|v| {
                v.pointer("/choices/0/delta/content")
                    .and_then(Value::as_str)
            })
            .collect();
        assert_eq!(text, "你好");
        let calls: Vec<_> = values
            .iter()
            .filter_map(|v| v.pointer("/choices/0/delta/tool_calls/0"))
            .collect();
        assert_eq!(calls.iter().filter(|v| v.get("id").is_some()).count(), 1);
        assert_eq!(calls[0]["id"], "call-1");
        assert!(calls.iter().all(|v| v["index"] == 0));
        let arguments: String = calls
            .iter()
            .filter_map(|v| v.pointer("/function/arguments").and_then(Value::as_str))
            .collect();
        assert_eq!(arguments, "{\"city\": \"北京\"}");
        assert_eq!(
            values.iter().filter(|v| v.get("usage").is_some()).count(),
            1
        );
        assert_eq!(values.last().unwrap()["usage"]["total_tokens"], 13);
        assert_eq!(
            values.last().unwrap()["choices"][0]["finish_reason"],
            "tool_calls"
        );
        assert_eq!(
            values
                .iter()
                .filter(|v| v
                    .pointer("/choices/0/finish_reason")
                    .is_some_and(|v| !v.is_null()))
                .count(),
            1
        );
    }
}

#[tokio::test]
async fn bedrock_upstream_tools_use_dense_indices_and_empty_arguments_match_buffered() {
    let mut frames = sequence()[..3].to_vec();
    for (block, id, input) in [(4, "first", "{ }"), (9, "second", "")] {
        frames.push(event(
            "contentBlockStart",
            json!({"contentBlockIndex":block,"start":{"toolUse":{"toolUseId":id,"name":"f"}}}),
        ));
        frames.push(event(
            "contentBlockDelta",
            json!({"contentBlockIndex":block,"delta":{"toolUse":{"input":input}}}),
        ));
        frames.push(event(
            "contentBlockStop",
            json!({"contentBlockIndex":block}),
        ));
    }
    frames.extend_from_slice(&sequence()[7..]);
    let (values, errors, done) = outputs(vec![Bytes::from(frames.concat())]).await;
    assert!(errors.is_empty(), "{errors:?}");
    assert_eq!(done, 1);
    let calls: Vec<_> = values
        .iter()
        .filter_map(|v| v.pointer("/choices/0/delta/tool_calls/0"))
        .collect();
    let starts: Vec<_> = calls
        .iter()
        .filter(|v| v.get("id").is_some())
        .map(|v| (v["index"].as_u64().unwrap(), v["id"].as_str().unwrap()))
        .collect();
    assert_eq!(starts, vec![(0, "first"), (1, "second")]);
    let buffered = super::accumulate_stream(super::fixtures::response(frames), "fixture")
        .await
        .unwrap();
    for index in 0..2 {
        let arguments: String = calls
            .iter()
            .filter(|v| v["index"] == index)
            .filter_map(|v| v.pointer("/function/arguments").and_then(Value::as_str))
            .collect();
        assert_eq!(
            arguments,
            buffered.tool_calls[index].arguments.as_deref().unwrap()
        );
    }
}

#[tokio::test]
async fn bedrock_upstream_missing_metadata_or_trailing_corruption_never_emits_done() {
    let frames = sequence();
    let mut corrupt = frames.concat();
    corrupt.extend_from_slice(&[1, 2, 3]);
    for wire in [frames[..8].concat(), corrupt] {
        let (values, errors, done) = outputs(vec![Bytes::from(wire)]).await;
        assert_eq!(errors.len(), 1);
        assert_eq!(done, 0);
        assert!(values
            .iter()
            .all(|v| v.pointer("/choices/0/finish_reason").unwrap().is_null()));
    }
}

struct Tracked {
    queue: VecDeque<Result<Bytes, Error>>,
    polls: Arc<AtomicUsize>,
    drops: Arc<AtomicUsize>,
}
impl Stream for Tracked {
    type Item = Result<Bytes, Error>;
    fn poll_next(mut self: Pin<&mut Self>, _: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        self.polls.fetch_add(1, Ordering::SeqCst);
        Poll::Ready(self.queue.pop_front())
    }
}
impl Drop for Tracked {
    fn drop(&mut self) {
        self.drops.fetch_add(1, Ordering::SeqCst);
    }
}

#[tokio::test]
async fn bedrock_upstream_backpressure_and_downstream_drop_do_not_drain_source() {
    let polls = Arc::new(AtomicUsize::new(0));
    let drops = Arc::new(AtomicUsize::new(0));
    let source = Tracked {
        queue: sequence().into_iter().map(|b| Ok(Bytes::from(b))).collect(),
        polls: polls.clone(),
        drops: drops.clone(),
    };
    let mut stream = Box::pin(translate(source, "fixture".into()));
    assert!(stream.next().await.unwrap().is_ok());
    assert_eq!(polls.load(Ordering::SeqCst), 1);
    drop(stream);
    assert_eq!(polls.load(Ordering::SeqCst), 1);
    assert_eq!(drops.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn bedrock_upstream_terminal_error_drops_source_before_error_delivery() {
    for transport in [false, true] {
        let polls = Arc::new(AtomicUsize::new(0));
        let drops = Arc::new(AtomicUsize::new(0));
        let mut queue = VecDeque::from([Ok(Bytes::from(sequence()[0].clone()))]);
        if transport {
            queue.push_back(Err(StreamError::Transport(std::io::Error::other(
                "fixture transport",
            ))));
        } else {
            queue.push_back(Ok(Bytes::from(event(
                "unknownPrivateEvent",
                json!({"secret":"private-fixture-secret"}),
            ))));
        }
        queue.push_back(Ok(Bytes::from(sequence().concat())));
        let source = Tracked {
            queue,
            polls: polls.clone(),
            drops: drops.clone(),
        };
        let mut stream = Box::pin(translate(source, "fixture".into()));
        assert!(stream.next().await.unwrap().is_ok());
        let error = stream.next().await.unwrap().unwrap_err();
        assert_eq!(matches!(error, StreamError::Transport(_)), transport);
        assert!(!error.to_string().contains("private-fixture-secret"));
        assert_eq!(drops.load(Ordering::SeqCst), 1);
        assert_eq!(polls.load(Ordering::SeqCst), 2);
        assert!(stream.next().await.is_none());
    }
}
