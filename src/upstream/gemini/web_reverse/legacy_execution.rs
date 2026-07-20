use std::collections::HashMap;

use serde_json::Value;

use crate::error::GatewayError;
use crate::protocol::canonical::CanonicalRelayRequest;
use crate::routing::candidate::ProviderAccountPayload;
use crate::upstream::client::UpstreamClient;

impl UpstreamClient {
    pub(crate) async fn execute_gemini_web_reverse_legacy_media(
        &self,
        provider_account_id: &str,
        payload: &ProviderAccountPayload,
        req: &CanonicalRelayRequest,
        model: &str,
        extra_headers: Option<&HashMap<String, String>>,
    ) -> Result<Value, GatewayError> {
        self.execute_gemini_canvas_media(provider_account_id, payload, req, model, extra_headers)
            .await
    }
}

pub async fn execute_legacy_media(
    client: &UpstreamClient,
    provider_account_id: &str,
    payload: &ProviderAccountPayload,
    req: &CanonicalRelayRequest,
    model: &str,
    extra_headers: Option<&HashMap<String, String>>,
) -> Result<Value, GatewayError> {
    client
        .execute_gemini_web_reverse_legacy_media(
            provider_account_id,
            payload,
            req,
            model,
            extra_headers,
        )
        .await
}
