//! Own the SAP byte stream independently of the public subscriber.
mod wire;

use super::{
    super::{
        evidence::Evidence,
        failure,
        transport::{DispatchError, Prepared},
        State, UpstreamId,
    },
    dispatch::{live_id, not_found, Observation},
    Receipt, RoutePin, SendHooks, Store,
};
use crate::{
    error::GatewayError,
    protocol::stream_decode::{BoundedSseDecoder, DecodeStep},
    routing::model_circuit::CircuitAttempt,
};
use bytes::Bytes;
use futures::StreamExt;
use serde_json::{json, Value};
use std::future::Future;
use tokio::time::Instant;
use wire::{event, Frame, Wire};

pub(super) type Publish<'a> = &'a (dyn Fn(Bytes, bool) + Sync);
pub(super) enum Source<'a> {
    Create {
        circuit: &'a CircuitAttempt,
        background: bool,
    },
    Resume {
        id: UpstreamId,
        cursor: Option<i32>,
    },
}
const MAX_FRAME: usize = 512 * 1024;

pub(super) async fn consume(
    prepared: Prepared,
    store: &Store<'_>,
    mut receipt: Receipt,
    pin: &RoutePin,
    hooks: SendHooks<'_>,
    source: Source<'_>,
    publish: Publish<'_>,
) -> Result<Observation, DispatchError> {
    let (mut wire, circuit) = match source {
        Source::Create {
            circuit,
            background,
        } => (Wire::create(background), Some(circuit)),
        Source::Resume { id, cursor } => (Wire::resume(id, cursor), None),
    };
    let mut status = None;
    let response = before(hooks.deadline, prepared.open(&mut status))
        .await
        .unwrap_or_else(|| Err(Evidence::failed(timeout(), status)))
        .map_err(|evidence| report(&receipt, &hooks, circuit, evidence))?;
    let status = response.status().as_u16();
    let failed = |receipt: &Receipt, error| {
        report(
            receipt,
            &hooks,
            circuit,
            Evidence::failed(error, Some(status)),
        )
    };
    if !response
        .headers()
        .get("content-type")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.split(';').next())
        .is_some_and(|v| v.trim().eq_ignore_ascii_case("text/event-stream"))
    {
        return Err(failed(&receipt, invalid()));
    }
    let mut bytes = Box::pin(response.bytes_stream());
    let mut decoder = BoundedSseDecoder::new_strict(MAX_FRAME);
    let mut terminal = None;
    loop {
        match decoder.decode_next() {
            DecodeStep::NeedInput => match before(hooks.deadline, bytes.next()).await {
                Some(Some(Ok(chunk))) => decoder.push_chunk(chunk),
                Some(None) => break,
                Some(Some(Err(_))) => {
                    return Err(failed(&receipt, failure("sap_aicore_body_incomplete")))
                }
                None => return Err(failed(&receipt, timeout())),
            },
            DecodeStep::Error(_) => return Err(failed(&receipt, invalid())),
            DecodeStep::Frame(frame) => {
                match wire.accept(frame).map_err(|e| failed(&receipt, e))? {
                    Frame::Ignore => {}
                    Frame::Delta(frame) => publish(frame, false),
                    Frame::Snapshot {
                        name,
                        sequence,
                        id,
                        state: State::Pending,
                        body,
                    } => {
                        // Bind the provider identity before the caller can use the public ID.
                        if name == "response.created" && circuit.is_some() {
                            match store.observe(&receipt, &id, State::Pending).await {
                                Ok(Some(bound)) => receipt = bound,
                                Ok(None) | Err(_) => {
                                    // We received headers, but the local receipt could not
                                    // advance. This is not an upstream health failure.
                                    return Err(report(
                                        &receipt,
                                        &hooks,
                                        circuit,
                                        Evidence::Neutral(status),
                                    ));
                                }
                            }
                        }
                        // A resumed stream can replay pending frames after a concurrent
                        // reader has already persisted the terminal state. The pending
                        // snapshot is history, not a new receipt transition.
                        let public = if circuit.is_none() {
                            super::projection::historical_pending(&receipt, pin, &body)
                                .map_err(|error| failed(&receipt, error))?
                        } else {
                            let observation = Observation { receipt, body };
                            let public = observation
                                .public_json(pin)
                                .map_err(|error| failed(&observation.receipt, error))?;
                            receipt = observation.receipt;
                            public
                        };
                        publish(snapshot(name, sequence, public), false);
                    }
                    Frame::Snapshot {
                        name,
                        sequence,
                        id,
                        state,
                        body,
                    } => {
                        terminal = Some((name, sequence, id, state, body));
                    }
                }
            }
        }
    }
    decoder
        .finish_strict()
        .map_err(|_| failed(&receipt, invalid()))?;
    let Some((name, sequence, id, state, body)) = terminal else {
        return Err(failed(&receipt, failure("sap_aicore_body_incomplete")));
    };
    let evidence = if let Some(circuit) = circuit {
        let evidence = Evidence::terminal(&body, status);
        evidence.finish(circuit);
        evidence
    } else {
        Evidence::Neutral(status)
    };
    (hooks.observe)(evidence);
    let latest = write_terminal(store, &receipt, &id, state)
        .await
        .map_err(|_| DispatchError::Recorded(receipt.id.clone()))?;
    let latest = store
        .settle_balance(&latest)
        .await
        .map_err(|_| DispatchError::SettlementPending(receipt.id.clone()))?
        .unwrap_or(latest);
    let observation = Observation {
        receipt: latest,
        body,
    };
    let public = observation
        .public_json(pin)
        .map_err(|_| DispatchError::Recorded(receipt.id.clone()))?;
    publish(snapshot(name, sequence, public), true);
    Ok(observation)
}

