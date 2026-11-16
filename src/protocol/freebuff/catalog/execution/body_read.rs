//! One bounded frame at a time, under the original step deadline/cancel and original run journal.
use super::super::super::{
    completion_decode::{decode_json, Accounting, StreamDecoder, BODY_LIMIT},
    BusinessError,
};
use super::super::CatalogCompletionBody;
use futures::StreamExt;
use serde_json::Value;
use std::{future::Future, pin::Pin};

pub(super) async fn read<C, F, O>(
    body: &mut CatalogCompletionBody<'_>,
    status: u16,
    kind: &str,
    declared: Option<u64>,
    accounting: &mut Accounting,
    mut cancelled: Pin<&mut C>,
    mut observe: O,
) -> Result<Value, BusinessError>
where
    C: Future<Output = ()>,
    O: FnMut(Value) -> F,
    F: Future<Output = Result<(), BusinessError>>,
{
    tokio::select! { biased; _ = cancelled.as_mut() => return Err(BusinessError::Cancelled), _ = std::future::ready(()) => {} }
    if status != 200 {
        return Err(BusinessError::InvalidResponse);
    }
    if declared.is_some_and(|size| size > BODY_LIMIT as u64) {
        return Err(BusinessError::ResponseTooLarge);
    }
    if kind == "text/event-stream" {
        let mut decoder = StreamDecoder::new();
        loop {
            let chunk = tokio::select! {
                biased;
                _ = cancelled.as_mut() => return Err(BusinessError::Cancelled),
                chunk = body.next() => chunk,
            };
            let Some(chunk) = chunk else {
                return decoder.finish();
            };
            let chunk = chunk.map_err(|e| {
                if e.is_timeout() {
                    BusinessError::Timeout
                } else {
                    BusinessError::Transport
                }
            })?;
            decoder.feed(chunk)?;
            loop {
                tokio::select! { biased; _ = cancelled.as_mut() => return Err(BusinessError::Cancelled), _ = std::future::ready(()) => {} }
                let frame = decoder.next_frame();
                *accounting = decoder.accounting().clone();
                // Even a rejected frame may carry a valid cost. Observe it before propagating its
                // error. Unchanged costs do not write SQLite; a cancelled write remains unknown.
                tokio::select! {
                    biased;
                    _ = cancelled.as_mut() => return Err(BusinessError::Cancelled),
                    saved = body._run.journal.observe(&body._run.accounting, accounting) => saved?,
                }
                let Some(frame) = frame? else {
                    break;
                };
                // The only externally awaited seam has no run/claim authority. It cannot finish,
                // replay or begin another step. Check cancellation even for an immediately-ready
                // observer, so a later frame in this same network chunk cannot bypass the signal.
                tokio::select! {
                    biased;
                    _ = cancelled.as_mut() => return Err(BusinessError::Cancelled),
                    result = observe(frame) => result?,
                }
            }
            #[cfg(test)]
            let _ = super::super::super::TEST_BODY_STARTED.try_with(|seen| seen.notify_one());
        }
    } else if kind == "application/json"
        || (kind.starts_with("application/") && kind.ends_with("+json"))
    {
        // JSON stays buffered and never invents provisional SSE events.
        let bytes = tokio::select! {
            biased;
            _ = cancelled.as_mut() => return Err(BusinessError::Cancelled),
            bytes = crate::protocol::upstream_body::collect_bounded_upstream_stream_with_limit_and_provider(
                body, "FreeBuff catalog completion", BODY_LIMIT, Some("freebuff")) => bytes,
        }.map_err(|error| if error.code.as_deref() == Some("upstream_body_too_large") {
            BusinessError::ResponseTooLarge
        } else { BusinessError::Transport })?;
        let value = serde_json::from_slice(&bytes).map_err(|_| BusinessError::InvalidResponse)?;
        decode_json(value, accounting)
    } else {
        Err(BusinessError::InvalidResponse)
    }
}
