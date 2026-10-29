//! Unified Jina accounts dispatch to two vendor-owned hosts, never to the fetched URL.
use crate::{
    error::GatewayError,
    implementation_lines::{compiled_out_error_for_line, RefactoredImplementationLine as Line},
    protocol::{
        canonical::{CanonicalRelayRequest, EndpointKind},
        search_api,
    },
    routing::candidate::ProviderAccountPayload,
    upstream::common::RequestPlan,
};

pub(super) fn owns_payload(payload: &ProviderAccountPayload) -> bool {
    payload.canonical_adapter() == "search_api_compatible"
        && payload.search_path.as_deref() == Some("/search")
        && payload.fetch_path.as_deref() == Some("/")
        && payload.search_query_field.as_deref() == Some("q")
        && payload.fetch_urls_field.as_deref() == Some("url")
}

pub(super) fn build_request_plan(
    payload: &ProviderAccountPayload,
    req: &CanonicalRelayRequest,
) -> Result<RequestPlan, GatewayError> {
    let (line, path, body) = match req.endpoint_kind {
        EndpointKind::Search => (
            Line::JinaSearchOfficialVendorApi,
            "/search",
            search_api::pack_search(req, "q"),
        ),
        EndpointKind::Fetch => {
            // Reader accepts one URL. Never silently truncate a multi-URL request to its first item.
            if req
                .raw_body
                .get("urls")
                .is_some_and(|urls| !urls.as_array().is_some_and(|urls| urls.len() == 1))
                || (req.raw_body.get("url").is_some() && req.raw_body.get("urls").is_some())
            {
                return Err(GatewayError::bad_request(
                    "Jina Reader requires exactly one url or a single-item urls array.",
                ));
            }
            let body = search_api::pack_fetch(req, "url");
            let url = body
                .get("url")
                .and_then(serde_json::Value::as_str)
                .and_then(|value| url::Url::parse(value).ok());
            if !url.is_some_and(|url| {
                matches!(url.scheme(), "http" | "https")
                    && url.host_str().is_some()
                    && url.username().is_empty()
                    && url.password().is_none()
            }) {
                return Err(GatewayError::bad_request(
                    "Jina Reader requires an HTTP(S) URL without embedded credentials.",
                ));
            }
            (Line::JinaReaderOfficialVendorApi, "/", body)
        }
        _ => {
            return Err(GatewayError::bad_request(
                "Jina supports only search and fetch operations.",
            ))
        }
    };
    // Account/profile gates cannot express the endpoint-specific implementation line.
    if !line.compiled_in() {
        return Err(compiled_out_error_for_line(line));
    }
    let base = payload.base_url.trim_end_matches('/');
    let base = if req.endpoint_kind == EndpointKind::Fetch && base == "https://s.jina.ai" {
        "https://r.jina.ai"
    } else {
        base
    };
    Ok(RequestPlan {
        method: rquest::Method::POST,
        url: format!("{base}{path}"),
        query: Vec::new(),
        body: Some(body),
        response_kind: req.endpoint_kind,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn payload(base: &str) -> ProviderAccountPayload {
        serde_json::from_value(json!({"adapter":"search_api_compatible", "base_url":base, "api_key":"synthetic",
            "search_path":"/search", "fetch_path":"/", "search_query_field":"q", "fetch_urls_field":"url"})).unwrap()
    }

    #[test]
    fn unified_jina_hosts_bodies_and_endpoint_feature_gates() {
        let search =
            search_api::normalize_search(json!({"model":"jina-search","query":"Rust","num":1}))
                .unwrap();
        let fetch = search_api::normalize_fetch(
            json!({"model":"jina-fetch","urls":["https://example.com/"]}),
        )
        .unwrap();
        let account = payload("https://s.jina.ai");
        assert!(owns_payload(&account));
        let plan = build_request_plan(&account, &search);
        assert_eq!(
            plan.is_ok(),
            Line::JinaSearchOfficialVendorApi.compiled_in()
        );
        if let Ok(plan) = plan {
            assert_eq!(plan.url, "https://s.jina.ai/search");
            assert_eq!(plan.body, Some(json!({"q":"Rust","num":1})));
        }
        let plan = build_request_plan(&account, &fetch);
        assert_eq!(
            plan.is_ok(),
            Line::JinaReaderOfficialVendorApi.compiled_in()
        );
        if let Ok(plan) = plan {
            assert_eq!(plan.url, "https://r.jina.ai/");
            assert_eq!(plan.body, Some(json!({"url":"https://example.com/"})));
        }
        let compiled = Line::JinaSearchOfficialVendorApi.compiled_in()
            && Line::JinaReaderOfficialVendorApi.compiled_in();
        assert_eq!(
            crate::implementation_lines::is_protocol_profile_compiled_in("jina"),
            compiled
        );
        assert_eq!(
            crate::implementation_lines::ensure_protocol_profile_compiled("jina").is_ok(),
            compiled
        );
        assert_eq!(
            crate::preset::builtin_presets().contains_key("jina"),
            compiled
        );
        assert_eq!(
            crate::protocol::registry::default_protocol_profile_for_preset("jina"),
            "jina"
        );
        assert_eq!(
            crate::protocol::registry::default_protocol_family_for_profile(
                "jina",
                "search_api_compatible"
            ),
            "search"
        );
        for (hint, base, expected) in [
            (Some("jina"), None, "jina"),
            (None, Some("https://s.jina.ai"), "jina"),
            (None, Some("https://r.jina.ai"), "jina_reader"),
        ] {
            assert_eq!(
                crate::protocol::registry::infer_protocol_profile(
                    "search_api_compatible",
                    hint,
                    base
                ),
                expected
            );
        }
        assert_ne!(
            crate::protocol::registry::infer_protocol_profile(
                "search_api_compatible",
                None,
                Some("https://s.jina.ai.example.invalid")
            ),
            "jina"
        );
    }

    #[test]
    fn reader_rejects_invalid_or_multiple_targets_before_any_send() {
        for body in [
            json!({"urls":["https://example.com/","https://example.org/"]}),
            json!({"url":"file:///private"}),
            json!({"url":"https://user:password@example.com/"}),
            json!({"url":"javascript:alert(1)"}),
            json!({"url":"https://example.com/","urls":["https://example.com/"]}),
        ] {
            let request = search_api::normalize_fetch(body).unwrap();
            assert!(build_request_plan(&payload("https://s.jina.ai"), &request).is_err());
        }
    }

    #[test]
    fn custom_origins_are_not_rewritten_to_a_different_host() {
        if !Line::JinaReaderOfficialVendorApi.compiled_in() {
            return;
        }
        let request = search_api::normalize_fetch(json!({"url":"https://example.com/"})).unwrap();
        for base in [
            "http://127.0.0.1:12345",
            "https://s.jina.ai.example.invalid",
            "https://s.jina.ai/custom",
        ] {
            assert_eq!(
                build_request_plan(&payload(base), &request).unwrap().url,
                format!("{base}/")
            );
        }
    }
}
