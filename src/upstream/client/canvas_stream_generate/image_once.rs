//! A captured Web image request is one generation, not a transport probe.
use super::*;

impl UpstreamClient {
    pub(super) async fn send_gemini_web_image_template_once(
        &self,
        payload: &ProviderAccountPayload,
        prepared: &StreamGeneratePreparation,
        template: &gemini_canvas::GeminiCanvasTextStreamGenerateTemplate,
    ) -> Result<StreamGenerateBody, GatewayError> {
        let target = url::Url::parse(&template.url).ok();
        let base = url::Url::parse(&payload.base_url).ok();
        if !target
            .as_ref()
            .zip(base.as_ref())
            .is_some_and(|(target, base)| {
                target.origin() == base.origin()
                    && target.path() == gemini_web::GEMINI_WEB_DEFAULT_STREAM_GENERATE_PATH
                    && target.username().is_empty()
                    && target.password().is_none()
                    && target.fragment().is_none()
            })
        {
            return Err(
                GatewayError::bad_request("Invalid Gemini Web image template target.")
                    .with_code("gemini_web_image_template_target_invalid"),
            );
        }
        let provider = "gemini_web_reverse_modular_compatible";
        let mut headers = HeaderMap::new();
        apply_gemini_canvas_replay_template_headers(
            &mut headers,
            &template.headers,
            &prepared.session,
        );
        let mut cookies = SessionCookies::new(&payload.base_url, &headers)?;
        cookies.apply(&template.url, &mut headers)?;
        let response = self
            .http
            .request(Method::POST, &template.url)
            .headers(headers)
            .query(&template.query)
            // Never carry a session to a redirect or resend a generation after 307/308.
            .redirect(rquest::redirect::Policy::none())
            .timeout(prepared.image_stream_timeout)
            .body(template.raw_post_data.clone())
            .send()
            .await
            .map_err(|error| classify_network_error(&error, Some(provider)))?;
        let status = response.status().as_u16();
        let response_headers = response.headers().clone();
        let content_type = response
            .headers()
            .get(rquest::header::CONTENT_TYPE)
            .and_then(|value| value.to_str().ok())
            .map(str::to_string);
        let body = crate::protocol::upstream_body::collect_bounded_upstream_text_with_provider(
            response,
            "Gemini Web image response",
            provider,
        )
        .await?;
        if !(200..300).contains(&status)
            || gemini_web::response_indicates_browser_challenge(
                status,
                content_type.as_deref(),
                &body,
            )
            || gemini_web::response_indicates_session_invalid(
                status,
                content_type.as_deref(),
                &body,
            )
            || gemini_canvas::response_indicates_image_generation_unavailable(
                status,
                content_type.as_deref(),
                &body,
            )
        {
            let mut error =
                classify_gemini_canvas_pure_http_error(status, content_type.as_deref(), &body);
            error.message =
                "Gemini Web image request failed; private response content was omitted.".into();
            return Err(error);
        }
        // Only classified, bounded success advances this request's readback state.
        cookies.receive(&template.url, &response_headers);
        Ok(StreamGenerateBody {
            text: body,
            cookies: Some(cookies),
        })
    }
}

#[cfg(test)]
#[path = "image_once_tests.rs"]
mod tests;
