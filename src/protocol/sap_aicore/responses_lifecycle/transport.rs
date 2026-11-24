//! Prepare before claiming a send. Once execute is entered, no error is proof of non-delivery.
use super::{evidence::Evidence, failure, RequestPlan};
use crate::{
    error::GatewayError, routing::candidate::ProviderAccountPayload,
    upstream::client::UpstreamClient,
};
use serde_json::Value;

pub(super) enum DispatchError {
    NotSent(GatewayError),
    DeliveryUnknown,
    Recorded(String),
    UnsentRefund(String),
    SettlementPending(String),
}

impl From<GatewayError> for DispatchError {
    fn from(error: GatewayError) -> Self {
        Self::NotSent(error)
    }
}

// Private fields and no Clone: the owner must hand this value to execute only once.
pub(super) struct Prepared {
    http: rquest::Client,
    request: rquest::Request,
}

pub(super) struct Received {
    pub body: Value,
    pub status: u16,
}

impl Prepared {
    pub(super) async fn execute_until(
        self,
        deadline: Option<tokio::time::Instant>,
    ) -> Result<Received, Evidence> {
        let mut status = None;
        match deadline {
            Some(deadline) => tokio::time::timeout_at(deadline, self.execute(&mut status))
                .await
                .map_err(|_| {
                    Evidence::failed(failure("sap_aicore_response_delivery_unknown"), status)
                })?,
            None => self.execute(&mut status).await,
        }
    }

    /// Takes only plans constructed by the lifecycle planners from a validated deployment.
    /// OAuth, destination checks and local request construction all precede the inference send.
    pub(super) async fn prepare(
        client: &UpstreamClient,
        payload: &ProviderAccountPayload,
        plan: RequestPlan,
    ) -> Result<Self, DispatchError> {
        let http = client.sap_aicore.stored_http()?.clone();
        let token = client.sap_aicore.get(client.client(), payload).await?;
        #[cfg(test)]
        let url = super::super::test_support::url(&plan.url);
        #[cfg(not(test))]
        let url = plan.url;
        let mut builder = http
            .request(plan.method, url)
            .query(&plan.query)
            .default_headers(false)
            .timeout(client.timeout)
            .bearer_auth(token)
            .header("AI-Resource-Group", &payload.headers["AI-Resource-Group"])
            .header("accept", "application/json");
        if let Some(body) = plan.body {
            builder = builder.json(&body);
        }
        let request = builder
            .build()
            .map_err(|_| failure("sap_aicore_stored_request_unavailable"))?;
        Ok(Self { http, request })
    }

    async fn execute(self, observed_status: &mut Option<u16>) -> Result<Received, Evidence> {
        let response = self
            .http
            .execute(self.request)
            .await
            .map_err(|_| Evidence::failed(failure("sap_aicore_transport_failed"), None))?;
        let status = response.status().as_u16();
        *observed_status = Some(status);
        if !response.status().is_success() {
            return Err(Evidence::failed(
                super::super::errors::http_rejected(&response, false),
                Some(status),
            ));
        }
        let bytes = crate::protocol::upstream_body::collect_bounded_upstream_body(
            response,
            "SAP stored Responses",
        )
        .await
        .map_err(|_| Evidence::failed(failure("sap_aicore_body_incomplete"), Some(status)))?;
        let body = crate::protocol::unique_json::parse(&bytes)
            .map_err(|_| Evidence::failed(failure("sap_aicore_response_failed"), Some(status)))?;
        Ok(Received { body, status })
    }
}
