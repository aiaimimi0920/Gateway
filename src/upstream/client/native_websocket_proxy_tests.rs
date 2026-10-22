//! CONNECT framing, authentication separation and bounded failure contracts.
use super::*;

#[tokio::test]
async fn native_websocket_connect_preserves_following_bytes_and_proxy_auth() {
    let (client, mut server) = tokio::io::duplex(16384);
    let mut client: Io = Box::new(client);
    let auth = http::HeaderValue::from_static("Basic cHJveHk6c2VjcmV0");
    let peer = async {
        let mut request = Vec::new();
        while !request.ends_with(b"\r\n\r\n") {
            request.push(server.read_u8().await.unwrap());
        }
        assert_eq!(String::from_utf8(request).unwrap(), "CONNECT [::1]:443 HTTP/1.1\r\nHost: [::1]:443\r\nProxy-Authorization: Basic cHJveHk6c2VjcmV0\r\n\r\n");
        server
            .write_all(b"HTTP/1.1 200 Connection established\r\n\r\nnext")
            .await
            .unwrap();
    };
    let (result, _) = tokio::join!(tunnel(&mut client, "[::1]", 443, Some(&auth)), peer);
    result.unwrap();
    let mut next = [0; 4];
    client.read_exact(&mut next).await.unwrap();
    assert_eq!(&next, b"next");
}

#[tokio::test]
async fn native_websocket_connect_rejects_refusal_and_oversized_headers() {
    for response in [
        b"HTTP/1.1 407 Proxy Authentication Required\r\n\r\n".to_vec(),
        vec![b'x'; 8192],
    ] {
        let (client, mut server) = tokio::io::duplex(16384);
        let mut client: Io = Box::new(client);
        let peer = async {
            let mut request = Vec::new();
            while !request.ends_with(b"\r\n\r\n") {
                request.push(server.read_u8().await.unwrap());
            }
            assert!(!String::from_utf8(request)
                .unwrap()
                .contains("Authorization"));
            server.write_all(&response).await.unwrap();
        };
        let (result, _) = tokio::join!(tunnel(&mut client, "example.test", 443, None), peer);
        assert!(result.is_err());
    }
}

#[test]
fn native_websocket_proxy_matcher_honors_no_proxy_and_credentials() {
    let matcher = Matcher::builder()
        .all("https://user:secret@proxy.test:8443")
        .no("localhost,127.0.0.1")
        .build();
    assert!(matcher
        .intercept(&"https://localhost/v1/responses".parse().unwrap())
        .is_none());
    let proxy = matcher
        .intercept(&"https://origin.test/v1/responses".parse().unwrap())
        .unwrap();
    assert_eq!(
        proxy.uri(),
        &"https://proxy.test:8443".parse::<http::Uri>().unwrap()
    );
    assert!(proxy.basic_auth().is_some());
    assert!(!proxy.uri().to_string().contains("secret"));
}
