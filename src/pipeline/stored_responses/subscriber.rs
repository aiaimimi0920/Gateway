//! A bounded SSE subscriber is not the owner of SAP transport or writes.
use super::{start_owned, Request};
use crate::{
    error::GatewayError,
    pipeline::{PipelineContext, PipelineOutput},
    protocol::stream_error::{ProtocolStreamError, StreamError},
    state::{stored_response_tasks::receive, AppState, StoredResponseAdmission},
    upstream::stream::TrackedStream,
};
use bytes::Bytes;
use futures::{stream, StreamExt};
use parking_lot::Mutex;
use std::sync::Arc;
use tokio::sync::mpsc;
use tokio_stream::wrappers::ReceiverStream;

#[derive(Default)]
struct End {
    terminal: Option<Bytes>,
    overflow: bool,
}

pub(super) struct Publisher {
    sender: Mutex<Option<mpsc::Sender<Bytes>>>,
    end: Arc<Mutex<End>>,
}

impl Publisher {
    pub(super) fn publish(&self, bytes: Bytes, terminal: bool) {
        if terminal {
            let mut end = self.end.lock();
            if !end.overflow {
                end.terminal = Some(bytes);
            }
            return;
        }
        let mut sender = self.sender.lock();
        if let Some(tx) = sender.as_ref() {
            // A full subscriber must not stall durable accounting, and a
            // partial delivery must never end with a success frame.
            if tx.try_send(bytes).is_err() {
                *sender = None;
                self.end.lock().overflow = true;
            }
        }
    }
}

pub(in crate::pipeline) async fn run_live(
    ctx: PipelineContext,
    state: Arc<AppState>,
    admission: StoredResponseAdmission,
) -> Result<PipelineOutput, GatewayError> {
    run_stream(ctx, Request::Create, state, true, admission).await
}

pub(super) async fn run_stream(
    ctx: PipelineContext,
    request: Request,
    state: Arc<AppState>,
    caller_admitted: bool,
    admission: StoredResponseAdmission,
) -> Result<PipelineOutput, GatewayError> {
    let (tx, mut rx) = mpsc::channel(16);
    let end = Arc::new(Mutex::new(End::default()));
    let publisher = Publisher {
        sender: Mutex::new(Some(tx)),
        end: end.clone(),
    };
    let (cancel, done) = start_owned(
        ctx,
        request,
        state,
        caller_admitted,
        admission,
        Some(publisher),
    );
    // A resumed stream may start with a validated delta or a durable terminal event.
    let Some(first) = rx.recv().await else {
        receive(done).await?;
        let terminal = end.lock().terminal.take().ok_or_else(|| {
            GatewayError::server_error("Stored SAP response stream did not start.")
        })?;
        return Ok(PipelineOutput::Sse(TrackedStream::new_with_error(
            stream::iter([Ok(terminal)]),
            |_, _| {},
        )));
    };
    let tail = stream::once(async move {
        let result = receive(done).await;
        drop(cancel);
        let mut end = end.lock();
        match result {
            Ok(_) if !end.overflow => end.terminal.take().ok_or_else(|| {
                stream_error("Stored SAP response stream ended before a durable terminal event.")
            }),
            Ok(_) => Err(stream_error(
                "Stored SAP response subscriber could not keep up.",
            )),
            Err(error) => Err(stream_error(&error.message)),
        }
    });
    let stream = stream::iter([Ok(first)])
        .chain(ReceiverStream::new(rx).map(Ok))
        .chain(tail);
    Ok(PipelineOutput::Sse(TrackedStream::new_with_error(
        stream,
        |_, _| {},
    )))
}

fn stream_error(message: &str) -> StreamError<rquest::Error> {
    StreamError::Protocol(ProtocolStreamError::invalid_data(message.to_owned()))
}
