//! Serial Responses WS turns: per-create governance, bounded frames and cancellation.
use super::*;
use crate::protocol::{
    codex_wire::NativeRequestWire,
    stream_decode::{BoundedSseDecoder, DecodeStep},
};
use crate::upstream::client::NativeResponsesSocket;
use axum::{
    extract::ws::{Message, WebSocket, WebSocketUpgrade},
    http::Uri,
};
use futures::{SinkExt, StreamExt};
use serde_json::{json, Value};
use std::time::Duration;

pub(crate) async fn handle_websocket(
    State(state): State<Arc<AppState>>,
    OptionalBearerToken(token): OptionalBearerToken,
    Query(query): Query<HashMap<String, String>>,
    uri: Uri,
    mut headers: HeaderMap,
    client_request_id: Option<axum::Extension<crate::http::native_json::ClientRequestId>>,
    ws: WebSocketUpgrade,
) -> Result<Response, GatewayError> {
    if client_request_id.is_some_and(|id| !id.0 .0) {
        headers.remove("x-request-id");
    }
    if uri.query().is_some_and(|q| q.len() > 8192) {
        return Err(GatewayError::bad_request("Request query too large"));
    }
    // Authenticate before upgrade, then authenticate again on every create so a
    // long-lived socket is not a permanent authorization grant.
    let mut ctx = prepare_context(
        json!({"input":[]}),
        token.clone(),
        query.clone(),
        &headers,
        &state,
    )?;
    crate::pipeline::stage_auth::run(&mut ctx, &state).await?;
    let limit = state.config.max_body_responses_bytes;
    Ok(ws
        .max_message_size(limit)
        .max_frame_size(limit)
        .on_upgrade(move |socket| async move {
            let _ = serve(socket, state, token, query, uri, headers).await;
        })
        .into_response())
}

async fn serve(
    socket: WebSocket,
    state: Arc<AppState>,
    token: Option<String>,
    query: HashMap<String, String>,
    uri: Uri,
    headers: HeaderMap,
) -> Result<(), ()> {
    let (mut writer, mut reader) = socket.split();
    let upstream = NativeResponsesSocket::default();
    loop {
        let next = tokio::time::timeout(Duration::from_secs(300), reader.next()).await;
        let text = match next {
            Ok(Some(Ok(Message::Text(text)))) => text,
            Ok(Some(Ok(Message::Ping(_) | Message::Pong(_)))) => {
                tokio::time::timeout(Duration::from_secs(30), writer.flush())
                    .await
                    .map_err(|_| ())?
                    .map_err(|_| ())?;
                continue;
            }
            _ => return Ok(()),
        };
        let ctx = prepare_turn(
            &text,
            &state,
            token.clone(),
            query.clone(),
            &uri,
            &headers,
            upstream.clone(),
        );
        let ctx = match ctx {
            Ok(ctx) => ctx,
            Err(error) => {
                send_error(&mut writer, &error.message).await?;
                continue;
            }
        };
        // Signal cancellation inside the pipeline and await failure settlement;
        // dropping an admission future here could strand a token reservation.
        let (cancel, cancelled) = tokio::sync::watch::channel(false);
        let pipeline = crate::pipeline::run_pipeline_cancellable(ctx, &state, cancelled);
        tokio::pin!(pipeline);
        let output = loop {
            tokio::select! {
                output = &mut pipeline => break output,
                message = reader.next() => {
                    if !control(&mut writer, message).await {
                        let _ = cancel.send(true);
                        let _ = pipeline.await;
                        return Ok(());
                    }
                }
            }
        };
        let mut stream = match output {
            Ok(PipelineOutput::Sse(stream)) => stream,
            Ok(_) => {
                send_error(&mut writer, "Expected Responses event stream").await?;
                continue;
            }
            Err(error) => {
                send_error(&mut writer, &error.message).await?;
                continue;
            }
        };
        let mut decoder = BoundedSseDecoder::new(16 * 1024 * 1024 + 1024);
        let mut upstream_failed = false;
        loop {
            let chunk = tokio::select! {
                biased;
                chunk = stream.next() => chunk,
                message = reader.next() => {
                    if !control(&mut writer, message).await { return Ok(()); }
                    continue;
                },
            };
            match chunk {
                Some(Ok(chunk)) => decoder.push_chunk(chunk),
                Some(Err(_)) => {
                    if !upstream_failed {
                        send_error(&mut writer, "Responses WebSocket stream interrupted").await?;
                    }
                    break;
                }
                None => break,
            }
            loop {
                match decoder.decode_next() {
                    DecodeStep::Frame(frame) => {
                        if let Ok(event) = serde_json::from_str::<Value>(&frame.data) {
                            upstream_failed |= matches!(
                                event["type"].as_str(),
                                Some("error" | "response.failed" | "response.incomplete")
                            );
                        }
                        // Await every write: slow clients backpressure upstream.
                        tokio::time::timeout(
                            Duration::from_secs(30),
                            writer.send(Message::Text(frame.data)),
                        )
                        .await
                        .map_err(|_| ())?
                        .map_err(|_| ())?;
                    }
                    DecodeStep::NeedInput => break,
                    DecodeStep::Error(_) => return Err(()),
                }
            }
        }
    }
}

fn prepare_turn(
    text: &str,
    state: &Arc<AppState>,
    token: Option<String>,
    query: HashMap<String, String>,
    uri: &Uri,
    headers: &HeaderMap,
    upstream: NativeResponsesSocket,
) -> Result<PipelineContext, GatewayError> {
    let body = crate::http::native_json_decode::decode(text.as_bytes())
        .map_err(|_| GatewayError::bad_request("Invalid Responses WebSocket JSON"))?;
    if body["type"] != "response.create"
        || body.get("stream").is_some_and(|value| value != true)
        || body.get("background").is_some()
    {
        return Err(GatewayError::bad_request(
            "Expected response.create without background or stream=false",
        ));
    }
    let mut ctx = prepare_context(body, token, query, headers, state)?;
    ctx.stream = true;
    ctx.canonical_req.stream = true;
    ctx.native_wire = Some(Arc::new(NativeRequestWire {
        native_session: None,
        compact: false,
        websocket: Some(upstream),
        connection: None,
        body: bytes::Bytes::copy_from_slice(text.as_bytes()),
        query: uri.query().map(str::to_owned),
        client_request_id: headers.contains_key("x-request-id"),
        header_order: headers
            .keys()
            .map(|name| name.as_str().to_owned())
            .collect(),
    }));
    Ok(ctx)
}

async fn control(
    writer: &mut futures::stream::SplitSink<WebSocket, Message>,
    message: Option<Result<Message, axum::Error>>,
) -> bool {
    if matches!(message, Some(Ok(Message::Ping(_) | Message::Pong(_)))) {
        return matches!(
            tokio::time::timeout(Duration::from_secs(30), writer.flush()).await,
            Ok(Ok(()))
        );
    }
    // This serial lane does not buffer concurrent creates or silently replay
    // cancellations. Any close, binary frame or overlapping create ends the turn.
    false
}

async fn send_error(
    writer: &mut futures::stream::SplitSink<WebSocket, Message>,
    message: &str,
) -> Result<(), ()> {
    let text = json!({"type":"error", "error":{"type":"invalid_request_error", "message":message}})
        .to_string();
    tokio::time::timeout(Duration::from_secs(30), writer.send(Message::Text(text)))
        .await
        .map_err(|_| ())?
        .map_err(|_| ())
}
