use super::*;

#[test]
fn http2_defaults_match_go_126_custom_provider_transport() {
    let options = wire::options();
    assert_eq!(options.initial_window_size, 4 * 1024 * 1024);
    assert_eq!(options.initial_conn_window_size, 65535 + 1024 * 1024 * 1024);
    assert_eq!(options.max_frame_size, Some(16384));
    assert_eq!(options.max_header_list_size, Some(10 * 1024 * 1024));
    assert_eq!(options.enable_push, Some(false));
    assert_eq!(options.header_table_size, None);
}

#[test]
fn all_connection_values_remove_their_named_headers() {
    for h2 in [false, true] {
        let mut request = http::Request::builder()
            .uri("https://example.test/")
            .header("connection", "keep-alive")
            .header("connection", "x-private-hop, x-second-hop")
            .header("x-private-hop", "synthetic")
            .header("x-second-hop", "synthetic")
            .body(rquest::Body::from("test"))
            .unwrap();
        wire::prepare(&mut request, h2).unwrap();
        assert!(!request.headers().contains_key("x-private-hop"));
        assert!(!request.headers().contains_key("x-second-hop"));
    }
}

#[test]
fn h2_serialization_retains_debug_redaction_flags() {
    let mut secret = http::HeaderValue::from_static("synthetic-secret");
    secret.set_sensitive(true);
    let mut request = http::Request::builder()
        .uri("https://example.test/")
        .header("authorization", secret.clone())
        .header("cookie", secret)
        .body(rquest::Body::from("test"))
        .unwrap();
    wire::prepare(&mut request, true).unwrap();
    assert!(request.headers()["authorization"].is_sensitive());
    assert!(request.headers()["cookie"].is_sensitive());
    assert!(!format!("{:?}", request.headers()).contains("synthetic-secret"));
}
