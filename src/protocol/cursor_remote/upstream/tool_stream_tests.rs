//! An inbound control/thinking frame must not cancel an already consumed tool result.
use super::{
    protobuf::{connect_frame, field_bytes, field_str, field_varint},
    tool_stream::execute_tools,
    transport::{
        CheckedClient, DuplexRequest, DuplexStream, Transport, UnaryRequest, UnaryResponse,
    },
    ProbeError, RunError, ToolSpec,
};
use crate::protocol::cursor_remote::runtime::{
    tools::{Bridge, Round},
    worker::WorkerHooks,
};
use bytes::Bytes;
use futures::future::BoxFuture;
use serde_json::json;
use std::sync::{
    atomic::{AtomicBool, AtomicUsize, Ordering},
    Arc,
};
use tokio::sync::{mpsc, oneshot, Notify};

struct Stream {
    incoming: mpsc::Receiver<Bytes>,
    outgoing: mpsc::UnboundedSender<Bytes>,
    observed: Arc<Notify>,
    received: usize,
}

impl DuplexStream for Stream {
    fn status(&self) -> u16 {
        200
    }

    fn next_chunk(&mut self) -> BoxFuture<'_, Result<Option<Bytes>, ProbeError>> {
        Box::pin(async move {
            // Re-entering the read after the second frame proves the decoder consumed it.
            if self.received == 2 {
                self.observed.notify_one();
            }
            let chunk = self.incoming.recv().await;
            self.received += usize::from(chunk.is_some());
            Ok(chunk)
        })
    }

    fn send_frame(&mut self, frame: Bytes) -> Result<(), ProbeError> {
        self.outgoing.send(frame).unwrap();
        Ok(())
    }
}

struct MockTransport(parking_lot::Mutex<Option<Stream>>);

impl Transport for MockTransport {
    fn unary<'a>(
        &'a self,
        _: u16,
        _: UnaryRequest<'a>,
    ) -> BoxFuture<'a, Result<UnaryResponse, ProbeError>> {
        panic!("this fixture exercises only the tool exchange");
    }

    fn open_duplex<'a>(
        &'a self,
        _: u16,
        _: DuplexRequest<'a>,
        hooks: &'a mut WorkerHooks,
    ) -> BoxFuture<'a, Result<Box<dyn DuplexStream>, RunError>> {
        Box::pin(async move {
            hooks.authorize().await.map_err(RunError::Gateway)?;
            hooks.sent().map_err(RunError::Gateway)?;
            Ok(Box::new(self.0.lock().take().unwrap()) as Box<dyn DuplexStream>)
        })
    }
}

fn update(tag: u32, text: &str) -> Vec<u8> {
    connect_frame(
        &field_bytes(1, &field_bytes(tag, &field_str(1, text).unwrap()).unwrap()).unwrap(),
        0,
    )
}

#[tokio::test]
async fn cursor_remote_tool_authorization_survives_inbound_frame() {
    let (incoming, receiver) = mpsc::channel(2);
    let (outgoing, mut sent_frames) = mpsc::unbounded_channel();
    let observed = Arc::new(Notify::new());
    let transport = MockTransport(parking_lot::Mutex::new(Some(Stream {
        incoming: receiver,
        outgoing,
        observed: observed.clone(),
        received: 0,
    })));
    let client = CheckedClient::new(1, &transport);
    let (events, mut phases) = mpsc::channel(1);
    let (commands, command_rx) = mpsc::channel(1);
    let mut hooks = WorkerHooks::new(
        Box::new(|| Box::pin(async { Ok(()) })),
        Box::new(|| {}),
        Arc::new(AtomicBool::new(false)),
        Some(Bridge {
            events,
            commands: command_rx,
        }),
    );
    let tools = [ToolSpec {
        name: "echo".into(),
        description: String::new(),
        parameters: json!({"type":"object"}),
    }];
    let continued = Arc::new(AtomicUsize::new(0));
    let count = continued.clone();
    let peer = async move {
        let mut mcp = field_str(1, "mcp_gateway_echo").unwrap();
        mcp.extend(field_str(3, "call_one").unwrap());
        mcp.extend(field_str(4, "gateway").unwrap());
        mcp.extend(field_str(5, "echo").unwrap());
        let mut exec = field_varint(1, 1).unwrap();
        exec.extend(field_bytes(11, &mcp).unwrap());
        incoming
            .send(Bytes::from(connect_frame(
                &field_bytes(2, &exec).unwrap(),
                0,
            )))
            .await
            .unwrap();
        assert_eq!(
            phases.recv().await.unwrap().unwrap().call.unwrap().id,
            "call_one"
        );
        let (started, authorization_started) = oneshot::channel();
        let (release, released) = oneshot::channel();
        commands
            .send(Round {
                call_id: "call_one".into(),
                output: "tool output".into(),
                authorize: Box::new(move || {
                    Box::pin(async move {
                        started.send(()).unwrap();
                        released.await.unwrap();
                        Ok(())
                    })
                }),
                sent: Box::new(move || {
                    count.fetch_add(1, Ordering::SeqCst);
                }),
                uncertain: Arc::new(AtomicBool::new(false)),
            })
            .await
            .unwrap();
        authorization_started.await.unwrap();
        incoming
            .send(Bytes::from(update(4, "during authorization")))
            .await
            .unwrap();
        observed.notified().await;
        // The receiver must still be owned by the original, pending authorization.
        assert!(
            release.send(()).is_ok(),
            "inbound frame discarded the tool authorization"
        );
        let frame = sent_frames.recv().await.unwrap();
        assert!(frame
            .windows(b"tool output".len())
            .any(|s| s == b"tool output"));
        let mut final_chunk = update(1, "done");
        final_chunk.extend(connect_frame(
            &field_bytes(1, &field_bytes(14, &[]).unwrap()).unwrap(),
            0,
        ));
        final_chunk.extend(connect_frame(b"{}", 2));
        incoming.send(Bytes::from(final_chunk)).await.unwrap();
    };
    let (result, ()) = tokio::time::timeout(std::time::Duration::from_secs(2), async {
        tokio::join!(
            execute_tools(
                "synthetic",
                "default".into(),
                "hello",
                &[],
                &tools,
                &client,
                &mut hooks
            ),
            peer
        )
    })
    .await
    .expect("tool continuation stalled");
    let output = result.map_err(RunError::into_gateway).unwrap();
    assert_eq!(output.text, "done");
    assert_eq!(output.thinking.as_deref(), Some("during authorization"));
    assert_eq!(continued.load(Ordering::SeqCst), 1);
}
