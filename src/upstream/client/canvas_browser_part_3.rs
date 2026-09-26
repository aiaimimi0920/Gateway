use super::*;

impl UpstreamClient {
    pub(super) async fn ensure_gemini_canvas_program_payload_handle(
        &self,
        payload: &ProviderAccountPayload,
        operation: &str,
        locale: &str,
        timeout: Duration,
    ) -> Result<ProviderAccountPayload, GatewayError> {
        if payload.adapter != "gemini_canvas_program_web_reverse_compatible" {
            return Ok(payload.clone());
        }

        let provider = payload.adapter.as_str();
        let bootstrap_operation =
            gemini_canvas_program_web_reverse_modular::normalize_gemini_canvas_program_bootstrap_operation(
                operation,
            );
        let mut ensured_payload = payload.clone();
        if !gemini_canvas_program_web_reverse_modular::gemini_canvas_program_payload_has_concrete_handle(
            &ensured_payload,
        ) {
            match self
                .try_ensure_gemini_canvas_program_payload_handle_pure_http(
                    &ensured_payload,
                    bootstrap_operation,
                    locale,
                    timeout,
                )
                .await
            {
                Ok(next_payload) => ensured_payload = next_payload,
                Err(error) => {
                    debug!(
                        provider,
                        code = ?error.code,
                        http_status = ?error.http_status,
                        "Gemini Canvas pure HTTP program create did not produce a concrete handle; falling back to browser discovery"
                    );
                }
            }
        }
        if gemini_canvas_program_web_reverse_modular::gemini_canvas_program_payload_handle_matches_operation(
            &ensured_payload,
            bootstrap_operation,
        ) {
            return Ok(ensured_payload);
        }
        let browser_pool_base_url = self.ensure_gemini_canvas_browser_pool(provider).await?;
        let bootstrap_prompt =
            gemini_canvas_program_web_reverse_modular::default_gemini_canvas_program_bootstrap_prompt(
                bootstrap_operation,
            );
        let bootstrap_payload = if gemini_canvas_program_web_reverse_modular::gemini_canvas_program_payload_has_concrete_handle(
            &ensured_payload,
        ) {
            ensured_payload.clone()
        } else {
            gemini_canvas_program_web_reverse_modular::strip_gemini_canvas_program_handle_hints_from_payload(
                &ensured_payload,
            )
        };
        let invocation = self
            .execute_gemini_canvas_program_bootstrap_request(
                provider,
                &browser_pool_base_url,
                &bootstrap_payload,
                bootstrap_operation,
                &bootstrap_prompt,
                locale,
                timeout,
            )
            .await?;
        let patch = gemini_canvas_program_web_reverse_modular::runtime_patch_from_browser_invocation(
            &invocation,
        )
        .ok_or_else(|| {
                gemini_canvas_program_web_reverse_modular::gemini_canvas_program_bootstrap_missing_handle_patch_error(
                    provider,
                )
            })?;
        let ensured_payload = merge_extra_body_patch_into_payload(&ensured_payload, &patch);
        if !gemini_canvas_program_web_reverse_modular::gemini_canvas_program_payload_has_concrete_handle(
            &ensured_payload,
        ) {
            return Err(
                gemini_canvas_program_web_reverse_modular::gemini_canvas_program_bootstrap_incomplete_error(
                    provider,
                ),
            );
        }
        persist_gemini_canvas_program_runtime_material(
            self.redis_pool.as_ref(),
            self.pg_pool.as_ref(),
            &ensured_payload,
            Some(patch),
        )
        .await;
        Ok(ensured_payload)
    }
}
