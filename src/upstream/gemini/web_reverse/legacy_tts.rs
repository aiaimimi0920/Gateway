use std::collections::HashMap;

use crate::error::GatewayError;
use crate::protocol::canonical::CanonicalRelayRequest;
use crate::routing::candidate::ProviderAccountPayload;
use crate::upstream::client::UpstreamClient;
use crate::upstream::response_types::BinaryUpstreamResponse;

impl UpstreamClient {
    pub(crate) async fn execute_gemini_web_reverse_legacy_tts(
        &self,
        payload: &ProviderAccountPayload,
        req: &CanonicalRelayRequest,
        model: &str,
        extra_headers: Option<&HashMap<String, String>>,
    ) -> Result<BinaryUpstreamResponse, GatewayError> {
        let request_timeout = self.timeout.max(std::time::Duration::from_secs(300));
        let runtime = crate::protocol::gemini_canvas::runtime_from_payload(payload)?;
        if crate::protocol::gemini_canvas::pure_http_enabled(payload) {
            return self
                .execute_gemini_canvas_direct_http_tts(
                    payload,
                    req,
                    model,
                    &runtime,
                    request_timeout,
                )
                .await;
        }

        self.execute_gemini_canvas_browser_backed_tts(payload, req, model, extra_headers)
            .await
    }
}

pub async fn execute_legacy_tts(
    client: &UpstreamClient,
    payload: &ProviderAccountPayload,
    req: &CanonicalRelayRequest,
    model: &str,
    extra_headers: Option<&HashMap<String, String>>,
) -> Result<BinaryUpstreamResponse, GatewayError> {
    client
        .execute_gemini_web_reverse_legacy_tts(payload, req, model, extra_headers)
        .await
}
