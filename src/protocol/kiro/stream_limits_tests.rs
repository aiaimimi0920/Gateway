//! Small injected limits prove state bounds without allocating production-sized buffers.
use bytes::Bytes;
use futures::StreamExt;
use serde_json::json;
use std::sync::{
    atomic::{AtomicBool, AtomicUsize, Ordering},
    Arc,
};
use std::time::Duration;

use super::event_stream_tests::{event_frame, request};
use super::stream_state::StreamLimits;
use super::{
    build_translator_state, push_anthropic_events, push_openai_events, translate_kiro_stream,
    KiroEvent,
};

fn tool(id: &str, name: &str, input: &str) -> Vec<u8> {
    event_frame(
        "toolUseEvent",
        json!({"toolUseId":id,"name":name,"input":input,"stop":false}),
    )
}

async fn output(anthropic: bool, frames: Vec<Vec<u8>>, limits: StreamLimits) -> String {
    let mut state = build_translator_state("claude-sonnet-4.6".into(), request());
    state.limits = limits;
    let input = futures::stream::iter(frames.into_iter().map(|frame| Ok(Bytes::from(frame))));
    translate_kiro_stream(input, state, anthropic)
        .collect::<Vec<_>>()
        .await
        .into_iter()
        .map(|chunk| String::from_utf8(chunk.unwrap().to_vec()).unwrap())
        .collect()
}

fn assert_failure(wire: &str) {
    assert!(wire.contains("Kiro streaming state exceeds"), "{wire}");
    assert!(!wire.contains("[DONE]"));
    assert!(!wire.contains("message_stop"));
    assert!(!wire.contains("REJECTED_INPUT"));
}

#[test]
fn emitted_arguments_are_not_retained_in_stream_state() {
    for anthropic in [false, true] {
        let mut state = build_translator_state("claude-sonnet-4.6".into(), request());
        for _ in 0..4 {
            let event = KiroEvent::ToolUse {
                name: "read".into(),
                tool_use_id: "a".into(),
                input: "RETAINED_ARGUMENT_MARKER".into(),
                stop: false,
            };
            if anthropic {
                push_anthropic_events(&mut state, event);
            } else {
                push_openai_events(&mut state, event);
            }
            assert!(
                String::from_utf8(state.outputs.drain(..).flatten().collect())
                    .unwrap()
                    .contains("RETAINED_ARGUMENT_MARKER")
            );
        }
        assert!(!format!("{:?}", state.pending_tools).contains("RETAINED_ARGUMENT_MARKER"));
    }
}

#[tokio::test]
async fn new_tool_over_count_limit_fails_without_success_terminal() {
    for anthropic in [false, true] {
        let wire = output(
            anthropic,
            vec![
                tool("a", "read", "accepted"),
                tool("b", "read", "REJECTED_INPUT"),
            ],
            StreamLimits {
                tools: 1,
                ..Default::default()
            },
        )
        .await;
        assert!(wire.contains("accepted"));
        assert_failure(&wire);
    }
}

#[tokio::test]
async fn metadata_limit_counts_retained_ids_and_restored_name_only_once() {
    for anthropic in [false, true] {
        let wire = output(
            anthropic,
            vec![
                tool("a", "read", "first"),
                tool("a", "ignored_changed_name", "second"),
                tool("b", "read", "REJECTED_INPUT"),
            ],
            StreamLimits {
                metadata_bytes: 6,
                ..Default::default()
            },
        )
        .await;
        assert!(wire.contains("first") && wire.contains("second"));
        assert_failure(&wire);
    }
}

#[tokio::test]
async fn anthropic_text_tool_text_obeys_total_block_limit() {
    let wire = output(
        true,
        vec![
            event_frame("assistantResponseEvent", json!({"content":"first"})),
            tool("a", "read", "second"),
            event_frame(
                "assistantResponseEvent",
                json!({"content":"REJECTED_INPUT"}),
            ),
        ],
        StreamLimits {
            blocks: 2,
            ..Default::default()
        },
    )
    .await;
    assert!(wire.contains("first") && wire.contains("second"));
    assert_failure(&wire);
}

#[tokio::test]
async fn repeated_tool_at_exact_limits_still_finishes_successfully() {
    for anthropic in [false, true] {
        let wire = output(
            anthropic,
            vec![tool("a", "read", "first"), tool("a", "ignored", "second")],
            StreamLimits {
                tools: 1,
                metadata_bytes: 6,
                blocks: 1,
            },
        )
        .await;
        assert!(!wire.contains("Kiro streaming state exceeds"));
        assert!(wire.contains("first") && wire.contains("second"));
        assert!(wire.contains(if anthropic { "message_stop" } else { "[DONE]" }));
    }
}

#[test]
fn restored_names_are_charged_as_utf8_bytes_before_state_changes() {
    let restored = "restored_\u{1f980}";
    for anthropic in [false, true] {
        let mut state = build_translator_state("claude-sonnet-4.6".into(), request());
        state.tool_name_map.insert("wire".into(), restored.into());
        let event = KiroEvent::ToolUse {
            name: "wire".into(),
            tool_use_id: "a".into(),
            input: String::new(),
            stop: false,
        };
        state.limits.metadata_bytes = 2 + restored.len() - 1;
        assert!(state.admit(&event, anthropic).is_err());
        assert_eq!(state.retained_tool_bytes, 0);
        assert!(state.pending_tools.is_empty());
        assert!(state.outputs.is_empty());
        assert_eq!(state.next_block_index, 0);
        state.limits.metadata_bytes += 1;
        state.admit(&event, anthropic).unwrap();
        if anthropic {
            push_anthropic_events(&mut state, event);
        } else {
            push_openai_events(&mut state, event);
        }
        assert_eq!(state.retained_tool_bytes, 2 + restored.len());
        assert_eq!(state.pending_tools["a"].name, restored);
    }
}

struct DropFlag(Arc<AtomicBool>);

impl Drop for DropFlag {
    fn drop(&mut self) {
        self.0.store(true, Ordering::SeqCst);
    }
}

#[tokio::test]
async fn state_limit_releases_upstream_before_error_delivery_without_extra_polling() {
    for anthropic in [false, true] {
        let dropped = Arc::new(AtomicBool::new(false));
        let polls = Arc::new(AtomicUsize::new(0));
        let flag = DropFlag(dropped.clone());
        let poll_count = polls.clone();
        let frames = vec![
            tool("a", "read", "accepted"),
            tool("b", "read", "REJECTED_INPUT"),
            tool("c", "read", "never_polled"),
        ];
        let input = futures::stream::iter(frames).map(move |frame| {
            let _keep_alive = &flag;
            poll_count.fetch_add(1, Ordering::SeqCst);
            Ok(Bytes::from(frame))
        });
        let mut state = build_translator_state("claude-sonnet-4.6".into(), request());
        state.limits.tools = 1;
        let stream = translate_kiro_stream(input, state, anthropic);
        futures::pin_mut!(stream);
        tokio::time::timeout(Duration::from_secs(2), async {
            loop {
                let chunk = stream.next().await.expect("limit error event").unwrap();
                let wire = String::from_utf8(chunk.to_vec()).unwrap();
                if wire.contains("Kiro streaming state exceeds") {
                    assert_failure(&wire);
                    assert!(dropped.load(Ordering::SeqCst));
                    assert_eq!(polls.load(Ordering::SeqCst), 2);
                    break;
                }
            }
            assert!(stream.next().await.is_none());
        })
        .await
        .unwrap();
    }
}
