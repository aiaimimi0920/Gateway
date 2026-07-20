use std::env;
use std::ffi::OsStr;
use std::future::Future;
use std::sync::Arc;

use axum::http::{HeaderMap, HeaderValue};
use parking_lot::Mutex;

pub(crate) const ROUTE_PROOF_SERVER_ENV: &str = "GATEWAY_LIVE_ROUTE_PROOF_ENABLED";
pub(crate) const ROUTE_PROOF_REQUEST_HEADER: &str = "x-neuro-gateway-route-proof-request";
pub(crate) const ROUTE_PROOF_REQUEST_VALUE: &str = "v1";
pub(crate) const ROUTE_PROOF_RESPONSE_HEADER: &str = "x-neuro-gateway-route-proof";
pub(crate) const ROUTE_PROOF_PROVIDER_LINE_HEADER: &str = "x-neuro-gateway-provider-line";
pub(crate) const ROUTE_PROOF_ADAPTER_HEADER: &str = "x-neuro-gateway-provider-adapter";
pub(crate) const ROUTE_PROOF_PROTOCOL_PROFILE_HEADER: &str = "x-neuro-gateway-protocol-profile";

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct RouteProof {
    pub(crate) provider_line: String,
}

impl RouteProof {
    pub(crate) fn new(provider_line: &str) -> Self {
        Self {
            provider_line: provider_line.to_string(),
        }
    }
}

tokio::task_local! {
    static ROUTE_PROOF_SINK: Arc<Mutex<Option<RouteProof>>>;
}

pub(crate) fn parse_server_enabled_flag(value: Option<&OsStr>) -> bool {
    value
        .and_then(OsStr::to_str)
        .map(str::trim)
        .map(|value| value.to_ascii_lowercase())
        .is_some_and(|value| matches!(value.as_str(), "1" | "true" | "yes" | "on"))
}

pub(crate) fn server_route_proof_enabled() -> bool {
    parse_server_enabled_flag(env::var_os(ROUTE_PROOF_SERVER_ENV).as_deref())
}

pub(crate) fn should_capture_route_proof(server_enabled: bool, headers: &HeaderMap) -> bool {
    server_enabled
        && headers
            .get(ROUTE_PROOF_REQUEST_HEADER)
            .and_then(|value| value.to_str().ok())
            .is_some_and(|value| value.trim() == ROUTE_PROOF_REQUEST_VALUE)
}

pub(crate) async fn capture_route_proof<F>(
    enabled: bool,
    future: F,
) -> (F::Output, Option<RouteProof>)
where
    F: Future,
{
    if !enabled {
        return (future.await, None);
    }

    let sink = Arc::new(Mutex::new(None));
    let output = ROUTE_PROOF_SINK.scope(Arc::clone(&sink), future).await;
    let proof = sink.lock().clone();
    (output, proof)
}

pub(crate) fn record_route_proof(proof: RouteProof) {
    let _ = ROUTE_PROOF_SINK.try_with(|sink| {
        *sink.lock() = Some(proof);
    });
}

pub(crate) fn clear_route_proof_headers(headers: &mut HeaderMap) {
    headers.remove(ROUTE_PROOF_RESPONSE_HEADER);
    headers.remove(ROUTE_PROOF_PROVIDER_LINE_HEADER);
    headers.remove(ROUTE_PROOF_ADAPTER_HEADER);
    headers.remove(ROUTE_PROOF_PROTOCOL_PROFILE_HEADER);
}

pub(crate) fn apply_route_proof_headers(headers: &mut HeaderMap, proof: &RouteProof) -> bool {
    if proof.provider_line.trim().is_empty() {
        return false;
    }

    let provider_line = match HeaderValue::try_from(proof.provider_line.as_str()) {
        Ok(value) => value,
        Err(_) => return false,
    };

    headers.insert(
        ROUTE_PROOF_RESPONSE_HEADER,
        HeaderValue::from_static(ROUTE_PROOF_REQUEST_VALUE),
    );
    headers.insert(ROUTE_PROOF_PROVIDER_LINE_HEADER, provider_line);
    true
}

#[cfg(test)]
mod tests {
    use std::ffi::OsStr;

