use super::*;

impl UpstreamClient {
    pub(crate) async fn execute_gemini_canvas_text(
        &self,
        payload: &ProviderAccountPayload,
        req: &CanonicalRelayRequest,
        model: &str,
        _extra_headers: Option<&std::collections::HashMap<String, String>>,
    ) -> Result<CanonicalRelayResponse, GatewayError> {
        let body = self
            .execute_gemini_canvas_generate_content_json(payload, req, model)
            .await?;
        gemini_api_modular::parse_generate_content_response(&body, model)
    }

    pub(crate) async fn execute_gemini_canvas_text_stream(
        &self,
        payload: &ProviderAccountPayload,
        req: &CanonicalRelayRequest,
        model: &str,
        _extra_headers: Option<&std::collections::HashMap<String, String>>,
    ) -> Result<
        std::pin::Pin<Box<dyn futures::Stream<Item = Result<bytes::Bytes, rquest::Error>> + Send>>,
        GatewayError,
    > {
        let canonical = self
            .execute_gemini_canvas_text(payload, req, model, None)
            .await?;
        let sse_bytes = canonical_response_to_openai_sse_bytes(req, model, &canonical)
            .into_iter()
            .map(Ok);
        Ok(Box::pin(futures::stream::iter(sse_bytes)))
    }
}
