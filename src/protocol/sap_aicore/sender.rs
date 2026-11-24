//! A request owns its short-lived token and can be sent exactly once after admission.
use super::*;
use crate::{protocol::canonical::CanonicalRelayRequest, upstream::client::UpstreamClient};

pub(crate) struct Prepared {
    http: rquest::Client,
    request: rquest::Request,
    token: String,
}

impl Prepared {
    pub(crate) async fn prepare(
        client: &UpstreamClient,
        payload: &ProviderAccountPayload,
        req: &CanonicalRelayRequest,
        model: &str,
        stream: bool,
    ) -> Result<Self, GatewayError> {
        let plan = runtime::plan(payload, req, model, stream)?;
        let http = client.sap_aicore.stored_http()?.clone();
        let token = client.sap_aicore.get(client.client(), payload).await?;
        #[cfg(test)]
        let url = super::test_support::url(&plan.url);
        #[cfg(not(test))]
        let url = plan.url;
        let request = http
            .request(plan.method, url)
            .query(&plan.query)
            .json(&plan.body)
            .default_headers(false)
            .timeout(client.timeout)
            .bearer_auth(&token)
            .header("AI-Resource-Group", &payload.headers["AI-Resource-Group"])
            .header(
                "accept",
                if stream && bedrock::selected(payload) {
                    bedrock::EVENTSTREAM
                } else if stream {
                    "text/event-stream"
                } else {
                    "application/json"
                },
            )
            .build()
            .map_err(|_| failure("sap_aicore_request_unavailable"))?;
        Ok(Self {
            http,
            request,
            token,
        })
    }

    pub(crate) async fn send(self) -> Result<(rquest::Response, String), GatewayError> {
        let response = self
            .http
            .execute(self.request)
            .await
            .map_err(|_| failure("sap_aicore_transport_failed"))?;
        if !response.status().is_success() {
            return Err(errors::http_rejected(&response, false));
        }
        Ok((response, self.token))
    }
}
