//! Native Codex Responses lane. Browser/product adapters never enter this transport.
use super::*;
use crate::protocol::{codex_wire::NativeRequestWire, request_profile::RequestClientProfile};
use crate::upstream::native_response;
use std::collections::HashMap;

#[cfg(test)]
#[path = "native_codex_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "native_codex_profile_tests.rs"]
mod profile_tests;

#[cfg(test)]
#[path = "native_codex_recovery_tests.rs"]
mod recovery_tests;

#[cfg(test)]
#[path = "qwen_workspace_tests.rs"]
mod qwen_workspace_tests;

#[path = "native_codex_profile.rs"]
mod profile;

#[path = "native_compact.rs"]
mod compact;

#[path = "native_websocket.rs"]
pub(super) mod websocket;

impl UpstreamClient {
    pub(crate) fn uses_native_codex(
        payload: &ProviderAccountPayload,
        req: &CanonicalRelayRequest,
        model: &str,
    ) -> bool {
        payload.adapter == "openai_compatible"
            && !crate::protocol::grok_oauth::business::owns_payload(payload)
            && !crate::protocol::dim_oauth::business::owns_payload(payload)
            && !crate::protocol::nous_portal::business::owns_payload(payload)
            && !crate::protocol::agnes::owns_payload(payload)
            && !crate::protocol::luma_inference::owns_payload(payload)
            && !crate::protocol::sensenova::owns_payload(payload)
            && !crate::protocol::commandcode::owns_payload(payload)
            && !crate::protocol::lobsterai::owns_payload(payload)
            && !crate::protocol::sap_aicore::owns_payload(payload)
            && !crate::protocol::quick_oauth::owns_payload(payload)
            && payload.resolve_execution_mode(req.endpoint_kind)
                == ProviderExecutionMode::DirectHttp
            && req.endpoint_kind == EndpointKind::Responses
            && !payload.bridges_openai_responses_to_chat_completions(req.endpoint_kind)
            // A streaming caller needs no forced-stream aggregation. In particular,
            // Codex OAuth presets must retain the native transport and caller wire.
            && (req.stream || !payload.prefers_forced_streaming_responses(req.endpoint_kind))
            && matches!(
                payload.request_client_profile.unwrap_or_default(),
                RequestClientProfile::Auto | RequestClientProfile::CodexCli
            )
            && !payload
                .request_client_profile
                .unwrap_or_default()
                .uses_claude(model, req.requested_model.as_deref())
    }

    pub(crate) async fn execute_stream_with_wire(
        &self,
        payload: &ProviderAccountPayload,
        req: &CanonicalRelayRequest,
        model: &str,
        extra: Option<&HashMap<String, String>>,
        wire: Option<&NativeRequestWire>,
    ) -> Result<UpstreamStreamingResponse, GatewayError> {
        if !crate::protocol::lobsterai::owns_payload(payload) {
            crate::protocol::lobsterai::management_boundary::reject_generic(payload)?;
        }
        let flow = crate::protocol::call_flow::prepare(payload)?;
        let payload = flow.payload.as_ref();
        crate::protocol::bedrock_turn::validate_request(payload, req)?;
        if wire.is_some_and(|w| w.native_session.is_some())
            && !Self::uses_native_codex(payload, req, model)
        {
            return Err(GatewayError::bad_request(
                "Native session cannot switch to a translated transport",
            ));
        }
        if let Some(session) = wire.and_then(|w| w.websocket.as_ref()) {
            if !Self::uses_native_codex(payload, req, model) {
                return Err(GatewayError::bad_request(
                    "WebSocket requires native Codex Responses",
                ));
            }
            return websocket::send(self, session, payload, req, model, extra, wire.unwrap()).await;
        }
        if Self::uses_native_codex(payload, req, model) {
            return Box::pin(self.send_native_codex(payload, req, model, true, extra, wire))
                .await
                .map(UpstreamStreamingResponse::Native);
        }
        if Self::uses_native_claude(payload, req, model) {
            return self
                .send_native_claude(payload, req, model, extra, wire)
                .await;
        }
        Box::pin(self.execute_stream(payload, req, model, extra)).await
    }

