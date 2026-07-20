use std::time::Duration;

use rquest::Method;
use serde_json::Value;

use crate::error::{classify_network_error, classify_upstream_error, GatewayError};
use crate::protocol::aistudio_web;
use crate::routing::candidate::ProviderAccountPayload;
use crate::upstream::client::UpstreamClient;

impl UpstreamClient {
    pub(crate) async fn execute_aistudio_fixture_text_request(
        &self,
        payload: &ProviderAccountPayload,
        model: &str,
        request_body: &Value,
    ) -> Result<String, GatewayError> {
        let provider = aistudio_web::AISTUDIO_WEB_REVERSE_ADAPTER;
        let url = aistudio_web::build_generate_content_url(&payload.base_url, model);
        let response = self
            .http
            .request(Method::POST, &url)
            .header(rquest::header::CONTENT_TYPE, "application/json")
            .json(request_body)
            .timeout(self.timeout.max(Duration::from_secs(60)))
            .send()
            .await
            .map_err(|error| classify_network_error(&error, Some(provider)))?;
        let status = response.status().as_u16();
        let body_text = response
            .text()
            .await
            .map_err(|error| classify_network_error(&error, Some(provider)))?;
        if !(200..300).contains(&status) {
            return Err(classify_upstream_error(status, &body_text, Some(provider)));
        }
        Ok(body_text)
    }
}
