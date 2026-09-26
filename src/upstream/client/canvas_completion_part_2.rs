use super::*;

impl UpstreamClient {
    pub(super) async fn harvest_gemini_canvas_direct_http_api_keys(
        &self,
        payload: &ProviderAccountPayload,
        runtime: &gemini_canvas::GeminiCanvasRuntime,
        session: &gemini_canvas::GeminiCanvasPureHttpSession,
        timeout: Duration,
    ) -> (Vec<String>, Vec<String>) {
        let provider = "gemini_canvas_compatible";
        let base_url = payload.base_url.trim_end_matches('/');
        let page_urls = [
            gemini_canvas::direct_http_referrer(base_url, &runtime.share_id),
            format!("{base_url}{}", gemini_web::GEMINI_WEB_DEFAULT_APP_PATH),
        ];
        let mut collected = Vec::new();
        let mut probes = Vec::new();

        for page_url in page_urls {
            let page_html = match self
                .fetch_gemini_canvas_direct_http_page_html(payload, session, &page_url, timeout)
                .await
            {
                Ok(body) => body,
                Err(error) => {
                    probes.push(format!(
                        "{} fetch_error={}",
                        page_url,
                        summarize_gateway_error(&error)
                    ));
                    debug!(
                        provider,
                        url = %page_url,
                        error = %summarize_gateway_error(&error),
                        "gemini canvas page API key harvest failed"
                    );
                    continue;
                }
            };
            let page_contains_aiza = page_html.contains("AIza");
            let harvested = gemini_canvas::extract_google_api_keys_from_page_blob(&page_html);
            probes.push(format!(
                "{} len={} contains_aiza={} api_key_count={}",
                page_url,
                page_html.len(),
                page_contains_aiza,
                harvested.len()
            ));
            debug!(
                provider,
                url = %page_url,
                page_len = page_html.len(),
                page_contains_aiza,
                api_key_count = harvested.len(),
                "gemini canvas direct HTTP page harvest probe"
            );
            if harvested.is_empty() {
                continue;
            }
            debug!(
                provider,
                url = %page_url,
                api_key_count = harvested.len(),
                "harvested Gemini Canvas API key candidates from current page state"
            );
            for candidate in harvested {
                if collected.iter().any(|existing| existing == &candidate) {
                    continue;
                }
                collected.push(candidate);
            }
        }

        (collected, probes)
    }

    pub(super) async fn fetch_gemini_canvas_direct_http_page_html(
        &self,
        payload: &ProviderAccountPayload,
        session: &gemini_canvas::GeminiCanvasPureHttpSession,
        page_url: &str,
        timeout: Duration,
    ) -> Result<String, GatewayError> {
        self.fetch_gemini_canvas_direct_http_page_html_with_locale(
            payload, session, page_url, timeout, None,
        )
        .await
    }

    pub(super) async fn fetch_gemini_canvas_direct_http_page_html_with_locale(
        &self,
        payload: &ProviderAccountPayload,
        session: &gemini_canvas::GeminiCanvasPureHttpSession,
        page_url: &str,
        timeout: Duration,
        locale_override: Option<&str>,
    ) -> Result<String, GatewayError> {
        let mut session = session.clone();
        self.fetch_gemini_canvas_direct_http_page_html_refreshing_session_with_locale(
            payload,
            &mut session,
            page_url,
            timeout,
            locale_override,
        )
        .await
    }

    pub(super) async fn fetch_gemini_canvas_direct_http_page_html_refreshing_session_with_locale(
        &self,
        payload: &ProviderAccountPayload,
        session: &mut gemini_canvas::GeminiCanvasPureHttpSession,
        page_url: &str,
        timeout: Duration,
        locale_override: Option<&str>,
    ) -> Result<String, GatewayError> {
        fetch_gemini_canvas_direct_http_page_html_refreshing_session_with_locale(
            &self.http,
            &self.plain_http,
            payload,
            session,
            page_url,
            timeout,
            locale_override,
        )
        .await
    }
}
