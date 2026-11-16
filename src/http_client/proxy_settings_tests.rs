use super::*;

#[tokio::test]
async fn disabled_policy_and_custom_loopback_bypass_connect_directly() {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let target = format!("http://{}/", listener.local_addr().unwrap());
    let server = tokio::spawn(async move {
        for _ in 0..4 {
            let (mut socket, _) = listener.accept().await.unwrap();
            let mut buffer = [0u8; 4096];
            let n = socket.read(&mut buffer).await.unwrap();
            assert!(String::from_utf8_lossy(&buffer[..n]).starts_with("GET / HTTP/1.1"));
            socket
                .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 0\r\nConnection: close\r\n\r\n")
                .await
                .unwrap();
        }
    });
    let timeout = std::time::Duration::from_secs(3);
    for policy in [
        Policy {
            mode: Mode::Off,
            url: String::new(),
        },
        Policy {
            mode: Mode::Custom,
            url: "http://127.0.0.1:1".into(),
        },
    ] {
        assert!(rquest(rquest::Client::builder(), &policy)
            .timeout(timeout)
            .build()
            .unwrap()
            .get(&target)
            .send()
            .await
            .unwrap()
            .status()
            .is_success());
        assert!(reqwest(reqwest::Client::builder(), &policy)
            .timeout(timeout)
            .build()
            .unwrap()
            .get(&target)
            .send()
            .await
            .unwrap()
            .status()
            .is_success());
    }
    tokio::time::timeout(timeout, server)
        .await
        .unwrap()
        .unwrap();
}

#[test]
fn validates_and_normalizes_without_persisting_unused_drafts() {
    assert_eq!(
        Policy {
            mode: Mode::Off,
            url: "http://user:secret@host".into()
        }
        .normalized()
        .unwrap()
        .url,
        ""
    );
    assert_eq!(
        Policy {
            mode: Mode::Custom,
            url: " http://localhost:7890 ".into()
        }
        .normalized()
        .unwrap()
        .url,
        "http://localhost:7890/"
    );
    for url in [
        "socks5://localhost:1080",
        "http://user:secret@host",
        "http://host/path",
        "http://host/?secret=x",
        "file:///tmp/proxy",
        "http://host:0",
    ] {
        assert!(Policy {
            mode: Mode::Custom,
            url: url.into()
        }
        .normalized()
        .is_err());
    }
}

#[test]
fn saves_pending_policy_without_mutating_running_clients() {
    let directory = std::env::temp_dir().join(format!("gateway-proxy-{}", uuid::Uuid::new_v4()));
    let initial = active().clone();
    let saved = save(
        &directory,
        Policy {
            mode: Mode::Custom,
            url: "http://127.0.0.1:7890".into(),
        },
    )
    .unwrap();
    assert!(saved.restart_required);
    assert_eq!(active(), &initial);
    assert_eq!(load(&directory).unwrap().url, "http://127.0.0.1:7890/");
    std::fs::write(directory.join("outbound-proxy.json"), b"invalid").unwrap();
    assert!(load(&directory).is_err());
    std::fs::remove_dir_all(directory).unwrap();
}

#[tokio::test]
async fn custom_policy_routes_both_client_families_through_the_selected_proxy() {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let policy = Policy {
        mode: Mode::Custom,
        url: format!("http://{}", listener.local_addr().unwrap()),
    }
    .normalized()
    .unwrap();
    let server = tokio::spawn(async move {
        for _ in 0..2 {
            let (mut socket, _) = listener.accept().await.unwrap();
            let mut buffer = [0u8; 4096];
            let n = socket.read(&mut buffer).await.unwrap();
            assert!(String::from_utf8_lossy(&buffer[..n])
                .starts_with("GET http://proxy-contract.invalid/"));
            socket
                .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\nConnection: close\r\n\r\nok")
                .await
                .unwrap();
        }
    });
    let timeout = std::time::Duration::from_secs(3);
    let response = rquest(rquest::Client::builder(), &policy)
        .timeout(timeout)
        .build()
        .unwrap()
        .get("http://proxy-contract.invalid/")
        .send()
        .await
        .unwrap();
    assert_eq!(response.status().as_u16(), 200);
    let response = reqwest(reqwest::Client::builder(), &policy)
        .timeout(timeout)
        .build()
        .unwrap()
        .get("http://proxy-contract.invalid/")
        .send()
        .await
        .unwrap();
    assert_eq!(response.status().as_u16(), 200);
    tokio::time::timeout(timeout, server)
        .await
        .unwrap()
        .unwrap();
}
