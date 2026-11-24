//! A process-local transport for the packaged Command Code acceptance fixtures.
use super::{sensenova_fixture, UpstreamClient};
use crate::{error::GatewayError, routing::candidate::ProviderAccountPayload};

const KEY_PREFIX: &str = "cpk-commandcode-fixture-";

pub(crate) fn load(config: &crate::config::Config) -> anyhow::Result<Option<rquest::Client>> {
    sensenova_fixture::load_isolated(
        config,
        "Command Code",
        "packaged-commandcode-",
        "GATEWAY_DEVTEST_COMMANDCODE_PROXY",
        "GATEWAY_DEVTEST_COMMANDCODE_CA",
    )
}

pub(crate) fn select<'a>(
    upstream: &'a UpstreamClient,
    payload: &ProviderAccountPayload,
    url: &str,
) -> Result<&'a rquest::Client, GatewayError> {
    let Some(http) = &upstream.commandcode_fixture_http else {
        return Ok(upstream.client());
    };
    // No fallback to the real network when an isolated request fails admission.
    if !crate::protocol::commandcode::owns_payload(payload) || !accepts(&payload.api_key, url) {
        return Err(GatewayError::bad_request(
            "Command Code fixture requires a synthetic request on an exact inference endpoint.",
        )
        .with_code("commandcode_fixture_request_invalid"));
    }
    Ok(http)
}

fn accepts(key: &str, url: &str) -> bool {
    matches!(
        url.strip_prefix(crate::protocol::commandcode::BASE),
        Some("/chat/completions" | "/messages" | "/responses" | "/systemone")
    ) && sensenova_fixture::namespaced_key(key, KEY_PREFIX)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::protocol::commandcode::BASE;

    #[test]
    fn commandcode_fixture_is_exact_to_synthetic_inference_targets() {
        let key = "cpk-commandcode-fixture-00112233445566778899aabbccddeeff";
        for path in ["/chat/completions", "/messages", "/responses", "/systemone"] {
            let target = format!("{BASE}{path}");
            assert!(accepts(key, &target));
            assert!(!accepts("normal-studio-key", &target));
        }
        for path in [
            "/models",
            "/systemone?extra=1",
            "/responses/old",
            "/messages?extra=1",
        ] {
            assert!(!accepts(key, &format!("{BASE}{path}")));
        }
        assert!(!accepts(
            key,
            "https://example.invalid/provider/v1/chat/completions"
        ));
    }
}
