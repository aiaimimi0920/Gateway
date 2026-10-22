//! Edge cases pinned to Go net/http/internal/httpcommon, not cookie parsing policy.
use crate::wire;
use http::{HeaderValue, Request};

#[test]
fn h2_cookie_split_preserves_go_whitespace_and_empty_segments() {
    let mut request = Request::builder()
        .uri("https://example.test/")
        .body(rquest::Body::from(""))
        .unwrap();
    let mut cookie = HeaderValue::from_static(" a=1 ;  b=2;;\tc=3;");
    cookie.set_sensitive(true);
    request.headers_mut().append("cookie", cookie);
    request
        .headers_mut()
        .append("cookie", HeaderValue::from_static(""));
    wire::prepare(&mut request, true).unwrap();
    let cookies: Vec<_> = request.headers().get_all("cookie").iter().collect();
    assert_eq!(
        cookies.iter().map(|v| v.as_bytes()).collect::<Vec<_>>(),
        vec![b" a=1 ".as_slice(), b"b=2", b"", b"\tc=3"]
    );
    assert!(cookies.iter().all(|v| v.is_sensitive()));
}

#[test]
fn h2_user_agent_uses_only_first_value_and_omits_explicit_empty() {
    for first in ["", "fixture-agent"] {
        let mut request = Request::builder()
            .uri("https://example.test/")
            .body(rquest::Body::from(""))
            .unwrap();
        request
            .headers_mut()
            .append("user-agent", first.parse().unwrap());
        request
            .headers_mut()
            .append("user-agent", "ignored-agent".parse().unwrap());
        wire::prepare(&mut request, true).unwrap();
        let values: Vec<_> = request.headers().get_all("user-agent").iter().collect();
        if first.is_empty() {
            assert!(values.is_empty());
        } else {
            assert_eq!(values, vec![&HeaderValue::from_static("fixture-agent")]);
        }
    }
}
