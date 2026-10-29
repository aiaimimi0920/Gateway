use super::*;

#[test]
fn public_only_by_default_and_explicit_private_never_permits_metadata() {
    for ip in ["127.0.0.1", "10.0.0.1", "192.168.1.1", "::1", "fd00::1"] {
        assert!(!allowed_ip(ip.parse().unwrap(), false), "{ip}");
        assert!(allowed_ip(ip.parse().unwrap(), true), "{ip}");
    }
    for ip in [
        "169.254.169.254",
        "100.100.100.200",
        "0.0.0.0",
        "224.0.0.1",
        "192.0.2.1",
        "fe80::1",
        "::ffff:169.254.169.254",
        "2002:7f00:1::1",
        "2001::1",
        "198.18.0.1",
        "198.19.255.255",
        "::ffff:198.18.0.1",
    ] {
        assert!(!allowed_ip(ip.parse().unwrap(), false), "{ip}");
        assert!(!allowed_ip(ip.parse().unwrap(), true), "{ip}");
    }
    for ip in ["1.1.1.1", "8.8.8.8", "2606:4700:4700::1111"] {
        assert!(allowed_ip(ip.parse().unwrap(), false), "{ip}");
    }
}

#[tokio::test]
async fn literal_fake_ips_and_metadata_never_enter_dns_recovery_or_connect() {
    for ip in [
        "198.18.0.1",
        "198.19.255.255",
        "169.254.169.254",
        "100.100.100.200",
    ] {
        let url = checked_url(&format!("https://{ip}/")).unwrap();
        for private in [false, true] {
            assert!(client(&url, private).await.is_err(), "{ip}");
        }
    }
    assert!(official_client(super::super::HealthAdapter::Auto)
        .await
        .is_err());
}

#[tokio::test]
async fn custom_transport_bypasses_system_proxy_and_does_not_follow_redirects() {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    for target in [
        "http://127.0.0.1/",
        "http://169.254.169.254/",
        "https://example.com/",
    ] {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let url = checked_url(&format!("http://{}/", listener.local_addr().unwrap())).unwrap();
        let response = format!("HTTP/1.1 302 Found\r\nLocation: {target}\r\nContent-Length: 0\r\nConnection: close\r\n\r\n");
        let server = tokio::spawn(async move {
            let (mut socket, _) = listener.accept().await.unwrap();
            let mut request = [0; 4096];
            socket.read(&mut request).await.unwrap();
            socket.write_all(response.as_bytes()).await.unwrap();
        });
        let client = client(&url, true).await.unwrap();
        let error = json(
            &client,
            url.as_str(),
            None,
            &super::super::MonitoringAuth::default(),
        )
        .await
        .unwrap_err();
        assert_eq!(error, ("http_error".into(), Some(302)));
        server.await.unwrap();
    }
}

#[test]
fn untrusted_site_url_cannot_carry_credentials_or_url_side_channels() {
    for url in [
        "file:///etc/passwd",
        "https://u:p@example.com/",
        "https://example.com/?secret=x",
        "https://example.com/#fragment",
    ] {
        assert!(checked_url(url).is_err(), "{url}");
    }
}
