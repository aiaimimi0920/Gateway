//! Exercise the real stream queue: encoder-only tests cannot detect lost request extensions.
use tokio::io::{AsyncReadExt, AsyncWriteExt};

#[tokio::test]
async fn compatibility_marker_survives_stream_queue_and_default_stays_unchanged() {
    for compatible in [false, true] {
        tokio::time::timeout(std::time::Duration::from_secs(5), capture(compatible))
            .await
            .expect("bounded in-memory HTTP/2 exchange");
    }
}

async fn capture(compatible: bool) {
    let (client_io, mut peer) = tokio::io::duplex(16 * 1024);
    let (mut client, connection) = crate::client::handshake(client_io).await.unwrap();
    let driver = tokio::spawn(connection);
    // Dropping this owner aborts the driver on success, panic, or timeout.
    struct Driver(tokio::task::JoinHandle<Result<(), crate::Error>>);
    impl Drop for Driver {
        fn drop(&mut self) {
            self.0.abort();
        }
    }
    let _driver = Driver(driver);
    let mut request = http::Request::builder()
        .uri("https://example.test/v1/messages")
        .header("cookie", "b=2")
        .body(())
        .unwrap();
    if compatible {
        request.extensions_mut().insert(crate::GoCompatibility);
    }
    let (_response, _body) = client.send_request(request, true).unwrap();
    let mut preface = [0u8; 24];
    peer.read_exact(&mut preface).await.unwrap();
    assert_eq!(&preface, b"PRI * HTTP/2.0\r\n\r\nSM\r\n\r\n");
    // Empty SETTINGS from the peer, not a synthetic encoder invocation.
    peer.write_all(&[0, 0, 0, 4, 0, 0, 0, 0, 0]).await.unwrap();
    loop {
        let mut header = [0u8; 9];
        peer.read_exact(&mut header).await.unwrap();
        let size = u32::from_be_bytes([0, header[0], header[1], header[2]]) as usize;
        assert!(size <= 16384);
        let mut payload = vec![0; size];
        peer.read_exact(&mut payload).await.unwrap();
        if header[3] == 1 {
            let indexed_literal_cookie =
                payload.windows(5).any(|v| v == [0x60, 3, b'b', b'=', b'2']);
            assert_eq!(indexed_literal_cookie, compatible);
            break;
        }
    }
}
