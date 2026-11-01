//! HTTP identity measured from the installed native hub. TLS belongs to the native transport.
pub const USER_AGENT: &str =
    "antigravity/hub/2.15.0 (aidev_client; os_type=windows; arch=amd64; cl=983516863)";

pub(super) fn apply(builder: rquest::RequestBuilder, token: &str) -> rquest::RequestBuilder {
    // The shared client can contain Chrome defaults. Do not mix browser or CLI profiles.
    builder
        .default_headers(false)
        .redirect(rquest::redirect::Policy::none())
        .header("user-agent", USER_AGENT)
        .header("content-type", "application/json")
        .bearer_auth(token)
}
