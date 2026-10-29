use super::*;
use futures::stream;

fn capture() -> Capture {
    Capture {
        completed: Arc::new(AtomicBool::new(false)),
        lines: ObservationLines::default(),
        parser: SseParseState::new(),
        untrusted: false,
    }
}

fn frame(kind: &str, status: &str) -> Bytes {
    Bytes::from(format!(
        "event: {kind}\ndata: {{\"type\":\"{kind}\",\"response\":{{\"status\":\"{status}\"}}}}\n\n"
    ))
}

#[test]
fn terminal_is_recognized_across_fragmentation_and_large_frames() {
    let bytes = frame("response.completed", "completed");
    for width in 1..32 {
        let mut capture = capture();
        for chunk in bytes.chunks(width) {
            capture.feed(&Bytes::copy_from_slice(chunk));
        }
        assert!(capture.completed.load(Ordering::Acquire), "width {width}");
    }
    let value = serde_json::json!({"type": "response.completed", "response": {
        "status": "completed", "output": [{"text": "x".repeat(100_000)}]
    }});
    let mut capture = capture();
    capture.feed(&Bytes::from(format!("data: {value}\n\n")));
    assert!(capture.completed.load(Ordering::Acquire));
}

#[test]
fn item_completion_done_or_malformed_terminal_is_not_response_completion() {
    let candidates = [
        frame("response.output_item.done", "completed"),
        frame("response.output_text.done", "completed"),
        frame("response.completed", "in_progress"),
        Bytes::from_static(b"data: [DONE]\n\n"),
        Bytes::from_static(b"data: {\"type\":\"response.completed\",\"response\":{}}\n\n"),
        Bytes::from_static(b"event: response.completed\ndata: {\"type\":\"response.failed\",\"response\":{\"status\":\"completed\"}}\n\n"),
    ];
    for bytes in candidates {
        let mut capture = capture();
        capture.feed(&bytes);
        assert!(!capture.completed.load(Ordering::Acquire));
    }
}

#[test]
fn error_or_observation_failure_cannot_be_hidden_by_a_terminal() {
    for error in [
        Bytes::from_static(b"data: {\"type\":\"error\",\"error\":{\"message\":\"fixture\"}}\n\n"),
        frame("response.failed", "failed"),
        frame("response.cancelled", "cancelled"),
        Bytes::from_static(b"data: invalid-json\n\n"),
    ] {
        for error_first in [true, false] {
            let mut capture = capture();
            let terminal = frame("response.completed", "completed");
            let frames = if error_first {
                [&error, &terminal]
            } else {
                [&terminal, &error]
            };
            for bytes in frames {
                capture.feed(bytes);
            }
            assert!(!capture.completed.load(Ordering::Acquire));
        }
    }
    let mut capture = capture();
    capture.lines = ObservationLines::new(32, 4);
    capture.feed(&Bytes::from(format!("data: {}\n\n", "x".repeat(64))));
    capture.feed(&frame("response.completed", "completed"));
    assert!(!capture.completed.load(Ordering::Acquire));
}

#[test]
fn incomplete_is_terminal_but_a_new_response_revokes_previous_completion() {
    let mut capture = capture();
    capture.feed(&frame("response.incomplete", "incomplete"));
    assert!(capture.completed.load(Ordering::Acquire));
    capture.feed(&frame("response.created", "in_progress"));
    assert!(!capture.completed.load(Ordering::Acquire));
}

#[tokio::test]
async fn observation_preserves_bytes_and_typed_errors() {
    let bytes = frame("response.completed", "completed");
    let (mut stream, completed) = tap(Box::pin(stream::iter([
        Ok(bytes.clone()),
        Err(std::io::Error::other("fixture")),
    ])));
    assert_eq!(stream.next().await.unwrap().unwrap(), bytes);
    assert!(completed.load(Ordering::Acquire));
    assert_eq!(
        stream.next().await.unwrap().unwrap_err().to_string(),
        "fixture"
    );
    assert!(!completed.load(Ordering::Acquire));
}
