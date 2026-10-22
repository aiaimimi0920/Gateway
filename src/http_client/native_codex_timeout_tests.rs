//! Local transport proof: productive streams outlive the idle timeout; stalled ones fail.
use super::*;
use tokio::io::{AsyncReadExt, AsyncWriteExt};

async fn serve(gap: Duration, count: usize) -> (String, tokio::task::JoinHandle<()>) {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("http://{}", listener.local_addr().unwrap());
    let task = tokio::spawn(async move {
        let (mut socket, _) = listener.accept().await.unwrap();
        let mut request = [0; 4096];
        socket.read(&mut request).await.unwrap();
        socket.write_all(b"HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nTransfer-Encoding: chunked\r\nConnection: close\r\n\r\n").await.unwrap();
        for _ in 0..count {
            tokio::time::sleep(gap).await;
            if socket.write_all(b"1\r\nx\r\n").await.is_err() {
                return;
            }
        }
        let _ = socket.write_all(b"0\r\n\r\n").await;
    });
    (url, task)
}

#[tokio::test]
async fn native_codex_timeout_keeps_productive_body_alive() {
    let idle = Duration::from_millis(500);
    let (url, server) = serve(Duration::from_millis(100), 10).await;
    let result = tokio::time::timeout(Duration::from_secs(5), async {
        client(idle).get(url).send().await.unwrap().bytes().await
    })
    .await
    .unwrap();
    server.await.unwrap();
    assert_eq!(result.unwrap().len(), 10);
}

#[tokio::test]
async fn native_codex_timeout_still_bounds_idle_body() {
    let (url, server) = serve(Duration::from_secs(2), 1).await;
    let result = tokio::time::timeout(Duration::from_secs(5), async {
        client(Duration::from_millis(300))
            .get(url)
            .send()
            .await
            .unwrap()
            .bytes()
            .await
    })
    .await
    .unwrap();
    server.abort();
    let _ = server.await;
    assert!(result.unwrap_err().is_timeout());
}
