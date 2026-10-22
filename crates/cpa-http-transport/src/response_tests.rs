//! Loopback response tests exercise the real parser, body bridge and decompression chain.
use super::*;
use http::{header, Method};
use std::io::Write;
use tokio::io::{AsyncReadExt, AsyncWriteExt};

fn gzip(input: &[u8]) -> Vec<u8> {
    let mut writer = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::default());
    writer.write_all(input).unwrap();
    writer.finish().unwrap()
}

async fn exchange(method: Method, status: u16, headers: &str, body: Vec<u8>) -> rquest::Response {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let head = format!("HTTP/1.1 {status} Test\r\n{headers}Connection: close\r\n\r\n");
    let server = tokio::spawn(async move {
        let (mut stream, _) = listener.accept().await.unwrap();
        let mut incoming = Vec::new();
        while !incoming.ends_with(b"\r\n\r\n") {
            assert!(incoming.len() < 16384);
            incoming.push(stream.read_u8().await.unwrap());
        }
        stream.write_all(head.as_bytes()).await.unwrap();
        stream.write_all(&body).await.unwrap();
    });
    // Test-only no-proxy selection; do not mutate process-wide proxy variables.
    let mut client = Client::new(None, true).unwrap();
    client.proxies = Matcher::builder().build();
    let request = rquest::Client::builder()
        .no_proxy()
        .build()
        .unwrap()
        .request(method, format!("http://{address}/"))
        .build()
        .unwrap();
    let response = client.send(request, Duration::from_secs(5)).await.unwrap();
    server.await.unwrap();
    response
}

#[tokio::test]
async fn no_body_responses_preserve_representation_metadata() {
    for (method, status) in [(Method::HEAD, 200), (Method::GET, 304), (Method::GET, 204)] {
        let response = exchange(method, status, "Content-Encoding: gzip\r\n", Vec::new()).await;
        assert_eq!(
            response.headers().get(header::CONTENT_ENCODING).unwrap(),
            "gzip"
        );
        assert!(response.bytes().await.unwrap().is_empty());
    }
}

#[tokio::test]
async fn repeated_encoding_fields_decode_in_reverse_order() {
    let response = exchange(
        Method::GET,
        200,
        "Content-Encoding: gzip\r\nContent-Encoding: gzip\r\n",
        gzip(&gzip(b"layered")),
    )
    .await;
    assert!(!response.headers().contains_key(header::CONTENT_ENCODING));
    assert_eq!(&response.bytes().await.unwrap()[..], b"layered");
}

#[tokio::test]
async fn gzip_decodes_all_members() {
    let mut body = gzip(b"first");
    body.extend(gzip(b"second"));
    let response = exchange(Method::GET, 200, "Content-Encoding: gzip\r\n", body).await;
    assert_eq!(&response.bytes().await.unwrap()[..], b"firstsecond");
}

#[tokio::test]
async fn unknown_encoding_keeps_entire_chain_opaque() {
    let body = gzip(b"opaque");
    let response = exchange(
        Method::GET,
        200,
        "Content-Encoding: gzip\r\nContent-Encoding: future-encoding\r\n",
        body.clone(),
    )
    .await;
    assert_eq!(
        response
            .headers()
            .get_all(header::CONTENT_ENCODING)
            .iter()
            .count(),
        2
    );
    assert_eq!(&response.bytes().await.unwrap()[..], body);
}
