use std::time::Duration;

use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::oneshot;
use tokio::task::JoinHandle;

use crate::protocol::upstream_body::MAX_ACCUMULATED_UPSTREAM_BODY_BYTES as LIMIT;

pub(super) enum BodyMode {
    DeclaredOversize,
    ChunkedOversize,
    ExactLimit,
    Finite(&'static [u8]),
    HeldSse,
}

pub(super) struct WireServer {
    pub(super) url: String,
    task: JoinHandle<()>,
    complete: Option<oneshot::Sender<()>>,
}

impl WireServer {
    pub(super) async fn start(
        path: &'static str,
        status: u16,
        content_type: &'static str,
        mode: BodyMode,
    ) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind");
        let url = format!("http://{}", listener.local_addr().expect("address"));
        let (complete, release) = oneshot::channel();
        let task = tokio::spawn(async move {
            let (mut socket, _) = listener.accept().await.expect("accept");
            let mut request = Vec::new();
            let mut buffer = [0; 4096];
            while !request.windows(4).any(|value| value == b"\r\n\r\n") {
                let length = socket.read(&mut buffer).await.expect("request");
                assert!(length > 0 && request.len() + length <= 64 * 1024);
                request.extend_from_slice(&buffer[..length]);
            }
            let header = String::from_utf8_lossy(&request);
            assert_eq!(
                header.lines().next().unwrap().split_whitespace().nth(1),
                Some(path)
            );
            let framing = match &mode {
                BodyMode::DeclaredOversize => format!("Content-Length: {}", LIMIT + 1),
                BodyMode::ExactLimit => format!("Content-Length: {LIMIT}"),
                BodyMode::Finite(body) => format!("Content-Length: {}", body.len()),
                _ => "Transfer-Encoding: chunked".to_string(),
            };
            let headers = format!("HTTP/1.1 {status} Test\r\nContent-Type: {content_type}\r\n{framing}\r\nConnection: close\r\n\r\n");
            socket.write_all(headers.as_bytes()).await.expect("headers");
            match mode {
                BodyMode::DeclaredOversize => std::future::pending::<()>().await,
                BodyMode::ChunkedOversize => {
                    if !write_padding(&mut socket, LIMIT + 1, true).await {
                        return;
                    }
                    // Do not send the terminal chunk: rejection must precede EOF.
                    std::future::pending::<()>().await;
                }
                BodyMode::ExactLimit => {
                    write_padding(&mut socket, LIMIT, false).await;
                }
                BodyMode::Finite(body) => {
                    socket.write_all(body).await.expect("finite body");
                }
                BodyMode::HeldSse => {
                    send_chunk(&mut socket, b"data: {\"p\":\"/message/content/parts/0\",\"o\":\"append\",\"v\":\"Paris\"}\n\n").await.expect("first SSE");
                    release.await.expect("test releases terminal SSE");
                    send_chunk(&mut socket, b"data: [DONE]\n\n")
                        .await
                        .expect("terminal SSE");
                    socket
                        .write_all(b"0\r\n\r\n")
                        .await
                        .expect("terminal chunk");
                }
            }
        });
        Self {
            url,
            task,
            complete: Some(complete),
        }
    }

    pub(super) fn complete(&mut self) {
        self.complete
            .take()
            .expect("completion sender")
            .send(())
            .expect("live server");
    }

    pub(super) async fn stop(mut self) {
        self.task.abort();
        match tokio::time::timeout(Duration::from_secs(5), &mut self.task)
            .await
            .expect("server cleanup")
        {
            Ok(()) => (),
            Err(error) => assert!(error.is_cancelled(), "server task failed: {error}"),
        }
    }
}

impl Drop for WireServer {
    fn drop(&mut self) {
        self.task.abort();
    }
}

async fn send_chunk(socket: &mut TcpStream, body: &[u8]) -> std::io::Result<()> {
    socket
        .write_all(format!("{:x}\r\n", body.len()).as_bytes())
        .await?;
    socket.write_all(body).await?;
    socket.write_all(b"\r\n").await
}

async fn write_padding(socket: &mut TcpStream, length: usize, chunked: bool) -> bool {
    let chunk = [b' '; 64 * 1024];
    let mut remaining = length;
    while remaining > 0 {
        let count = remaining.min(chunk.len());
        let result = if chunked {
            send_chunk(socket, &chunk[..count]).await
        } else {
            socket.write_all(&chunk[..count]).await
        };
        if result.is_err() {
            return false;
        }
        remaining -= count;
    }
    true
}
