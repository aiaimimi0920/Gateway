//! RFC 6455 request framing; target authority and nonce are transport-owned.
use reqwest::header::{HeaderMap, HeaderValue};

pub(super) fn prepare(
    request: &mut reqwest::Request,
    key: &str,
) -> Result<(), crate::error::GatewayError> {
    let authority = &request.url()[url::Position::BeforeHost..url::Position::AfterPort];
    let mut headers = HeaderMap::new();
    // Never copy the downstream authority, nonce or hop-by-hop transport control.
    // Application headers were already credential-filtered and ordered by the
    // native request builder; retain their explicit provider overrides.
    for (name, value) in request.headers() {
        if !matches!(
            name.as_str(),
            "host"
                | "connection"
                | "upgrade"
                | "sec-websocket-version"
                | "sec-websocket-key"
                | "sec-websocket-extensions"
                | "sec-websocket-protocol"
                | "proxy-authorization"
                | "proxy-connection"
        ) {
            headers.append(name, value.clone());
        }
    }
    // The pinned writer removes these using HeaderMap's swap-remove before
    // serializing application headers. Put controls last so those removals
    // cannot permute the caller's application-header order.
    for (name, value) in [
        ("host", authority),
        ("connection", "Upgrade"),
        ("upgrade", "websocket"),
        ("sec-websocket-version", "13"),
        ("sec-websocket-key", key),
    ] {
        headers.insert(
            name,
            HeaderValue::from_str(value).map_err(|_| {
                crate::error::GatewayError::bad_request("Invalid WebSocket handshake header")
            })?,
        );
    }
    *request.headers_mut() = headers;
    *request.version_mut() = reqwest::Version::HTTP_11;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn native_websocket_handshake_owns_authority_nonce_and_preserves_application_order() {
        let mut request = reqwest::Client::new()
            .get("http://[::1]:8080/v1/responses")
            .header("host", "untrusted.example")
            .header("sec-websocket-key", "untrusted")
            .header("sec-websocket-extensions", "unimplemented")
            .header("authorization", "Bearer selected-provider")
            .header("originator", "codex_exec")
            .header("proxy-authorization", "Basic must-not-reach-origin")
            .build()
            .unwrap();
        prepare(&mut request, "new-connection-nonce").unwrap();
        assert_eq!(request.headers()["host"], "[::1]:8080");
        assert_eq!(
            request.headers()["sec-websocket-key"],
            "new-connection-nonce"
        );
        assert_eq!(
            request.headers()["authorization"],
            "Bearer selected-provider"
        );
        assert!(!request.headers().contains_key("sec-websocket-extensions"));
        assert!(!request.headers().contains_key("proxy-authorization"));
        let mut application = request.headers().clone();
        for name in [
            "host",
            "connection",
            "upgrade",
            "sec-websocket-version",
            "sec-websocket-key",
        ] {
            application.remove(name);
        }
        assert_eq!(
            application
                .keys()
                .map(|name| name.as_str())
                .collect::<Vec<_>>(),
            ["authorization", "originator"]
        );
    }
}
