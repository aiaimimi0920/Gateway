use super::*;
use serde_json::json;

fn payload(preset: &str) -> ProviderAccountPayload {
    let document = serde_json::from_value(json!({"providers":[{
        "id":preset, "preset":preset, "base_url":"https://example.com", "api_key":"synthetic"
    }], "model_routes":[]}))
    .unwrap();
    crate::routing::config::RouteConfigStore::from_document(document)
        .unwrap()
        .get_providers()
        .remove(0)
        .payload
}

#[test]
fn fetch_response_contract_rejects_empty_and_embedded_errors() {
    for (preset, body) in [
        ("you", json!([])),
        (
            "you",
            json!([{"url":"https://example.com", "error":"blocked"}]),
        ),
        (
            "tavily",
            json!({"results":[], "failed_results":[{"url":"https://example.com", "error":"blocked"}]}),
        ),
        (
            "websearchapi",
            json!({"code":500, "data":{"url":"https://example.com", "content":"not successful"}}),
        ),
        (
            "websearchapi",
            json!({"code":200, "data":{"url":"javascript:alert(1)", "content":"not a page"}}),
        ),
    ] {
        assert_eq!(
            validate(&payload(preset), &body, false)
                .unwrap_err()
                .http_status,
            Some(502)
        );
    }
}

#[test]
fn fetch_response_contract_accepts_content_and_retains_partial_failures() {
    for (preset, body) in [
        (
            "you",
            json!([{"url":"https://example.com", "markdown":"# Page"}]),
        ),
        (
            "tavily",
            json!({"results":[{"url":"https://example.com", "raw_content":"Page"}], "failed_results":[{"url":"https://example.org", "error":"blocked"}]}),
        ),
        (
            "websearchapi",
            json!({"code":200, "data":{"url":"https://example.com", "content":"Page"}}),
        ),
    ] {
        assert_eq!(validate(&payload(preset), &body, false).unwrap(), 1);
    }
}

#[test]
fn fetch_response_contract_does_not_mistake_metadata_for_page_content() {
    let body = json!([{"url":"https://example.com", "metadata":{"site_name":"Example"}}]);
    assert!(validate(&payload("you"), &body, false).is_err());
    assert_eq!(validate(&payload("you"), &body, true).unwrap(), 1);
}