    pub(crate) async fn execute_json_with_wire(
        &self,
        provider_id: &str,
        mode: ProviderExecutionMode,
        payload: &ProviderAccountPayload,
        req: &CanonicalRelayRequest,
        model: &str,
        extra: Option<&HashMap<String, String>>,
        wire: Option<&NativeRequestWire>,
    ) -> Result<Value, GatewayError> {
        if !crate::protocol::lobsterai::owns_payload(payload) {
            crate::protocol::lobsterai::management_boundary::reject_generic(payload)?;
        }
        let flow = crate::protocol::call_flow::prepare(payload)?;
        let payload = flow.payload.as_ref();
        crate::protocol::bedrock_turn::validate_request(payload, req)?;
        if wire.is_some_and(|w| w.compact) && !Self::uses_native_codex(payload, req, model) {
            return Err(compact::unsupported());
        }
        if Self::uses_native_codex(payload, req, model) {
            let response =
                Box::pin(self.send_native_codex(payload, req, model, false, extra, wire)).await?;
            return serde_json::from_slice(
                &native_response::read(response, &payload.adapter).await?,
            )
            .map_err(|e| GatewayError::server_error(format!("Invalid native upstream JSON: {e}")));
        }
        Box::pin(self.execute_json_passthrough(provider_id, mode, payload, req, model, extra)).await
    }

    async fn send_native_codex(
        &self,
        payload: &ProviderAccountPayload,
        req: &CanonicalRelayRequest,
        model: &str,
        stream: bool,
        extra: Option<&HashMap<String, String>>,
        wire: Option<&NativeRequestWire>,
    ) -> Result<reqwest::Response, GatewayError> {
        // Agnes fixtures isolate the profiled transport only. Fail before any
        // client or connection pool can send if dispatch regresses in a test.
        #[cfg(test)]
        assert!(
            !crate::protocol::agnes::owns_payload(payload)
                && !crate::protocol::luma_inference::owns_payload(payload)
                && !crate::protocol::sensenova::owns_payload(payload)
                && !crate::protocol::commandcode::owns_payload(payload)
                && !crate::protocol::lobsterai::owns_payload(payload)
                && !crate::protocol::sap_aicore::owns_payload(payload)
                && !crate::protocol::quick_oauth::owns_payload(payload),
            "Agnes test attempted an unisolated native Codex transport"
        );
        let plan = if wire.is_some_and(|w| w.compact) {
            compact::plan(payload, req, model)?
        } else {
            Self::build_request_plan(payload, req, model, stream)?
        };
        let connection = wire
            .filter(|w| w.websocket.is_none())
            .and_then(|w| w.connection.as_ref());
        let (builder, generation) = self.native_request(payload, req, model, extra, wire, &plan)?;
        let response = async {
            if let Some(scope) = wire.and_then(|w| w.native_session) {
                let connection = wire.and_then(|w| w.connection.as_ref()).ok_or_else(|| {
                    GatewayError::bad_request("Native session connection required")
                })?;
                let _session_lease = self
                    .initialize_sdk_session(&builder, payload, scope, connection, extra)
                    .await?;
                self.send_native_session(builder, payload, scope).await
            } else {
                crate::http_client::send_native_codex(builder, &payload.adapter).await
            }
        }
        .await;
        if let (Some(connection), Some(generation), Err(error)) =
            (connection, generation, &response)
        {
            connection.recover_after_send(generation, error);
        }
        native_response::success(response?, &payload.adapter).await
    }

