use serde_json::json;

pub(crate) fn make_payload(
    adapter: &str,
    base_url: &str,
) -> crate::routing::candidate::ProviderAccountPayload {
    serde_json::from_value(json!({
        "adapter": adapter,
        "baseUrl": base_url,
        "apiKey": "sk-test"
    }))
    .expect("payload")
}
