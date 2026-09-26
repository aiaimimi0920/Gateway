use std::io;
use std::time::Duration;

use crate::routing::candidate::ProviderAccountPayload;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;
use tokio::task::JoinHandle;

pub(super) const SECRET: &str = "quota-contract-private-marker";

pub(super) enum Reply {
    Disconnect,
    TruncatedBody,
    Http(u16, &'static str),
}

pub(super) struct LoopbackFixture {
    pub(super) base_url: String,
    server: JoinHandle<io::Result<String>>,
}

impl LoopbackFixture {
    pub(super) async fn new(reply: Reply) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let base_url = format!("http://{}", listener.local_addr().unwrap());
        let server = tokio::spawn(async move {
            tokio::time::timeout(Duration::from_secs(5), serve(listener, reply))
                .await
                .expect("loopback fixture deadline")
        });
        Self { base_url, server }
    }

    pub(super) async fn finish(&mut self) -> String {
        tokio::time::timeout(Duration::from_secs(6), &mut self.server)
            .await
            .expect("fixture completion deadline")
            .expect("fixture task did not panic")
            .expect("fixture IO completed")
    }

    pub(super) fn payload(&self, adapter: &str) -> ProviderAccountPayload {
        serde_json::from_value(serde_json::json!({
            "adapter": adapter,
            "base_url": self.base_url,
            "api_key": SECRET,
            "balance_path": format!("/balance?accessToken={SECRET}"),
            "headers": {
                "utdid": "quota-contract-device",
                "version": "0.5.6",
                "Cookie": "cna=quota-contract-cna",
            },
        }))
        .unwrap()
    }
}

impl Drop for LoopbackFixture {
    fn drop(&mut self) {
        // Abort only the task that owns this fixture's listener and socket.
        self.server.abort();
    }
}

async fn serve(listener: TcpListener, reply: Reply) -> io::Result<String> {
    let (mut stream, _) = listener.accept().await?;
    drop(listener);
    let mut request = Vec::new();
    let mut chunk = [0_u8; 1024];
    loop {
        let read = stream.read(&mut chunk).await?;
        if read == 0 || request.len() + read > 8192 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "incomplete or oversized request",
            ));
        }
        request.extend_from_slice(&chunk[..read]);
        if request.windows(4).any(|part| part == b"\r\n\r\n") {
            break;
        }
    }
    match reply {
        Reply::Disconnect => {}
        Reply::TruncatedBody => {
            stream
                .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 64\r\nConnection: close\r\n\r\n{")
                .await?;
        }
        Reply::Http(status, body) => {
            let response = format!(
                "HTTP/1.1 {status} fixture\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                body.len()
            );
            stream.write_all(response.as_bytes()).await?;
        }
    }
    stream.shutdown().await?;
    String::from_utf8(request).map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))
}
