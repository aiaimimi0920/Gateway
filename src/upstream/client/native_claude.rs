//! Direct streaming Messages only: preserve bounded caller wire without Chrome defaults.
use super::*;
use crate::protocol::{claude_wire, codex_wire::NativeRequestWire};
use std::collections::HashMap;

impl UpstreamClient {
    pub(crate) fn uses_native_claude(
        payload: &ProviderAccountPayload,
        req: &CanonicalRelayRequest,
        model: &str,
    ) -> bool {
        payload.canonical_adapter() == "anthropic_compatible"
            && !payload.request_client_profile.unwrap_or_default().is_cpa()
            && req.endpoint_kind == EndpointKind::Messages
            && req.stream
            && payload.resolve_execution_mode(req.endpoint_kind)
                == ProviderExecutionMode::DirectHttp
            && payload
                .request_client_profile
                .unwrap_or_default()
                .uses_claude(model, req.requested_model.as_deref())
    }

    pub(crate) async fn send_native_claude(
        &self,
        payload: &ProviderAccountPayload,
        req: &CanonicalRelayRequest,
        model: &str,
        extra: Option<&HashMap<String, String>>,
        wire: Option<&NativeRequestWire>,
    ) -> Result<UpstreamStreamingResponse, GatewayError> {
        let plan = Self::build_request_plan(payload, req, model, true)?;
        let mut headers = build_inference_headers(payload, req, model, extra);
        headers
            .entry("connection")
            .or_insert(HeaderValue::from_static("keep-alive"));
        let reference_identity = headers
            .get("user-agent")
            .is_some_and(|v| v == claude_wire::USER_AGENT);
        if wire.is_some_and(|w| !w.client_request_id)
            && !payload
                .headers
                .keys()
                .any(|k| k.eq_ignore_ascii_case("x-request-id"))
        {
            headers.remove("x-request-id");
        }
        // Auth and Host remain selected-provider owned. Never copy arbitrary caller headers.
        let mut ordered = HeaderMap::new();
        let fallback = [
            "accept",
            "content-type",
            "user-agent",
            "x-claude-code-session-id",
            "x-stainless-arch",
            "x-stainless-lang",
            "x-stainless-os",
            "x-stainless-package-version",
            "x-stainless-retry-count",
            "x-stainless-runtime",
            "x-stainless-runtime-version",
            "x-stainless-timeout",
            "anthropic-beta",
            "anthropic-dangerous-direct-browser-access",
            "anthropic-version",
            "x-api-key",
            "x-app",
        ];
        let native = extra
            .and_then(|h| h.get("user-agent").zip(h.get("x-app")))
            .is_some_and(|(ua, app)| claude_wire::identity(ua, app));
        // Ordinary SDK compression describes its downstream transport, not ours.
        // All four decoders are enabled in Cargo.toml; operator settings win.
        if !native
            && !payload
                .headers
                .keys()
                .any(|k| k.eq_ignore_ascii_case("accept-encoding"))
        {
            headers.insert(
                "accept-encoding",
                HeaderValue::from_static("gzip, deflate, br, zstd"),
            );
        }
        let order: Vec<&str> = if native {
            wire.map(|w| w.header_order.iter().map(String::as_str).collect())
                .unwrap_or_else(|| fallback.to_vec())
        } else {
            fallback.to_vec()
        };
        for name in order {
            if let Some(value) = headers.remove(name) {
                if let Ok(name) = HeaderName::from_bytes(name.as_bytes()) {
                    ordered.insert(name, value);
                }
            }
        }
        ordered.extend(headers);
        let mut url = url::Url::parse(&plan.url)
            .map_err(|_| GatewayError::bad_request("Invalid upstream URL"))?;
        if let Some(query) = wire.and_then(|w| w.query.as_deref()) {
            let query = claude_wire::query(query).map_err(GatewayError::bad_request)?;
            if url.query().is_some_and(|base| {
                url::form_urlencoded::parse(base.as_bytes()).any(|(key, _)| key == "beta")
            }) {
                return Err(GatewayError::bad_request(
                    "Caller beta conflicts with configured upstream query",
                ));
            }
            let merged = match url.query() {
                Some(base) if !base.is_empty() => format!("{base}&{query}"),
                _ => query.to_owned(),
            };
            url.set_query(Some(&merged));
        }
        // Pin only the measured reference identity; unknown versions retain plain transport.
        let client = if reference_identity {
            &self.native_claude_http
        } else {
            &self.plain_http
        };
        let mut builder = client
            .request(plan.method.clone(), url.as_str())
            // API keys are provider-owned; never replay them at a redirect target.
            .redirect(rquest::redirect::Policy::none())
            .headers(ordered)
            .timeout(self.timeout);
        if reference_identity {
            let mut original = rquest::header::OrigHeaderMap::new();
            for name in claude_wire::HEADER_ORDER {
                original.insert(*name);
            }
            builder = builder.orig_headers(original);
        }
        if !plan.query.is_empty() {
            builder = builder.query(&plan.query);
        }
        if let Some(body) = &plan.body {
            builder = match wire.and_then(|w| w.unchanged_body(body)) {
                Some(bytes) => builder.body(bytes),
                None => builder.json(body),
            };
        }
        let response = builder
            .send()
            .await
            .map_err(|e| classify_network_error(&e, Some(&payload.adapter)))?;
        if !response.status().is_success() {
            return Err(crate::upstream::response_error::classify_response_error(
                response,
                &payload.adapter,
                "upstream HTTP error body",
            )
            .await);
        }
        Ok(UpstreamStreamingResponse::Http(response))
    }
}
