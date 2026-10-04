//! Retry diagnostics may contain bounded timings/categories, never raw error material.
use neuro_gateway::error::{FallbackHint, GatewayError};
use neuro_gateway::retry::{execute_with_retry, RetryPolicy};
use std::io::{self, Write};
use std::sync::{Arc, Mutex};

#[derive(Clone, Default)]
struct LogSink(Arc<Mutex<Vec<u8>>>);

impl Write for LogSink {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        self.0.lock().unwrap().extend_from_slice(bytes);
        Ok(bytes.len())
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

impl<'a> tracing_subscriber::fmt::MakeWriter<'a> for LogSink {
    type Writer = Self;
    fn make_writer(&'a self) -> Self {
        self.clone()
    }
}

#[tokio::test(start_paused = true)]
async fn retry_and_wait_exhaustion_logs_do_not_expose_error_body_or_headers() {
    let sink = LogSink::default();
    let subscriber = tracing_subscriber::fmt()
        .without_time()
        .with_ansi(false)
        .with_max_level(tracing::Level::WARN)
        .with_writer(sink.clone())
        .finish();
    let _guard = tracing::subscriber::set_default(subscriber);
    let policy = RetryPolicy {
        max_retries: 1,
        ..Default::default()
    };
    for hint in [1, u64::MAX] {
        let _ = execute_with_retry(
            || async {
                let mut error = GatewayError::rate_limited("request-body-secret", hint)
                    .with_provider("https://fixture.invalid?token=header-secret")
                    .with_code("raw-code-secret");
                error.fallback_hint = FallbackHint::Retry {
                    delay_ms: hint,
                    reason: "fallback-reason-secret".to_string(),
                };
                Err::<(), _>(error)
            },
            &policy,
        )
        .await;
    }
    let text = String::from_utf8(sink.0.lock().unwrap().clone()).unwrap();
    assert!(text.contains("retrying"));
    assert!(text.contains("wait budget exhausted"));
    assert!(text.contains("RateLimit"));
    for forbidden in [
        "request-body-secret",
        "header-secret",
        "raw-code-secret",
        "fallback-reason-secret",
        "fixture.invalid",
        "18446744073709551615",
    ] {
        assert!(!text.contains(forbidden), "{text}");
    }
}
