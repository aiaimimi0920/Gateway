//! 纯来源测试不读实际环境/注册表；系统回落 wire 测试只用合成 loopback。
use super::*;

fn environment(values: &[(&str, &str)], cgi: bool) -> Addresses {
    environment_addresses(
        |key| {
            values
                .iter()
                .find(|(name, _)| *name == key)
                .map(|(_, value)| value.to_string())
        },
        cgi,
    )
}

#[test]
fn uppercase_valid_then_lowercase_then_all_with_protocol_overrides() {
    let selected = environment(
        &[
            ("ALL_PROXY", "all.invalid:8000"),
            ("all_proxy", "lower-all.invalid:8001"),
            ("HTTP_PROXY", "http://upper.invalid:8002"),
            ("http_proxy", "http://lower.invalid:8003"),
            ("HTTPS_PROXY", "http://secure.invalid:8004"),
        ],
        false,
    );
    assert_eq!(selected.http.as_deref(), Some("http://upper.invalid:8002/"));
    assert_eq!(
        selected.https.as_deref(),
        Some("http://secure.invalid:8004/")
    );
    for invalid in [
        "",
        "  ",
        "http://[",
        "ftp://unsupported.invalid",
        "socks5://unsupported.invalid",
    ] {
        let selected = environment(
            &[
                ("HTTP_PROXY", invalid),
                ("http_proxy", "lower.invalid:8003"),
            ],
            false,
        );
        assert_eq!(selected.http.as_deref(), Some("http://lower.invalid:8003/"));
        let selected = environment(
            &[("ALL_PROXY", "all.invalid:8000"), ("HTTP_PROXY", invalid)],
            false,
        );
        assert_eq!(selected.http.as_deref(), Some("http://all.invalid:8000/"));
    }
}

#[test]
fn cgi_ignores_both_http_variables_but_retains_all_and_https() {
    let values = [
        ("HTTP_PROXY", "untrusted.invalid:1"),
        ("http_proxy", "untrusted-lower.invalid:2"),
        ("ALL_PROXY", "all.invalid:3"),
        ("HTTPS_PROXY", "secure.invalid:4"),
    ];
    let selected = environment(&values, true);
    assert_eq!(selected.http.as_deref(), Some("http://all.invalid:3/"));
    assert_eq!(selected.https.as_deref(), Some("http://secure.invalid:4/"));
    assert!(environment(&values[..2], true).is_empty());
}

#[test]
fn system_fallback_is_whole_map_and_requires_enable_one() {
    let selected = resolve_addresses(
        environment(&[("HTTP_PROXY", "env.invalid:1")], false),
        true,
        Some("http=system.invalid:2;https=system.invalid:3"),
    );
    assert_eq!(selected.http.as_deref(), Some("http://env.invalid:1/"));
    assert!(selected.https.is_none());
    let selected = resolve_addresses(
        environment(&[("HTTP_PROXY", "http://[")], false),
        true,
        Some("system.invalid:4"),
    );
    assert_eq!(selected.http.as_deref(), Some("http://system.invalid:4/"));
    assert_eq!(selected.https.as_deref(), Some("http://system.invalid:4/"));
    assert!(resolve_addresses(Addresses::default(), false, Some("system.invalid:4")).is_empty());
    assert!(resolve_addresses(Addresses::default(), true, None).is_empty());
}

#[test]
fn windows_protocol_lists_preserve_old_validation_and_scope() {
    let selected =
        system_addresses("http=first.invalid:1;https=https://secure.invalid:2;http=last.invalid:3");
    assert_eq!(selected.http.as_deref(), Some("http://last.invalid:3/"));
    assert_eq!(selected.https.as_deref(), Some("https://secure.invalid:2/"));
    for invalid in [
        "http=proxy.invalid:1;",
        "http=proxy.invalid:1;https=wrong=shape",
    ] {
        assert!(system_addresses(invalid).is_empty());
    }
    let selected = system_addresses("http=proxy.invalid:1;https=http://[");
    assert!(selected.http.is_some());
    assert!(selected.https.is_none());
    let selected = system_addresses("http://proxy.invalid:1");
    assert!(selected.http.is_some());
    assert!(selected.https.is_none());
    assert!(system_addresses("HTTP=proxy.invalid:1").is_empty());
}

#[test]
fn proxy_credentials_and_scheme_less_hosts_remain_supported() {
    assert_eq!(
        normalize_address("localhost:8080").as_deref(),
        Some("http://localhost:8080/")
    );
    assert_eq!(
        normalize_address("http://user:p%40ss@proxy.invalid:8080").as_deref(),
        Some("http://user:p%40ss@proxy.invalid:8080/")
    );
    assert_eq!(
        normalize_address("https://[::1]:8080").as_deref(),
        Some("https://[::1]:8080/")
    );
}

#[tokio::test]
async fn system_only_proxy_respects_empty_absent_and_explicit_bypass_on_wire() {
    use tokio::{
        io::{AsyncReadExt, AsyncWriteExt},
        net::TcpListener,
    };
    tokio::time::timeout(std::time::Duration::from_secs(10), async {
        for (environment, system, direct) in [
            (Some(""), "127.0.0.1", false),
            (None, "127.0.0.1", true),
            (Some("127.0.0.1"), "not-matching.invalid", true),
            (Some("not-matching.invalid"), "127.0.0.1", false),
            (None, "*.fixture.invalid; 127.0.0.1", true),
        ] {
            let origin = TcpListener::bind("127.0.0.1:0").await.unwrap();
            let proxy = TcpListener::bind("127.0.0.1:0").await.unwrap();
            let url = format!("http://{}/proxy-policy", origin.local_addr().unwrap());
            let addresses = resolve_addresses(
                Addresses::default(),
                true,
                Some(&proxy.local_addr().unwrap().to_string()),
            );
            let bypass = resolve_bypass(environment.map(str::to_owned), Some(system.into()));
            let client = apply(rquest::Client::builder().http1_only(), &addresses, bypass)
                .build()
                .unwrap();
            let server = async {
                let (mut socket, _) = if direct { &origin } else { &proxy }
                    .accept()
                    .await
                    .unwrap();
                let mut headers = Vec::new();
                while !headers.ends_with(b"\r\n\r\n") {
                    assert!(headers.len() < 8192);
                    headers.push(socket.read_u8().await.unwrap());
                }
                let target = if direct { "/proxy-policy" } else { &url };
                assert!(String::from_utf8(headers)
                    .unwrap()
                    .starts_with(&format!("GET {target} HTTP/1.1\r\n")));
                socket
                    .write_all(
                        b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\nConnection: close\r\n\r\nOK",
                    )
                    .await
                    .unwrap();
            };
            let request = async {
                assert_eq!(
                    client.get(&url).send().await.unwrap().text().await.unwrap(),
                    "OK"
                );
            };
            tokio::join!(server, request);
        }
    })
    .await
    .unwrap();
}
