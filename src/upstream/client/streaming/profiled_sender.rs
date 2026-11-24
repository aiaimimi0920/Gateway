//! One final transport boundary shared by buffered and streaming requests.
use super::*;

#[path = "mistral_vibe_wire.rs"]
mod mistral_vibe_wire;

impl UpstreamClient {
    pub(crate) fn send_profiled_plan(
        &self,
        payload: &ProviderAccountPayload,
        plan: &RequestPlan,
        mut headers: rquest::header::HeaderMap,
    ) -> Result<crate::upstream::profiled_request::Pending, GatewayError> {
        if crate::protocol::commandcode::owns_payload(payload) {
            return Err(crate::protocol::commandcode::invalid());
        }
        if crate::protocol::sap_aicore::owns_payload(payload) {
            return Err(crate::protocol::sap_aicore::invalid());
        }
        crate::upstream::request_profile::finalize_cpa_full_headers(payload, plan, &mut headers)?;
        crate::upstream::openai_compatible_common::finalize_vibe_affinity(
            payload,
            plan,
            &mut headers,
        );
        // Sign only after the final fingerprint/header mutations, for buffered and stream alike.
        let headers = sign_bedrock_runtime_headers_if_needed(payload, plan, headers)?;
        let mut headers = headers;
        crate::upstream::grok_oauth::finalize_headers(payload, plan, &mut headers)?;
        crate::upstream::kiro_api_key::finalize_headers(payload, plan, &mut headers)?;
        crate::upstream::kiro_oauth_runtime::finalize_headers(payload, plan, &mut headers)?;
        crate::upstream::vertex_express::finalize_headers(payload, plan, &mut headers)?;
        crate::upstream::vertex_oauth::finalize_headers(payload, plan, &mut headers)?;
        crate::protocol::cloudflare_workers_ai::finalize_headers(payload, plan, &mut headers)?;
        crate::protocol::kilo_free::finalize_headers(payload, plan, &mut headers)?;
        crate::protocol::vercel_free::finalize(payload, plan, &mut headers)?;
        let vercel =
            crate::upstream::vercel_credit_guard::Check::for_plan(&self.plain_http, payload, plan)?;
        let kilo = crate::protocol::kilo_free::owns_payload(payload);
        let cloudflare = crate::upstream::cloudflare_neuron_guard::Check::for_payload(
            &self.cloudflare_neurons,
            &self.plain_http,
            payload,
        )?;
        crate::protocol::dim_oauth::runtime::validate_plan(payload, plan)?;
        if crate::protocol::dim_oauth::business::owns_payload(payload) {
            headers = crate::protocol::dim_oauth::runtime::headers(payload);
        }
        crate::protocol::nous_portal::runtime::finalize_headers(payload, plan, &mut headers)?;
        crate::protocol::agnes::finalize_headers(payload, plan, &mut headers)?;
        crate::protocol::luma_inference::finalize_headers(payload, plan, &mut headers)?;
        #[cfg(test)]
        let luma_plan =
            crate::protocol::luma_inference::test_support::transport_plan(payload, plan);
        #[cfg(test)]
        let plan = luma_plan.as_ref().unwrap_or(plan);
        crate::protocol::sensenova::finalize_headers(payload, plan, &mut headers)?;
        #[cfg(test)]
        let sensenova_plan =
            crate::protocol::sensenova::test_support::transport_plan(payload, plan);
        #[cfg(test)]
        let plan = sensenova_plan.as_ref().unwrap_or(plan);
        crate::protocol::minimax::finalize_headers(payload, plan, &mut headers)?;
        #[cfg(test)]
        let minimax_plan = crate::protocol::minimax::test_support::transport_plan(payload, plan);
        #[cfg(test)]
        let plan = minimax_plan.as_ref().unwrap_or(plan);
        #[cfg(test)]
        let agnes_plan = crate::protocol::agnes::test_support::transport_plan(payload, plan);
        #[cfg(test)]
        let plan = agnes_plan.as_ref().unwrap_or(plan);
        // Tests retain validated product plans; only the final transport destination changes.
        #[cfg(test)]
        let nous_plan = crate::protocol::nous_portal::test_support::transport_plan(payload, plan);
        #[cfg(test)]
        let plan = nous_plan.as_ref().unwrap_or(plan);
        #[cfg(test)]
        let dim_plan = crate::protocol::dim_oauth::test_support::transport_plan(payload, plan);
        #[cfg(test)]
        let plan = dim_plan.as_ref().unwrap_or(plan);
        #[cfg(all(test, feature = "line-cloudflare-workers-ai"))]
        let cf_plan = super::super::tests::cloudflare_wire_tests::transport_plan(payload, plan);
        // Legacy transport-only fixtures do not make live analytics requests.
        #[cfg(all(test, feature = "line-cloudflare-workers-ai"))]
        let cloudflare = cloudflare.filter(|_| cf_plan.is_none());
        #[cfg(all(test, feature = "line-cloudflare-workers-ai"))]
        let plan = cf_plan.as_ref().unwrap_or(plan);
        #[cfg(all(test, feature = "line-qwen-official-api"))]
        let local_plan = super::super::qwen_pipeline_test_support::transport_plan(payload, plan);
        #[cfg(all(test, feature = "line-qwen-official-api"))]
        let plan = local_plan.as_ref().unwrap_or(plan);
        #[cfg(test)]
        let grok_plan = super::super::grok_oauth_test_support::transport_plan(payload, plan);
        #[cfg(test)]
        let plan = grok_plan.as_ref().unwrap_or(plan);
        #[cfg(test)]
        let key_plan = super::super::kiro_api_key_test_support::transport_plan(payload, plan);
        #[cfg(test)]
        let plan = key_plan.as_ref().unwrap_or(plan);
        #[cfg(test)]
        let express_plan = super::super::vertex_express_test_support::transport_plan(payload, plan);
        #[cfg(test)]
        let plan = express_plan.as_ref().unwrap_or(plan);
        #[cfg(test)]
        let vertex_plan = super::super::vertex_oauth_test_support::transport_plan(payload, plan);
        #[cfg(test)]
        let plan = vertex_plan.as_ref().unwrap_or(plan);
        #[cfg(all(test, feature = "line-accio-web-reverse-api"))]
        let accio_plan =
            crate::provider_credential_refresh::accio::server_peer::transport_plan(payload, plan);
        #[cfg(all(test, feature = "line-accio-web-reverse-api"))]
        let plan = accio_plan.as_ref().unwrap_or(plan);
        let fixture = self
            .sensenova_fixture_http
            .as_ref()
            .filter(|_| crate::protocol::sensenova::owns_payload(payload));
        let vibe_wire = mistral_vibe_wire::matches(payload, plan);
        let builder = if let Some(http) = fixture {
            if !super::super::sensenova_fixture::synthetic_key(&payload.api_key) {
                return Err(GatewayError::bad_request(
                    "SenseNova fixture accepts only synthetic keys",
                )
                .with_code("sensenova_fixture_key_invalid"));
            }
            build_request_builder_from_plan(http, self.timeout, plan, headers)
        } else if vibe_wire {
            let body = mistral_vibe_wire::body(plan)?;
            let headers = mistral_vibe_wire::framing(headers);
            self.send_plan_with_body(plan, headers, Some(body))
        } else {
            self.send_plan(plan, headers)
        };
        let builder = if vibe_wire {
            mistral_vibe_wire::envelope(builder)
        } else if crate::protocol::nous_portal::business::owns_payload(payload)
            || crate::protocol::agnes::owns_payload(payload)
            || crate::protocol::luma_inference::owns_payload(payload)
            || crate::protocol::sensenova::owns_payload(payload)
            || crate::protocol::minimax::owns_payload(payload)
            || crate::protocol::cloudflare_workers_ai::owns_payload(payload)
            || kilo
            || crate::protocol::vercel_free::owns_payload(payload)
        {
            // A shared Client's ambient headers must not add a second credential after validation.
            builder
                .default_headers(false)
                .redirect(rquest::redirect::Policy::none())
        } else if crate::preset::is_mistral_vibe_wire(payload)
            || payload.canonical_adapter() == "bedrock_converse_compatible"
            || payload.canonical_adapter() == "kiro_compatible"
            || crate::protocol::qwen::workspace_api::owns_payload(payload)
            || crate::protocol::grok_oauth::business::owns_payload(payload)
            || crate::protocol::vertex_express::owns_payload(payload)
            || crate::protocol::vertex_oauth::business::owns_payload(payload)
            || crate::protocol::dim_oauth::business::owns_payload(payload)
        {
            builder.redirect(rquest::redirect::Policy::none())
        } else {
            builder
        };
        let rust = crate::upstream::cpa_transport::rust_enabled(payload)?;
        crate::upstream::profiled_request::Pending::new(
            builder,
            &payload.adapter,
            rust,
            self.timeout,
        )
        .map(|pending| {
            pending
                .with_cloudflare(cloudflare)
                .with_kilo_free(kilo)
                .with_vercel(vercel)
        })
    }
}
