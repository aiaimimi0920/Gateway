//! Terminal delivery must release upstream even if the consumer never polls again.
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};
use std::task::Poll;

use bytes::Bytes;
use futures::StreamExt;
use serde_json::json;

use super::event_stream_tests::{event_frame, provider_error_frame, request};
use super::{build_translator_state, translate_kiro_stream};

struct DropSignal(Arc<AtomicBool>);

impl Drop for DropSignal {
    fn drop(&mut self) {
        self.0.store(true, Ordering::SeqCst);
    }
}

#[tokio::test]
async fn terminal_errors_release_upstream_before_consumer_polls_again() {
    for anthropic in [false, true] {
        for parser_failure in [false, true] {
            let dropped = Arc::new(AtomicBool::new(false));
            let signal = DropSignal(dropped.clone());
            let frame = if parser_failure {
                let mut bytes = event_frame("assistantResponseEvent", json!({"content":"bad"}));
                bytes[8] ^= 1;
                bytes
            } else {
                provider_error_frame()
            };
            let mut input = Some(Bytes::from(frame));
            let inner = futures::stream::poll_fn(move |_| {
                let _keep_alive = &signal;
                Poll::Ready(input.take().map(Ok))
            });
            let mut output = Box::pin(translate_kiro_stream(
                inner,
                build_translator_state("claude-sonnet-4.6".into(), request()),
                anthropic,
            ));
            loop {
                let chunk = output.next().await.expect("error event").unwrap();
                if String::from_utf8_lossy(&chunk).contains("\"error\"") {
                    break;
                }
            }
            assert!(
                dropped.load(Ordering::SeqCst),
                "upstream retained: anthropic={anthropic}, parser_failure={parser_failure}"
            );
            assert!(output.next().await.is_none());
        }
    }
}

#[tokio::test]
async fn upstream_error_releases_connection_without_synthetic_completion() {
    for anthropic in [false, true] {
        let dropped = Arc::new(AtomicBool::new(false));
        let signal = DropSignal(dropped.clone());
        let mut error = Some(rquest::Client::new().get("not a URL").build().unwrap_err());
        let inner = futures::stream::poll_fn(move |_| {
            let _keep_alive = &signal;
            Poll::Ready(error.take().map(Err))
        });
        let mut output = Box::pin(translate_kiro_stream(
            inner,
            build_translator_state("claude-sonnet-4.6".into(), request()),
            anthropic,
        ));
        assert!(output.next().await.unwrap().is_err());
        assert!(dropped.load(Ordering::SeqCst));
        assert!(output.next().await.is_none());
    }
}
