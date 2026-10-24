//! Explicit OpenRouter refresh: fixed same-origin endpoints, bounded public catalogue,
//! and generation evidence collected separately by the normal protocol probe.
use super::{catalogue, transport};

pub(super) const API_BASE: &str = "https://openrouter.ai/api/v1";

pub(super) fn matches(source: &str) -> bool {
    let Ok(url) = transport::checked_url(source) else {
        return false;
    };
    url.scheme() == "https"
        && url.host_str() == Some("openrouter.ai")
        && url.port().is_none()
        && matches!(url.path().trim_end_matches('/'), "/api" | "/api/v1")
}

pub(super) async fn load(client: &rquest::Client, key: &str) -> anyhow::Result<Vec<String>> {
    let response = client
        .get(format!("{API_BASE}/models"))
        .bearer_auth(key)
        .send()
        .await?;
    // OpenRouter includes descriptions/pricing in its catalogue; keep a finite larger cap.
    let body = transport::read_json_bounded(response, 8 * 1_048_576).await?;
    let models = catalogue::parse(&body);
    anyhow::ensure!(!models.is_empty(), "No model catalogue found.");
    Ok(models)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::provider_discovery::{endpoint_candidates, DiscoveredProtocol};

    #[test]
    fn known_bases_use_the_api_prefix() {
        for source in [
            "https://openrouter.ai/api",
            "https://openrouter.ai/api/",
            "https://openrouter.ai/api/v1",
            "https://openrouter.ai:443/api/v1/",
        ] {
            assert!(matches(source));
            assert_eq!(
                endpoint_candidates::bases(source, DiscoveredProtocol::ChatCompletions).unwrap(),
                vec![API_BASE.to_owned()]
            );
        }
    }

    #[test]
    fn unrelated_or_ambiguous_urls_keep_generic_behavior() {
        for source in [
            "http://openrouter.ai/api",
            "https://openrouter.ai.evil.test/api",
            "https://openrouter.ai:444/api",
            "https://user@openrouter.ai/api",
            "https://openrouter.ai/api?key=hidden",
            "https://openrouter.ai/api#fragment",
            "https://openrouter.ai/custom/api",
        ] {
            assert!(!matches(source), "{source}");
        }
    }
}
