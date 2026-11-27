use super::*;

#[test]
fn cursor_remote_proxy_reuses_matching_policy_without_fixed_port() {
    for port in [17890, 28443] {
        let address = format!("http://127.0.0.1:{port}");
        let matcher = Matcher::builder().https(&address).no("").build();
        assert_eq!(proxy_port(&matcher, Some(&address)), Ok(port));
    }
    assert_eq!(explicit_port(OsStr::new("42344")), Ok(42344));
    for value in ["", "0", "65536", "-1", "not-a-port"] {
        assert_eq!(explicit_port(OsStr::new(value)), Err("proxy_invalid"));
    }
}

#[test]
fn cursor_remote_proxy_off_http_only_and_each_bypass_fail_closed() {
    assert_eq!(
        proxy_port(&Matcher::builder().build(), None),
        Err("proxy_required")
    );
    assert_eq!(
        proxy_port(
            &Matcher::builder().http("http://127.0.0.1:17890").build(),
            None
        ),
        Err("proxy_required")
    );
    for bypass in [
        "*",
        "api2.cursor.sh",
        "cursor.com",
        "agentn.global.api5.cursor.sh",
    ] {
        let matcher = Matcher::builder()
            .https("http://127.0.0.1:17890")
            .no(bypass)
            .build();
        assert_eq!(
            proxy_port(&matcher, Some("http://127.0.0.1:17890")),
            Err("proxy_required"),
            "{bypass}"
        );
    }
}

#[test]
fn cursor_remote_proxy_rejects_incompatible_tunnels() {
    for address in [
        "https://127.0.0.1:17890",
        "http://proxy.example.test:17890",
        "http://localhost:17890",
        "http://[::1]:17890",
        "http://user:secret@127.0.0.1:17890",
        "http://127.0.0.1:17890/path",
        "http://127.0.0.1:17890/?query=1",
        "http://127.0.0.1:17890/#fragment",
        "http://127.0.0.1:0",
    ] {
        let matcher = Matcher::builder().https(address).no("").build();
        assert_eq!(
            proxy_port(&matcher, Some(address)),
            Err("proxy_unsupported"),
            "{address}"
        );
    }
}

#[test]
fn cursor_remote_proxy_rejects_source_matcher_disagreement() {
    let matcher = Matcher::builder().https("http://127.0.0.1:17890").build();
    assert_eq!(
        proxy_port(&matcher, Some("http://127.0.0.1:28443")),
        Err("proxy_unsupported")
    );
}
