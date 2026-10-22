//! CPA transport selection and fail-closed migration from the retired Go IPC path.
use crate::{error::GatewayError, routing::candidate::ProviderAccountPayload};

pub(crate) fn rust_enabled(payload: &ProviderAccountPayload) -> Result<bool, GatewayError> {
    select(
        payload.request_client_profile.unwrap_or_default().is_cpa(),
        std::env::var("GATEWAY_CPA_TRANSPORT").ok().as_deref(),
    )
}

fn select(cpa: bool, configured: Option<&str>) -> Result<bool, GatewayError> {
    if !cpa {
        return Ok(false);
    }
    match configured {
        None | Some("rust") => Ok(true),
        Some("go") => Err(GatewayError::bad_request(
            "CPA Go worker support was removed; remove GATEWAY_CPA_TRANSPORT=go or set it to rust",
        )
        .with_code("cpa_transport_configuration")),
        Some(_) => Err(GatewayError::bad_request(
            "CPA transport must be rust; no legacy transport fallback is available",
        )
        .with_code("cpa_transport_configuration")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn cpa_transport_defaults_to_rust_and_rejects_retired_or_unknown_modes() {
        assert!(select(true, None).unwrap());
        assert!(select(true, Some("rust")).unwrap());
        assert!(select(true, Some("go")).is_err());
        assert!(select(true, Some("legacy")).is_err());
        assert!(!select(false, Some("go")).unwrap());
    }
}
