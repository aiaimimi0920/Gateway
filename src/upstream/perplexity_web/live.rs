//! Real snapshot observations precede completion; standard text never falsely appends replacements.
use super::{exchange::Exchange, Session, ASK_URL, MAX_DEADLINE};
use crate::{
    error::GatewayError,
    protocol::{
        canonical::CanonicalRelayRequest,
        perplexity_web::{self, public_snapshot, Update},
        sse_parse::format_sse_event,
        stream_error::{ProtocolStreamError, StreamError},
    },
    routing::candidate::ProviderAccountPayload,
    upstream::response_types::UpstreamStreamingResponse,
};
use bytes::Bytes;
use std::{collections::VecDeque, time::Duration};
const OUTPUT_LIMIT: usize = 32 * 1024 * 1024;

pub async fn execute_stream(
    client: &rquest::Client,
    timeout: Duration,
    payload: &ProviderAccountPayload,
    request: &CanonicalRelayRequest,
    model: &str,
) -> Result<UpstreamStreamingResponse, GatewayError> {
    let session = Session::from_payload(payload)?;
    let body = perplexity_web::pack_single_turn_for_model(request, model)?;
    let exchange =
        Exchange::open(client, &session, &body, timeout.min(MAX_DEADLINE), ASK_URL).await?;
    let state = Live {
        exchange: Some(exchange),
        pending: VecDeque::new(),
        bytes: 0,
        sequence: 0,
        request: request.clone(),
        model: model.into(),
    };
    let stream = futures::stream::unfold(Some(state), |state| async move {
        let mut state = state?;
        match state.next().await {
            Ok(Some(bytes)) => Some((Ok(bytes), Some(state))),
            Ok(None) => None,
            Err(error) => {
                let code = error
                    .code
                    .clone()
                    .unwrap_or_else(|| "perplexity_web_stream_failed".into());
                Some((
                    Err(StreamError::Protocol(ProtocolStreamError::classified(
                        code, error,
                    ))),
                    None,
                ))
            }
        }
    });
    Ok(UpstreamStreamingResponse::NativeEvents(Box::pin(stream)))
}
struct Live {
    exchange: Option<Exchange>,
    pending: VecDeque<Bytes>,
    bytes: usize,
    sequence: usize,
    request: CanonicalRelayRequest,
    model: String,
}
impl Live {
    fn admit(&mut self, size: usize) -> Result<(), GatewayError> {
        self.bytes = self
            .bytes
            .checked_add(size)
            .filter(|v| *v <= OUTPUT_LIMIT)
            .ok_or_else(|| {
                perplexity_web::failure(
                    "perplexity_web_output_limit",
                    "Perplexity Web 公共输出超过本地大小预算。",
                )
            })?;
        Ok(())
    }
    async fn next(&mut self) -> Result<Option<Bytes>, GatewayError> {
        if let Some(frame) = self.pending.pop_front() {
            return Ok(Some(frame));
        }
        let Some(exchange) = self.exchange.as_mut() else {
            return Ok(None);
        };
        match exchange.next().await? {
            Update::Snapshot(answer) => {
                self.sequence += 1;
                let carrier = serde_json::json!({public_snapshot::KEY:public_snapshot::value(self.sequence, answer.as_ref()),"choices":[]});
                let frame = Bytes::from(format_sse_event(
                    Some(public_snapshot::EVENT),
                    &carrier.to_string(),
                ));
                self.admit(frame.len())?;
                Ok(Some(frame))
            }
            Update::Complete(answer) => {
                // Final projection is fully admitted before any standard content/stop/DONE is emitted.
                let frames = crate::upstream::canonical_sse::canonical_response_to_openai_sse_bytes(
                    &self.request,
                    &self.model,
                    &answer.into_canonical(),
                );
                self.admit(frames.iter().map(Bytes::len).sum())?;
                self.exchange = None;
                self.pending.extend(frames);
                Ok(self.pending.pop_front())
            }
        }
    }
}
