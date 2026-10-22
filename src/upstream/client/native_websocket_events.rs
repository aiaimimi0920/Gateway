//! Pull-driven frame stream; cancellation drops an in-flight socket, never replays.
use super::*;

struct Turn {
    connection: Option<Connection>,
    slot: tokio::sync::OwnedMutexGuard<Option<Connection>>,
    request: Option<String>,
    failed: bool,
}

pub(super) fn stream(
    connection: Connection,
    slot: tokio::sync::OwnedMutexGuard<Option<Connection>>,
    request: String,
) -> impl futures::Stream<
    Item = Result<bytes::Bytes, crate::protocol::stream_error::StreamError<rquest::Error>>,
> + Send {
    let initial = Turn {
        connection: Some(connection),
        slot,
        request: Some(request),
        failed: false,
    };
    futures::stream::unfold(Some(initial), |turn| async move {
        let mut turn = turn?;
        let Some(mut connection) = turn.connection.take() else {
            return turn.failed.then(|| {
                (
                    Err(protocol_error("Upstream WebSocket response failed")),
                    None,
                )
            });
        };
        // Only send after stream handoff: ambiguous writes cannot trigger fallback.
        if let Some(request) = turn.request.take() {
            if connection
                .socket
                .send(Message::Text(request.into()))
                .await
                .is_err()
            {
                return Some((Err(protocol_error("Upstream WebSocket write failed")), None));
            }
        }
        loop {
            match connection.socket.next().await {
                Some(Ok(Message::Text(text))) => {
                    let event: Value = match serde_json::from_str(&text) {
                        Ok(event) => event,
                        Err(_) => {
                            return Some((
                                Err(protocol_error("Invalid upstream WebSocket JSON")),
                                None,
                            ))
                        }
                    };
                    let terminal = matches!(
                        event["type"].as_str(),
                        Some(
                            "response.completed"
                                | "response.failed"
                                | "response.incomplete"
                                | "error"
                        )
                    );
                    // Envelope only: taps observe usage without rewriting JSON.
                    let data = text
                        .lines()
                        .map(|line| format!("data: {line}\n"))
                        .collect::<String>()
                        + "\n";
                    if terminal {
                        turn.failed = event["type"] != "response.completed"
                            || event["response"]["status"] != "completed";
                        *turn.slot = Some(connection);
                    } else {
                        turn.connection = Some(connection);
                    }
                    return Some((Ok(bytes::Bytes::from(data)), Some(turn)));
                }
                Some(Ok(Message::Ping(_))) => {
                    if connection.socket.flush().await.is_err() {
                        return Some((Err(protocol_error("Upstream WebSocket pong failed")), None));
                    }
                }
                Some(Ok(Message::Pong(_))) => {}
                _ => {
                    return Some((
                        Err(protocol_error(
                            "Upstream WebSocket closed before completion",
                        )),
                        None,
                    ))
                }
            }
        }
    })
}
