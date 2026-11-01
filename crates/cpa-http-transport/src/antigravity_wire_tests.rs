//! Byte-level HTTP/1 regression for the measured native framing; synthetic payloads only.
use super::*;
use tokio::io::{AsyncReadExt, AsyncWriteExt};

#[tokio::test]
async fn antigravity_native_h1_order_and_generation_chunking() {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let server = tokio::spawn(async move {
        let (mut socket, _) = listener.accept().await.unwrap();
        let mut captures = Vec::new();
        for chunked in [false, true] {
            let mut head = Vec::new();
            while !head.ends_with(b"\r\n\r\n") {
                assert!(head.len() < 8192);
                head.push(socket.read_u8().await.unwrap());
            }
            let size = if chunked { 12 } else { 2 };
            let mut body = vec![0; size];
            socket.read_exact(&mut body).await.unwrap();
            captures.push((String::from_utf8(head).unwrap(), body));
            socket
                .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\n\r\n{}")
                .await
                .unwrap();
        }
        captures
    });
    let mut client = Client::with_profile(Profile::AntigravityHub215, None, true).unwrap();
    client.proxies = Matcher::builder().build();
    let builder = rquest::Client::builder().no_proxy().build().unwrap();
    for method in ["loadCodeAssist", "streamGenerateContent?alt=sse"] {
        let request = builder
            .post(format!("http://{address}/v1internal:{method}"))
            .default_headers(false)
            .header("user-agent", "synthetic-native-agent")
            .header("content-type", "application/json")
            .bearer_auth("synthetic")
            .body("{}")
            .build()
            .unwrap();
        client
            .send(request, Duration::from_secs(2))
            .await
            .unwrap()
            .bytes()
            .await
            .unwrap();
    }
    let captures = tokio::time::timeout(Duration::from_secs(3), server)
        .await
        .unwrap()
        .unwrap();
    for ((head, body), chunked) in captures.into_iter().zip([false, true]) {
        let headers: Vec<_> = head
            .split("\r\n")
            .skip(1)
            .filter(|s| !s.is_empty())
            .collect();
        assert_eq!(
            headers,
            vec![
                format!("Host: {address}"),
                "User-Agent: synthetic-native-agent".into(),
                if chunked {
                    "Transfer-Encoding: chunked"
                } else {
                    "Content-Length: 2"
                }
                .into(),
                "Authorization: Bearer synthetic".into(),
                "Content-Type: application/json".into(),
                "Accept-Encoding: gzip".into()
            ]
        );
        assert_eq!(
            body.as_slice(),
            if chunked {
                b"2\r\n{}\r\n0\r\n\r\n".as_slice()
            } else {
                b"{}"
            }
        );
    }
}