    use axum::http::HeaderMap;

    use super::*;

    #[test]
    fn route_proof_requires_server_and_request_opt_in() {
        let mut headers = HeaderMap::new();
        headers.insert(ROUTE_PROOF_REQUEST_HEADER, "v1".parse().unwrap());

        assert!(!should_capture_route_proof(false, &headers));
        assert!(should_capture_route_proof(true, &headers));

        headers.insert(ROUTE_PROOF_REQUEST_HEADER, "unexpected".parse().unwrap());
        assert!(!should_capture_route_proof(true, &headers));
    }

    #[test]
    fn route_proof_server_flag_is_disabled_by_default() {
        assert!(!parse_server_enabled_flag(None));
        assert!(!parse_server_enabled_flag(Some(OsStr::new("false"))));
        assert!(parse_server_enabled_flag(Some(OsStr::new("true"))));
        assert!(parse_server_enabled_flag(Some(OsStr::new("1"))));
    }

    #[tokio::test]
    async fn capture_returns_only_the_winning_non_secret_route_identity() {
        let ((), proof) = capture_route_proof(true, async {
            record_route_proof(RouteProof::new("linkup-search-official-vendor-api"));
        })
        .await;

        let proof = proof.expect("route proof should be captured");
        assert_eq!(proof.provider_line, "linkup-search-official-vendor-api");
    }

    #[test]
    fn response_headers_are_versioned_and_exclude_credential_material() {
        let proof = RouteProof::new("linkup-search-official-vendor-api");
        let mut headers = HeaderMap::new();

        assert!(apply_route_proof_headers(&mut headers, &proof));
        assert_eq!(headers[ROUTE_PROOF_RESPONSE_HEADER], "v1");
        assert_eq!(
            headers[ROUTE_PROOF_PROVIDER_LINE_HEADER],
            "linkup-search-official-vendor-api"
        );
        assert!(headers.get(ROUTE_PROOF_ADAPTER_HEADER).is_none());
        assert!(headers.get(ROUTE_PROOF_PROTOCOL_PROFILE_HEADER).is_none());
        assert!(headers.get("x-neuro-provider-id").is_none());
        assert!(headers.get("x-neuro-provider-credential-id").is_none());
        assert!(headers.get("x-neuro-provider-label").is_none());
    }

    #[test]
    fn untrusted_response_proof_headers_are_cleared_before_injection() {
        let mut headers = HeaderMap::new();
        headers.insert(ROUTE_PROOF_RESPONSE_HEADER, "forged".parse().unwrap());
        headers.insert(
            ROUTE_PROOF_PROVIDER_LINE_HEADER,
            "forged-provider-line".parse().unwrap(),
        );
        headers.insert(
            ROUTE_PROOF_ADAPTER_HEADER,
            "forged-adapter".parse().unwrap(),
        );
        headers.insert(
            ROUTE_PROOF_PROTOCOL_PROFILE_HEADER,
            "forged-profile".parse().unwrap(),
        );

        clear_route_proof_headers(&mut headers);

        assert!(headers.get(ROUTE_PROOF_RESPONSE_HEADER).is_none());
        assert!(headers.get(ROUTE_PROOF_PROVIDER_LINE_HEADER).is_none());
        assert!(headers.get(ROUTE_PROOF_ADAPTER_HEADER).is_none());
        assert!(headers.get(ROUTE_PROOF_PROTOCOL_PROFILE_HEADER).is_none());
    }

    #[test]
    fn invalid_route_identity_does_not_emit_partial_headers() {
        let proof = RouteProof::new("linkup-search-official-vendor-api\nsecret");
        let mut headers = HeaderMap::new();

        assert!(!apply_route_proof_headers(&mut headers, &proof));
        assert!(headers.get(ROUTE_PROOF_RESPONSE_HEADER).is_none());
        assert!(headers.get(ROUTE_PROOF_PROVIDER_LINE_HEADER).is_none());
        assert!(headers.get(ROUTE_PROOF_ADAPTER_HEADER).is_none());
        assert!(headers.get(ROUTE_PROOF_PROTOCOL_PROFILE_HEADER).is_none());
    }
}
