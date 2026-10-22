//! Proxy rejection and bounded-read contracts, using no external network or secrets.
use crate::{dial::Io, proxy, Error};
use tokio::io::{AsyncReadExt, AsyncWriteExt};

#[tokio::test]
async fn connect_status_and_header_limit() {
    for (reply, accepted) in [
        (b"HTTP/1.1 200\r\n\r\n".to_vec(), true),
        (b"HTTP/1.0 200 OK\r\n\r\n".to_vec(), true),
        (b"HTTP/1.1 407 Required\r\n\r\n".to_vec(), false),
        (b"HTTP/1.1 2000 Invalid\r\n\r\n".to_vec(), false),
        (vec![b'x'; 16384], false),
        (b"HTTP/1.1 200".to_vec(), false),
    ] {
        let (client, mut server) = tokio::io::duplex(32768);
        let task = tokio::spawn(async move {
            let mut head = Vec::new();
            while !head.ends_with(b"\r\n\r\n") {
                head.push(server.read_u8().await.unwrap());
            }
            assert!(head.starts_with(b"CONNECT example.test:443 HTTP/1.1\r\n"));
            server.write_all(&reply).await.unwrap();
        });
        let mut io: Io = Box::new(client);
        assert_eq!(
            proxy::connect(&mut io, &"https://example.test".parse().unwrap(), None)
                .await
                .is_ok(),
            accepted
        );
        task.await.unwrap();
    }
}

#[tokio::test]
async fn socks_rejects_bad_methods_auth_and_truncated_reply() {
    for reply in [
        vec![4, 0],
        vec![5, 255],
        vec![5, 2, 1, 1],
        vec![5, 0, 5, 1, 0, 1],
        vec![5, 0, 5, 0, 0, 1, 127],
    ] {
        let (client, mut server) = tokio::io::duplex(1024);
        let task = tokio::spawn(async move {
            let mut greeting = [0; 4];
            server.read_exact(&mut greeting).await.unwrap();
            assert_eq!(greeting, [5, 2, 0, 2]);
            server.write_all(&reply).await.unwrap();
            server.shutdown().await.unwrap();
            let mut rest = Vec::new();
            server.read_to_end(&mut rest).await.unwrap();
        });
        let mut io: Io = Box::new(client);
        assert!(proxy::socks5(
            &mut io,
            &"https://example.test".parse().unwrap(),
            Some(("user", "synthetic"))
        )
        .await
        .is_err());
        drop(io);
        task.await.unwrap();
    }
}

#[tokio::test]
async fn socks_credentials_are_bounded_before_writing() {
    let (client, _server) = tokio::io::duplex(16);
    let mut io: Io = Box::new(client);
    let long = "x".repeat(256);
    for auth in [
        ("", "password"),
        (long.as_str(), "password"),
        ("user", long.as_str()),
    ] {
        assert_eq!(
            proxy::socks5(
                &mut io,
                &"https://example.test".parse().unwrap(),
                Some(auth)
            )
            .await,
            Err(Error::UnsupportedProxy)
        );
    }
}

#[tokio::test]
async fn stalled_connect_can_be_cancelled_and_drops_socket() {
    let (client, mut server) = tokio::io::duplex(1024);
    let task = tokio::spawn(async move {
        let mut io: Io = Box::new(client);
        proxy::connect(&mut io, &"https://example.test".parse().unwrap(), None).await
    });
    let mut head = Vec::new();
    while !head.ends_with(b"\r\n\r\n") {
        head.push(server.read_u8().await.unwrap());
    }
    task.abort();
    assert!(task.await.unwrap_err().is_cancelled());
    assert_eq!(
        server.read_u8().await.unwrap_err().kind(),
        std::io::ErrorKind::UnexpectedEof
    );
}
