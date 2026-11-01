//! Antigravity-only native client and pool. Never uses CPA's singleton, CA config or fallback.
use super::failure;
use crate::error::GatewayError;
#[cfg(feature = "cpa-rust-transport")]
use std::time::Duration;

pub(super) async fn send(
    builder: rquest::RequestBuilder,
) -> Result<rquest::Response, GatewayError> {
    #[cfg(not(feature = "cpa-rust-transport"))]
    {
        let _ = builder;
        Err(failure(
            "antigravity_transport_configuration",
            "Antigravity native transport is not enabled in this build",
        ))
    }
    #[cfg(feature = "cpa-rust-transport")]
    {
        use gateway_cpa_http_transport::{Client, Error, Initialization, Profile};
        static CLIENT: Initialization<Client> = Initialization::new();
        let deadline = tokio::time::Instant::now() + Duration::from_secs(180);
        let map = |error: Error| {
            failure(
                match error {
                    Error::Trust | Error::Target | Error::UnsupportedProxy | Error::Body => {
                        "antigravity_transport_configuration"
                    }
                    Error::Timeout => "antigravity_transport_timeout",
                    _ => "antigravity_transport_failed",
                },
                "Antigravity native HTTP failed; private details omitted; not replayed",
            )
        };
        let client = CLIENT
            .get(deadline, || {
                // Startup trust configuration only. No request-controlled CA or verification bypass.
                let ca = std::env::var_os("GATEWAY_ANTIGRAVITY_CA_CERTIFICATE")
                    .map(std::path::PathBuf::from);
                Client::with_profile_and_proxy(
                    Profile::AntigravityHub215,
                    ca.as_deref(),
                    false,
                    crate::http_client::native_proxy_matcher(),
                )
            })
            .await
            .map_err(map)?;
        let request = builder.build().map_err(|_| {
            failure(
                "antigravity_request_invalid",
                "Antigravity HTTP request could not be built",
            )
        })?;
        client
            .send_with_deadline(request, Some(deadline))
            .await
            .map_err(map)
    }
}
