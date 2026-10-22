//! Owned send operation: preserve legacy error classification while admitting optional CPA IO.
use crate::error::{classify_network_error, GatewayError};
use rquest::{RequestBuilder, Response};
use std::time::Duration;

pub(crate) struct Pending {
    builder: RequestBuilder,
    provider: String,
    rust: bool,
    timeout: Duration,
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
        })
    }

    pub(crate) async fn send(self) -> Result<Response, GatewayError> {
        #[cfg(feature = "cpa-rust-transport")]
        if self.rust {
            return send_rust(self.builder, &self.provider, self.timeout).await;
        }
        #[cfg(not(feature = "cpa-rust-transport"))]
        let _ = (self.rust, self.timeout);
        self.builder
            .send()
            .await
            .map_err(|e| classify_network_error(&e, Some(&self.provider)))
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
    client.send(request, remaining).await.map_err(map)
}