    fn native_request(
        &self,
        payload: &ProviderAccountPayload,
        req: &CanonicalRelayRequest,
        model: &str,
        extra: Option<&HashMap<String, String>>,
        wire: Option<&NativeRequestWire>,
        plan: &crate::upstream::common::RequestPlan,
    ) -> Result<(reqwest::RequestBuilder, Option<u64>), GatewayError> {
        let filtered = extra.map(|headers| {
            headers
                .iter()
                .filter(|(name, _)| {
                    crate::protocol::codex_wire::FORWARDED_HEADERS.contains(&name.as_str())
                        || matches!(name.as_str(), "x-request-id" | "traceparent" | "tracestate")
                })
                .map(|(name, value)| (name.clone(), value.clone()))
                .collect()
        });
        let mut headers = build_inference_headers(payload, req, model, filtered.as_ref());
        if wire.is_some_and(|wire| wire.websocket.is_some()) {
            // An Upgrade GET has no JSON/SSE entity. Keep explicit provider
            // overrides (and caller Accept), not generation transport defaults.
            for name in ["content-type", "accept"] {
                let configured = payload
                    .headers
                    .keys()
                    .any(|key| key.eq_ignore_ascii_case(name));
                let supplied = extra.is_some_and(|headers| headers.contains_key(name));
                if !configured && !supplied {
                    headers.remove(name);
                }
            }
        }
        // Local tracing remains on the response/log; never invent an upstream identity.
        if wire.is_some_and(|w| !w.client_request_id)
            && !payload
                .headers
                .keys()
                .any(|k| k.eq_ignore_ascii_case("x-request-id"))
        {
            headers.remove("x-request-id");
        }
        if let Some(wire) = wire {
            let sdk_order = profile::sdk_envelope(&mut headers, payload, req, extra, wire);
            let header_order = sdk_order.as_ref().unwrap_or(&wire.header_order);
            let mut ordered = HeaderMap::new();
            for name in header_order {
                if let Some(value) = headers.remove(name.as_str()) {
                    if let Ok(name) = HeaderName::from_bytes(name.as_bytes()) {
                        ordered.insert(name, value);
                    }
                }
                // Gateway authentication need not include the upstream account
                // header. Place the selected account next to its bearer token,
                // as native OAuth does, without trusting the inbound account ID.
                if name == "authorization"
                    && !header_order.iter().any(|key| key == "chatgpt-account-id")
                {
                    if let Some(value) = headers.remove("chatgpt-account-id") {
                        ordered.insert("chatgpt-account-id", value);
                    }
                }
            }
            ordered.extend(headers);
            headers = ordered;
        }
        let mut url = url::Url::parse(&plan.url)
            .map_err(|_| GatewayError::bad_request("Invalid upstream URL"))?;
        if let Some(query) = wire.and_then(|w| w.query.as_deref()) {
            let merged = match url.query() {
                Some(base) if !base.is_empty() => format!("{base}&{query}"),
                _ => query.to_owned(),
            };
            url.set_query(Some(&merged));
        }
        let codex_tls12 = payload.request_client_profile == Some(RequestClientProfile::CodexCli)
            || url.host_str().is_some_and(|host| {
                host.eq_ignore_ascii_case("chatgpt.com")
                    || host.to_ascii_lowercase().ends_with(".chatgpt.com")
            });
        // Production requests use the accepted connection's pool. Internal
        // callers without wire evidence retain the existing client behavior.
        let (http, generation) = wire
            .and_then(|w| {
                w.websocket
                    .as_ref()
                    .map(|socket| (socket.client(self.timeout).clone(), None))
                    .or_else(|| {
                        w.connection.as_ref().map(|connection| {
                            let (client, generation) = connection.client_for_profile(
                                self.timeout,
                                w.native_session.is_some(),
                                codex_tls12,
                            );
                            (client, Some(generation))
                        })
                    })
            })
            .unwrap_or_else(|| {
                let session = wire.is_some_and(|w| w.native_session.is_some());
                let client = if !codex_tls12 || session || crate::request_wait::transport::managed()
                {
                    self.fallback_native_http
                        .client_for_profile(self.timeout, session, codex_tls12)
                        .0
                } else {
                    self.native_http.clone()
                };
                (client, None)
            });
        let mut builder = http.request(plan.method.clone(), url).headers(headers);
        // reqwest total timeout includes body consumption; never apply it to SSE.
        // Buffered/auxiliary requests retain their existing total deadline.
        if !crate::request_wait::transport::managed()
            && !plan
                .body
                .as_ref()
                .is_some_and(|body| body["stream"] == true)
        {
            builder = builder.timeout(self.timeout);
        }
        if !plan.query.is_empty() {
            builder = builder.query(&plan.query);
        }
        if let Some(body) = &plan.body {
            builder = match wire.and_then(|w| w.unchanged_body(body)) {
                Some(raw) => builder.body(raw),
                None => builder.json(body),
            };
        }
        Ok((builder, generation))
    }
}
