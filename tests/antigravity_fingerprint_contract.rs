//! First-flight capture at the production Antigravity owner. No trusted peer or Google traffic.
#![cfg(feature = "cpa-rust-transport")]
use neuro_gateway::{routing::candidate::ProviderAccountPayload, upstream::antigravity};
use std::time::Duration;
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::TcpListener,
};

fn take<'a>(input: &mut &'a [u8], count: usize) -> &'a [u8] {
    assert!(count <= input.len());
    let (head, tail) = input.split_at(count);
    *input = tail;
    head
}
fn word(input: &mut &[u8]) -> usize {
    u16::from_be_bytes(take(input, 2).try_into().unwrap()) as usize
}
fn vector(input: &mut &[u8]) -> Vec<u16> {
    let size = word(input);
    take(input, size)
        .chunks_exact(2)
        .map(|v| u16::from_be_bytes(v.try_into().unwrap()))
        .collect()
}
fn shape(record: &[u8]) -> serde_json::Value {
    assert_eq!(record[0], 22);
    let mut input = &record[9..];
    let version = word(&mut input);
    take(&mut input, 32);
    let session = take(&mut input, 1)[0] as usize;
    take(&mut input, session);
    let ciphers = vector(&mut input);
    let compression = take(&mut input, 1)[0] as usize;
    let compression = take(&mut input, compression).to_vec();
    assert_eq!(word(&mut input), input.len());
    let mut extensions = Vec::new();
    let mut bodies = serde_json::Map::new();
    while !input.is_empty() {
        let kind = word(&mut input);
        let size = word(&mut input);
        let mut body = take(&mut input, size);
        extensions.push(kind);
        let value = match kind {
            10 | 13 | 50 => serde_json::json!(vector(&mut body)),
            51 => {
                assert_eq!(word(&mut body), body.len());
                let mut shares = Vec::new();
                while !body.is_empty() {
                    let group = word(&mut body);
                    let size = word(&mut body);
                    take(&mut body, size);
                    shares.push([group, size]);
                }
                serde_json::json!(shares)
            }
            _ => serde_json::json!(hex::encode(body)),
        };
        bodies.insert(kind.to_string(), value);
    }
    serde_json::json!({"record_length":record.len(),"record_version":hex::encode(&record[1..3]),
        "legacy_version":version,"session_id_length":session,"compression":compression,"ciphers":ciphers,
        "extensions":extensions,"bodies":bodies})
}
async fn receive(listener: &TcpListener) -> Vec<u8> {
    let (mut socket, address) = listener.accept().await.unwrap();
    assert!(address.ip().is_loopback());
    let mut connect = Vec::new();
    while !connect.ends_with(b"\r\n\r\n") {
        assert!(connect.len() < 8192);
        connect.push(socket.read_u8().await.unwrap());
    }
    assert!(connect.starts_with(b"CONNECT daily-cloudcode-pa.googleapis.com:443 "));
    socket
        .write_all(b"HTTP/1.1 200 Connection established\r\n\r\n")
        .await
        .unwrap();
    let mut header = [0; 5];
    socket.read_exact(&mut header).await.unwrap();
    let size = u16::from_be_bytes([header[3], header[4]]) as usize;
    assert!(size <= 16384);
    let mut record = header.to_vec();
    record.resize(size + 5, 0);
    socket.read_exact(&mut record[5..]).await.unwrap();
    record
}

#[tokio::test]
async fn antigravity_first_flight_matches_installed_native_client() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let proxy = format!("http://{}", listener.local_addr().unwrap());
    // Only this integration-test process is affected. Both old and native transports use this proxy.
    std::env::set_var("HTTPS_PROXY", &proxy);
    std::env::set_var("NO_PROXY", "127.0.0.1,localhost");
    let client = rquest::Client::builder()
        .no_proxy()
        .proxy(rquest::Proxy::all(proxy).unwrap())
        .build()
        .unwrap();
    let payload: ProviderAccountPayload = serde_json::from_value(serde_json::json!({
        "adapter":"antigravity_oauth_compatible", "base_url":"https://daily-cloudcode-pa.googleapis.com/v1internal", "api_key":"synthetic-only"})).unwrap();
    let expected: serde_json::Value = serde_json::from_str(include_str!(
        "fixtures/antigravity/native-hub-2.15.0-client-hello.json"
    ))
    .unwrap();
    for _ in 0..3 {
        let (result, record) = tokio::time::timeout(Duration::from_secs(8), async {
            tokio::join!(antigravity::probe(&client, &payload), receive(&listener))
        })
        .await
        .expect("bounded ClientHello capture");
        assert!(result.is_err(), "fixture intentionally never completes TLS");
        if let Some(path) = std::env::var_os("ANTIGRAVITY_HELLO_ARTIFACT") {
            std::fs::write(path, &record).unwrap();
        }
        assert_eq!(shape(&record), expected);
    }
}
