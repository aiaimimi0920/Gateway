//! Owned send operation: preserve legacy error classification while admitting optional CPA IO.
use crate::error::{classify_network_error, GatewayError};
use rquest::{RequestBuilder, Response};
use std::time::Duration;

pub(crate) struct Pending {
    builder: RequestBuilder,
    provider: String,
    rust: bool,
    timeout: Duration,
    cloudflare: Option<super::cloudflare_neuron_guard::Check>,
    vercel: Option<super::vercel_credit_guard::Check>,
    kilo_free: bool,
}
impl Pending {
    pub(crate) fn new(
        builder: RequestBuilder,
        provider: &str,
        rust: bool,
        timeout: Duration,
    ) -> Result<Self, GatewayError> {
        if rust && !cfg!(feature = "cpa-rust-transport") {
            return Err(GatewayError::bad_request(
                "CPA Rust transport is not enabled in this build",
            )
            .with_code("cpa_transport_configuration"));
        }
        Ok(Self {
            builder,
            provider: provider.into(),
            rust,
            timeout,
            cloudflare: None,
            vercel: None,
            kilo_free: false,
        })
    }

    pub(crate) fn with_kilo_free(mut self, enabled: bool) -> Self {
        self.kilo_free = enabled;
        self
    }

    pub(crate) fn with_provider(mut self, provider: &str) -> Self {
        self.provider = provider.into();
        self
    }

    pub(crate) fn with_cloudflare(
        mut self,
        check: Option<super::cloudflare_neuron_guard::Check>,
    ) -> Self {
        self.cloudflare = check;
        self
    }

    pub(crate) fn with_vercel(mut self, check: Option<super::vercel_credit_guard::Check>) -> Self {
        self.vercel = check;
        self
    }

    pub(crate) async fn send(self) -> Result<Response, GatewayError> {
        if let Some(check) = self.cloudflare {
            check
                .admit()
                .await
                .map_err(|error| error.with_provider(&self.provider))?;
        }
        let metered = match &self.vercel {
            Some(check) => check
                .admit()
                .await
                .map_err(|error| error.with_provider(&self.provider))?,
            None => None,
        };
        crate::pipeline::stage_rate_limit::quota_send::after_quota().await?;
        #[cfg(feature = "cpa-rust-transport")]
        if self.rust {
            return send_rust(self.builder, &self.provider, self.timeout).await;
        }
        #[cfg(not(feature = "cpa-rust-transport"))]
        let _ = (self.rust, self.timeout);
        let response = self
            .builder
            .send()
            .await
            .map_err(|e| classify_network_error(&e, Some(&self.provider)))?;
        if let Some(permit) = metered.filter(|_| response.status().as_u16() == 402) {
            return Err(super::vercel_credit_guard::payment_error(
                response,
                &self.provider,
                permit,
            )
            .await);
        }
        if self.kilo_free && response.status().as_u16() == 429 {
            return Err(crate::protocol::kilo_free::rate_limit(response.headers())
                .with_provider(&self.provider));
        }
        Ok(response)
    }
}

#[cfg(feature = "cpa-rust-transport")]
async fn send_rust(
    builder: RequestBuilder,
    provider: &str,
    timeout: Duration,
) -> Result<Response, GatewayError> {
    use crate::error::classify_native_transport_failure;
    use gateway_cpa_http_transport::{Client, Error, Initialization};
    static CLIENT: Initialization<Client> = Initialization::new();
    let deadline = tokio::time::Instant::now() + timeout;
    let map = |error: Error| match error {
        Error::Trust | Error::Target | Error::UnsupportedProxy | Error::Body => {
            GatewayError::bad_request(error.to_string())
                .with_code("cpa_transport_configuration")
                .with_provider(provider)
        }
        _ => classify_native_transport_failure(error == Error::Timeout, provider),
    };
    let client = CLIENT
        .get(deadline, || {
            // Trust and proxy policy are instance-startup configuration, never caller headers.
            let ca = std::env::var_os("GATEWAY_CPA_CA_CERTIFICATE").map(std::path::PathBuf::from);
            let test = std::env::var("GATEWAY_CPA_TEST_HTTP").ok().as_deref() == Some("1");
            Client::new(ca.as_deref(), test)
        })
        .await
        .map_err(map)?;
    let request = builder
        .build()
        .map_err(|e| classify_network_error(&e, Some(provider)))?;
    let remaining = deadline
        .checked_duration_since(tokio::time::Instant::now())
        .ok_or_else(|| map(Error::Timeout))?;
    if crate::request_wait::transport::managed() {
        client.send_with_deadline(request, None).await.map_err(map)
    } else {
        client.send(request, remaining).await.map_err(map)
    }
}
