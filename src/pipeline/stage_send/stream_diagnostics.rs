//! Preserve bounded, sanitized stream failure evidence without changing replay behavior.
use futures::StreamExt;
use std::{error::Error, sync::Arc};

pub(super) type FailureEvidence = Arc<parking_lot::Mutex<Option<String>>>;

pub(super) fn observe(stream: super::ByteStream) -> (super::ByteStream, FailureEvidence) {
    let evidence = Arc::new(parking_lot::Mutex::new(None));
    let captured = Arc::clone(&evidence);
    let mut decoder = crate::protocol::stream_decode::BoundedSseDecoder::new(64 * 1024);
    let stream = stream.map(move |result| {
        if let Err(error) = &result {
            *captured.lock() = Some(describe(error));
        } else if let Ok(bytes) = &result {
            decoder.push_chunk(bytes.clone());
            while let crate::protocol::stream_decode::DecodeStep::Frame(frame) =
                decoder.decode_next()
            {
                let Ok(value) = serde_json::from_str::<serde_json::Value>(&frame.data) else {
                    continue;
                };
                let error = value
                    .get("error")
                    .filter(|v| !v.is_null())
                    .or_else(|| value.pointer("/response/error").filter(|v| !v.is_null()));
                let kind = value
                    .get("type")
                    .and_then(|v| v.as_str())
                    .or(frame.event_name.as_deref())
                    .unwrap_or("");
                if error.is_some()
                    || matches!(kind, "error" | "response.failed" | "response.cancelled")
                {
                    let message = error
                        .map(|v| v.to_string())
                        .unwrap_or_else(|| kind.to_owned());
                    *captured.lock() =
                        Some(crate::error::sanitize_provider_error_message(&message));
                }
            }
        }
        result
    });
    (Box::pin(stream), evidence)
}

fn describe(error: &(dyn Error + 'static)) -> String {
    // Bound both chain traversal and retained text; never persist unsanitized URLs/secrets.
    let mut parts = Vec::new();
    let mut current = Some(error);
    for _ in 0..8 {
        let Some(cause) = current else { break };
        let message = crate::error::sanitize_provider_error_message(&cause.to_string());
        if parts.last() != Some(&message) {
            parts.push(message);
        }
        current = cause.source();
    }
    crate::error::sanitize_provider_error_message(&parts.join(": "))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn preserves_nested_failure_without_duplicate_wrappers() {
        let error = std::io::Error::other("connection reset by peer");
        assert_eq!(describe(&error), "connection reset by peer");
    }
}
