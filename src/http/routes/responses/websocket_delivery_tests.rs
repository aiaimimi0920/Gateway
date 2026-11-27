//! Real loopback WS delivery with synthetic events and an owned paused worker.
//! This exercises the production relay, not Cursor's external service.
use super::*;
use crate::{
    pipeline::PipelineContext,
    protocol::cursor_remote::{
        runtime::{
            worker::{WorkerOutput, WorkerToolCall},
            Runtime,
        },
        tool_request,
    },
};
use axum::{extract::ws::WebSocketUpgrade, routing::get, Router};
use serde_json::json;
use std::sync::{atomic::AtomicBool, Arc};
use tokio::sync::{mpsc, watch};
use tokio::time::{timeout, Instant};

fn paused_runtime() -> Runtime {
    Runtime::fixture(|_input, hooks| {
        Box::pin(async move {
            hooks.authorize().await?;
            hooks.sent()?;
            let _ = hooks
                .tool(WorkerToolCall {
                    call_id: "call_delivery".into(),
                    name: "echo".into(),
                    arguments: "{}".into(),
                    text: String::new(),
                    thinking: None,
                    requested_model: "default".into(),
                })
                .await?;
            Ok(WorkerOutput {
                usage: None,
                text: "done".into(),
                thinking: None,
                requested_model: "default".into(),
            })
        })
    })
}

#[derive(Clone, Copy, Debug)]
enum Case {
    Clean,
    MissingGuard,
    MissingId,
    ClosedGuard,
    TrailingFrame,
    InvalidUtf8,
    Truncated,
}
async fn paused(
    rt: &Runtime,
    case: Case,
) -> (cursor_websocket::Connection, cursor_websocket::Turn) {
    let req = crate::protocol::responses::normalize_responses(json!({
        "model":"cursor-personal","store":false,"input":"hello",
        "tools":[{"type":"function","name":"echo","parameters":{"type":"object"}}]
    }))
    .unwrap();
    let request = tool_request::parse(&req).unwrap();
    let mut ctx = PipelineContext::new(req, None);
    let connection = cursor_websocket::Connection::new();
    let turn = connection.begin(&mut ctx).unwrap();
    let mut result = rt
        .execute_tools(
            "delivery".into(),
            request,
            json!({}),
            Instant::now() + Duration::from_secs(5),
            Box::new(|| Box::pin(async { Ok(()) })),
            Box::new(|| {}),
            watch::channel(false).0,
            Arc::new(AtomicBool::new(false)),
        )
        .await
        .unwrap();
    let guard = result.publication.take().unwrap();
    if matches!(case, Case::MissingGuard) {
        drop(guard);
    } else {
        ctx.cursor_tool_publication
            .take()
            .unwrap()
            .send(guard)
            .unwrap();
    }
    if matches!(case, Case::ClosedGuard) {
        rt.close();
    }
    (connection, turn)
}

fn events(case: Case) -> Vec<Result<Bytes, StreamError<rquest::Error>>> {
    let mut completed = json!({"type":"response.completed","response":{"id":"resp_delivery","status":"completed",
        "output":[{"type":"function_call","call_id":"call_delivery","name":"echo","arguments":"{}"}]}});
    if matches!(case, Case::MissingId) {
        completed["response"].as_object_mut().unwrap().remove("id");
    }
    let mut frames = vec![
        Ok(Bytes::from_static(
            b"data: {\"type\":\"response.created\"}\n\n",
        )),
        Ok(Bytes::from_static(
            b"data: {\"type\":\"response.reasoning_text.delta\",\"item_id\":\"rs_delivery\",\"output_index\":0,\"content_index\":0,\"delta\":\"public\"}\n\n",
        )),
        Ok(Bytes::from(format!("data: {completed}\n\n"))),
    ];
    match case {
        Case::TrailingFrame => frames.push(Ok(Bytes::from_static(
            b"data: {\"type\":\"response.output_text.delta\",\"delta\":\"late\"}\n\n",
        ))),
        Case::InvalidUtf8 => frames.push(Ok(Bytes::from_static(b"data: \xff\n\n"))),
        Case::Truncated => frames.push(Ok(Bytes::from_static(b"data: {"))),
        _ => {}
    }
    frames
}

struct Server(tokio::task::JoinHandle<()>);
impl Drop for Server {
    fn drop(&mut self) {
        self.0.abort();
    }
}

async fn check(case: Case) {
    let rt = Arc::new(paused_runtime());
    let (tx, mut rx) = mpsc::channel(1);
    let fixture = rt.clone();
    let router = Router::new().route(
        "/events",
        get(move |upgrade: WebSocketUpgrade| {
            let rt = fixture.clone();
            let tx = tx.clone();
            async move {
                upgrade.on_upgrade(move |socket| async move {
                    let (mut connection, turn) = paused(&rt, case).await;
                    let (mut writer, mut reader) = socket.split();
                    let result = relay(
                        &mut writer,
                        &mut reader,
                        futures::stream::iter(events(case)),
                        Some(turn),
                        &mut connection,
                    )
                    .await;
                    drop(connection);
                    // Dropping the real socket is the failure signal; no fake completion.
                    drop((writer, reader));
                    let _ = tx.send(result.is_ok()).await;
                })
            }
        }),
    );
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("ws://{}/events", listener.local_addr().unwrap());
    let server = Server(tokio::spawn(async move {
        axum::serve(listener, router).await.unwrap()
    }));
    let outcome = timeout(Duration::from_secs(10), async {
        let (mut socket, _) = tokio_tungstenite::connect_async(url).await.unwrap();
        let mut received = Vec::new();
        while let Some(Ok(message)) = socket.next().await {
            if message.is_text() {
                received.push(serde_json::from_str::<Value>(message.to_text().unwrap()).unwrap());
            }
        }
        let clean = matches!(case, Case::Clean);
        assert_eq!(rx.recv().await, Some(clean), "{case:?}");
        assert!(received
            .iter()
            .any(|event| event["type"] == "response.created"));
        assert!(received
            .iter()
            .any(|event| event["type"] == "response.reasoning_text.delta"));
        assert_eq!(
            received
                .iter()
                .filter(|event| event["type"] == "response.completed")
                .count(),
            usize::from(clean),
            "{case:?}"
        );
        if clean {
            assert_eq!(received.last().unwrap()["response"]["id"], "resp_delivery");
        }
        rt.fixture_wait_idle().await;
        if !matches!(case, Case::ClosedGuard) {
            assert!(rt.available());
        }
    })
    .await;
    server.0.abort();
    timeout(Duration::from_secs(5), rt.join()).await.unwrap();
    outcome.unwrap();
}

#[tokio::test]
async fn cursor_websocket_delivery_withholds_invalid_completion_and_reaps_pause() {
    for case in [
        Case::Clean,
        Case::MissingGuard,
        Case::MissingId,
        Case::ClosedGuard,
        Case::TrailingFrame,
        Case::InvalidUtf8,
        Case::Truncated,
    ] {
        check(case).await;
    }
}
