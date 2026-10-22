//! Cancellation must release origin ownership; it must not replay a paid request.
use super::*;
use tokio::io::{AsyncReadExt, AsyncWriteExt};

async fn read_head(stream: &mut tokio::net::TcpStream) {
    let mut data = Vec::new();
    while !data.ends_with(b"\r\n\r\n") {
        assert!(data.len() < 16384);
        data.push(stream.read_u8().await.unwrap());
    }
}

#[tokio::test]
async fn header_timeout_releases_origin_for_explicit_next_request() {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let server = tokio::spawn(async move {
        let (mut first, _) = listener.accept().await.unwrap();
        read_head(&mut first).await;
        // Keep the first socket open without returning headers. The client must abandon it.
        let (mut next, _) = tokio::time::timeout(Duration::from_secs(2), listener.accept())
            .await
            .unwrap()
            .unwrap();
        read_head(&mut next).await;
        next.write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\n\r\nok")
            .await
            .unwrap();
    });
    let mut client = Client::new(None, true).unwrap();
    client.proxies = Matcher::builder().build();
    let builder = rquest::Client::builder().no_proxy().build().unwrap();
    let request = || builder.get(format!("http://{address}/")).build().unwrap();
    assert!(matches!(
        client.send(request(), Duration::from_millis(100)).await,
        Err(Error::Timeout)
    ));
    let next = client.send(request(), Duration::from_secs(1)).await;
    let body = match next {
        Ok(response) => response.bytes().await.ok(),
        Err(_) => None,
    };
    server.abort();
    let _ = server.await;
    assert_eq!(body.as_deref(), Some(&b"ok"[..]));
}

#[tokio::test]
async fn abandoned_partial_body_does_not_poison_next_request() {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let server = tokio::spawn(async move {
        let (mut first, _) = listener.accept().await.unwrap();
        read_head(&mut first).await;
        first
            .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 200\r\n\r\nx")
            .await
            .unwrap();
        let (mut next, _) = tokio::time::timeout(Duration::from_secs(2), listener.accept())
            .await
            .unwrap()
            .unwrap();
        read_head(&mut next).await;
        next.write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\n\r\nok")
            .await
            .unwrap();
    });
    let mut client = Client::new(None, true).unwrap();
    client.proxies = Matcher::builder().build();
    let builder = rquest::Client::builder().no_proxy().build().unwrap();
    let request = || builder.get(format!("http://{address}/")).build().unwrap();
    let first = client
        .send(request(), Duration::from_secs(1))
        .await
        .unwrap();
    drop(first);
    let next = client.send(request(), Duration::from_secs(1)).await;
    let body = match next {
        Ok(response) => response.bytes().await.ok(),
        Err(_) => None,
    };
    server.abort();
    let _ = server.await;
    assert_eq!(body.as_deref(), Some(&b"ok"[..]));
}

#[tokio::test]
async fn slow_h1_body_does_not_serialize_an_independent_request() {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let server = tokio::spawn(async move {
        let (mut first, _) = listener.accept().await.unwrap();
        read_head(&mut first).await;
        first
            .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\n\r\n")
            .await
            .unwrap();
        let (mut next, _) = listener.accept().await.unwrap();
        read_head(&mut next).await;
        next.write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\n\r\nok")
            .await
            .unwrap();
        first.write_all(b"ok").await.unwrap();
    });
    let mut client = Client::new(None, true).unwrap();
    client.proxies = Matcher::builder().build();
    let builder = rquest::Client::builder().no_proxy().build().unwrap();
    let request = || builder.get(format!("http://{address}/")).build().unwrap();
    let first = client
        .send(request(), Duration::from_secs(1))
        .await
        .unwrap();
    let next = client.send(request(), Duration::from_secs(1)).await;
    let body = match next {
        Ok(response) => response.bytes().await.ok(),
        Err(_) => None,
    };
    drop(first);
    server.abort();
    let _ = server.await;
    assert_eq!(body.as_deref(), Some(&b"ok"[..]));
}
