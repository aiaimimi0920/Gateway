//! Fetch contracts are bounded and never silently discard requested URLs.
use crate::{
    error::GatewayError,
    protocol::{canonical::CanonicalRelayRequest, search_api},
    routing::candidate::ProviderAccountPayload,
};
use serde_json::{json, Value};

pub(crate) fn target_urls(req: &CanonicalRelayRequest) -> Result<Vec<&str>, GatewayError> {
    let body = &req.raw_body;
    if body.get("url").is_some() && body.get("urls").is_some() {
        return Err(GatewayError::bad_request(
            "Specify either url or urls, not both",
        ));
    }
    let value = body.get("url").or_else(|| body.get("urls"));
    let urls = match value {
        Some(Value::String(url)) => vec![url.as_str()],
        Some(Value::Array(urls)) if !urls.is_empty() && urls.len() <= 20 => urls
            .iter()
            .map(|url| {
                url.as_str()
                    .ok_or_else(|| GatewayError::bad_request("urls must contain strings"))
            })
            .collect::<Result<Vec<_>, _>>()?,
        _ => {
            return Err(GatewayError::bad_request(
                "Fetch requires 1..20 HTTP(S) URLs",
            ))
        }
    };
    if urls.iter().any(|url| {
        url.len() > 8192
            || url::Url::parse(url).map_or(true, |parsed| {
                !matches!(parsed.scheme(), "http" | "https") || parsed.host_str().is_none()
            })
    }) {
        return Err(GatewayError::bad_request(
            "Fetch requires valid HTTP(S) URLs",
        ));
    }
    Ok(urls)
}

pub(crate) fn pack(
    payload: &ProviderAccountPayload,
    req: &CanonicalRelayRequest,
) -> Result<Value, GatewayError> {
    let urls = target_urls(req)?;
    let field = payload.fetch_urls_field.as_deref().unwrap_or("url");
    if field == "url" && urls.len() != 1 {
        return Err(GatewayError::bad_request(
            "Selected fetch provider supports only one URL per request",
        )
        .with_code("unsupported_search_provider_endpoint"));
    }
    let mut body = search_api::pack_fetch(req, field);
    let map = body
        .as_object_mut()
        .ok_or_else(|| GatewayError::bad_request("Fetch requires an object"))?;
    map.insert(
        field.into(),
        if field == "urls" {
            json!(urls)
        } else {
            json!(urls[0])
        },
    );
    match payload.fetch_path.as_deref() {
        Some("/v1/contents") => {
            map.entry("formats")
                .or_insert_with(|| json!(["markdown", "metadata"]));
        }
        Some("/scrape") => {
            map.entry("returnFormat")
                .or_insert_with(|| json!("markdown"));
        }
        _ => {}
    }
    Ok(body)
}

#[cfg(test)]
mod tests {
    use super::*;

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
    fn search_fetch_request_rejects_invalid_or_ambiguous_targets() {
        for targets in [
            json!({"urls":["https://example.com", "file:///private"]}),
            json!({"url":"https://example.com", "urls":["https://example.org"]}),
            json!({"urls":vec!["https://example.com";21]}),
            json!({"urls":["https://example.com", 42]}),
        ] {
            let request = search_api::normalize_fetch(targets).unwrap();
            assert!(pack(&payload("you"), &request).is_err());
        }
    }

    #[test]
    fn search_fetch_request_keeps_batches_or_rejects_them_before_io() {
        let request = search_api::normalize_fetch(
            json!({"urls":["https://example.com/a", "https://example.com/b"]}),
        )
        .unwrap();
        for preset in ["you", "tavily", "exa"] {
            let body = pack(&payload(preset), &request).unwrap();
            assert_eq!(body["urls"].as_array().unwrap().len(), 2);
        }
        for preset in ["linkup", "websearchapi"] {
            assert!(pack(&payload(preset), &request).is_err());
        }
    }

    #[test]
    fn search_tools_linkup_pool_adds_required_search_defaults() {
        let request =
            search_api::normalize_search(json!({"model":"search", "query":"Rust"})).unwrap();
        let plan = crate::upstream::search_provider_helpers::build_search_provider_request_plan(
            &payload("linkup"),
            &request,
        )
        .unwrap();
        assert_eq!(
            plan.body.unwrap(),
            json!({"q":"Rust", "depth":"standard", "outputType":"searchResults"})
        );
    }
}