async fn write_terminal(
    store: &Store<'_>,
    receipt: &Receipt,
    id: &UpstreamId,
    state: State,
) -> Result<Receipt, GatewayError> {
    if let Some(row) = store.observe(receipt, id, state.clone()).await? {
        return Ok(row);
    }
    // A concurrent GET/cancel/delete may have advanced the revision. The
    // typed public projection will reject different phase/usage; never revive a tombstone.
    let read_live = || async {
        store
            .read(&receipt.id, &receipt.owner_tag)
            .await?
            .filter(|row| {
                row.binding_tag == receipt.binding_tag && live_id(row).as_ref() == Some(id)
            })
            .ok_or_else(not_found)
    };
    let latest = read_live().await?;
    if let Some(row) = store.observe(&latest, id, state).await? {
        return Ok(row);
    }
    // The second CAS can lose to DELETE as well. Do not project the pre-CAS row.
    read_live().await
}

fn snapshot(name: &str, seq: u64, response: Value) -> Bytes {
    event(
        name,
        &json!({"type":name,"sequence_number":seq,"response":response}),
    )
}

fn report(
    receipt: &Receipt,
    hooks: &SendHooks<'_>,
    circuit: Option<&CircuitAttempt>,
    evidence: Evidence,
) -> DispatchError {
    if let Some(circuit) = circuit {
        evidence.finish(circuit);
        (hooks.observe)(evidence);
    } else if let Some(status) = evidence.status() {
        (hooks.observe)(Evidence::Neutral(status));
    }
    DispatchError::Recorded(receipt.id.clone())
}

async fn before<T>(deadline: Option<Instant>, future: impl Future<Output = T>) -> Option<T> {
    match deadline {
        Some(at) => tokio::time::timeout_at(at, future).await.ok(),
        None => Some(future.await),
    }
}

fn invalid() -> GatewayError {
    failure("sap_aicore_response_failed")
}

fn timeout() -> GatewayError {
    failure("sap_aicore_response_delivery_unknown")
}
