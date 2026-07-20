use std::collections::HashMap;

use serde_json::{Map, Value};

use crate::error::GatewayError;
use crate::protocol::canonical::{
    CanonicalMessage, CanonicalRelayRequest, ContentPart, EndpointKind, MessageRole, ProtocolFamily,
};

pub const SEARCH_ROUTE_MODEL: &str = "search-search";
pub const FETCH_ROUTE_MODEL: &str = "search-fetch";
pub const RESEARCH_ROUTE_MODEL: &str = "search-research";
pub const BALANCE_ROUTE_MODEL: &str = "search-balance";

pub const LEGACY_SHARED_SEARCH_ROUTE_MODEL: &str = "search-api-search";
pub const LEGACY_SHARED_FETCH_ROUTE_MODEL: &str = "search-api-fetch";
pub const LEGACY_SHARED_RESEARCH_ROUTE_MODEL: &str = "search-api-research";
pub const LEGACY_SHARED_BALANCE_ROUTE_MODEL: &str = "search-api-balance";

pub const LEGACY_SEARCH_ROUTE_MODEL: &str = "linkup-search";
pub const LEGACY_FETCH_ROUTE_MODEL: &str = "linkup-fetch";
pub const LEGACY_RESEARCH_ROUTE_MODEL: &str = "linkup-research";
pub const LEGACY_BALANCE_ROUTE_MODEL: &str = "linkup-balance";

pub fn resolve_legacy_search_route_model_alias(model: &str) -> Option<&'static str> {
    match model {
        SEARCH_ROUTE_MODEL | LEGACY_SHARED_SEARCH_ROUTE_MODEL => Some(LEGACY_SEARCH_ROUTE_MODEL),
        FETCH_ROUTE_MODEL | LEGACY_SHARED_FETCH_ROUTE_MODEL => Some(LEGACY_FETCH_ROUTE_MODEL),
        RESEARCH_ROUTE_MODEL | LEGACY_SHARED_RESEARCH_ROUTE_MODEL => {
            Some(LEGACY_RESEARCH_ROUTE_MODEL)
        }
        BALANCE_ROUTE_MODEL | LEGACY_SHARED_BALANCE_ROUTE_MODEL => Some(LEGACY_BALANCE_ROUTE_MODEL),
        _ => None,
    }
}

pub fn normalize_search(body: Value) -> Result<CanonicalRelayRequest, GatewayError> {
    normalize_post_request(
        body,
        EndpointKind::Search,
        SEARCH_ROUTE_MODEL,
        Some(&["q", "query"]),
        "Search endpoints do not support `stream: true`",
    )
}

pub fn normalize_fetch(body: Value) -> Result<CanonicalRelayRequest, GatewayError> {
    normalize_post_request(
        body,
        EndpointKind::Fetch,
        FETCH_ROUTE_MODEL,
        Some(&["url", "urls"]),
        "Fetch/extract endpoints do not support `stream: true`",
    )
}

pub fn normalize_research_create(body: Value) -> Result<CanonicalRelayRequest, GatewayError> {
    normalize_post_request(
        body,
        EndpointKind::ResearchCreate,
        RESEARCH_ROUTE_MODEL,
        Some(&["q", "input"]),
        "Research endpoints do not support `stream: true`",
    )
}

pub fn normalize_research_list(
    query_params: HashMap<String, String>,
) -> Result<CanonicalRelayRequest, GatewayError> {
    Ok(build_get_request(
        EndpointKind::ResearchList,
        RESEARCH_ROUTE_MODEL,
        map_to_json_object(query_params),
    ))
}

pub fn normalize_research_get(research_id: &str) -> Result<CanonicalRelayRequest, GatewayError> {
    let trimmed = research_id.trim();
    if trimmed.is_empty() {
        return Err(GatewayError::bad_request("missing research id"));
    }

    Ok(build_get_request(
        EndpointKind::ResearchGet,
        RESEARCH_ROUTE_MODEL,
        serde_json::json!({ "id": trimmed }),
    ))
}

pub fn normalize_credits_balance() -> Result<CanonicalRelayRequest, GatewayError> {
    Ok(build_get_request(
        EndpointKind::CreditsBalance,
        BALANCE_ROUTE_MODEL,
        Value::Object(Map::new()),
    ))
}

pub fn pack_search_api(req: &CanonicalRelayRequest) -> Value {
    let mut body = req.raw_body.clone();
    if let Value::Object(ref mut map) = body {
        map.remove("model");
        map.remove("stream");
    }
    body
}

pub fn pack_search(req: &CanonicalRelayRequest, query_field: &str) -> Value {
    let mut body = pack_search_api(req);
    if let Value::Object(ref mut map) = body {
        rewrite_search_query_field(map, query_field);
    }
    body
}

pub fn pack_fetch(req: &CanonicalRelayRequest, urls_field: &str) -> Value {
    let mut body = pack_search_api(req);
    if let Value::Object(ref mut map) = body {
        rewrite_fetch_target_field(map, urls_field);
    }
    body
}

pub fn extract_search_api_query(req: &CanonicalRelayRequest) -> Vec<(String, String)> {
    let Value::Object(map) = &req.raw_body else {
        return Vec::new();
    };

    map.iter()
        .filter_map(|(key, value)| match value {
            Value::Null => None,
            Value::String(s) => Some((key.clone(), s.clone())),
            Value::Bool(b) => Some((key.clone(), b.to_string())),
            Value::Number(n) => Some((key.clone(), n.to_string())),
            _ => None,
        })
        .collect()
}

pub fn research_id(req: &CanonicalRelayRequest) -> Option<&str> {
    req.raw_body
        .get("id")
        .and_then(|v| v.as_str())
        .map(str::trim)
        .filter(|v| !v.is_empty())
}

fn normalize_post_request(
    body: Value,
    endpoint_kind: EndpointKind,
    default_route_model: &str,
    required_text_fields: Option<&[&str]>,
    stream_error: &str,
) -> Result<CanonicalRelayRequest, GatewayError> {
    let body_obj = body.as_object().ok_or_else(|| {
        GatewayError::bad_request("Search-provider request body must be a JSON object")
    })?;

    if body_obj
        .get("stream")
        .and_then(|v| v.as_bool())
        .unwrap_or(false)
    {
        return Err(GatewayError::bad_request(stream_error));
    }

    let requested_model = body_obj
        .get("model")
        .and_then(|v| v.as_str())
        .map(str::trim)
        .filter(|v| !v.is_empty())
        .map(str::to_string)
        .or_else(|| Some(default_route_model.to_string()));

    let explicit_session_key = body_obj
        .get("user")
        .and_then(|v| v.as_str())
        .map(str::to_string);

    let messages = if let Some(fields) = required_text_fields {
        let value = extract_required_text_field(body_obj, fields).ok_or_else(|| {
            GatewayError::bad_request(format!(
                "missing or invalid one of required fields: {}",
                fields.join(", ")
            ))
        })?;

        vec![CanonicalMessage {
            role: MessageRole::User,
            content: vec![ContentPart::Text { text: value }],
            name: None,
            tool_call_id: None,
            tool_calls: vec![],
        }]
    } else {
        vec![]
    };

    Ok(CanonicalRelayRequest {
        protocol_family: ProtocolFamily::SearchApi,
        endpoint_kind,
        requested_model,
        stream: false,
        messages,
        tools: vec![],
        tool_choice: None,
        reasoning: None,
        metadata: None,
        raw_body: body,
        previous_response_id: None,
        explicit_session_key,
        extra: HashMap::new(),
    })
}

fn extract_required_text_field(map: &Map<String, Value>, fields: &[&str]) -> Option<String> {
    for field in fields {
        let Some(value) = map.get(*field) else {
            continue;
        };
        match value {
            Value::String(text) => {
                let trimmed = text.trim();
                if !trimmed.is_empty() {
                    return Some(trimmed.to_string());
                }
            }
            Value::Array(values) => {
                let joined = values
                    .iter()
                    .filter_map(Value::as_str)
                    .map(str::trim)
                    .filter(|value| !value.is_empty())
                    .collect::<Vec<_>>()
                    .join("\n");
                if !joined.is_empty() {
                    return Some(joined);
                }
            }
            _ => {}
        }
    }

    None
}

fn rewrite_search_query_field(map: &mut Map<String, Value>, query_field: &str) {
    match query_field {
        "query" => {
            if !map.contains_key("query") {
                if let Some(value) = map.remove("q") {
                    map.insert("query".to_string(), value);
                }
            }
        }
        "q" => {
            if !map.contains_key("q") {
                if let Some(value) = map.remove("query") {
                    map.insert("q".to_string(), value);
                }
            }
        }
        _ => {}
    }
}

fn rewrite_fetch_target_field(map: &mut Map<String, Value>, urls_field: &str) {
    match urls_field {
        "urls" => {
            if !map.contains_key("urls") {
                if let Some(Value::String(url)) = map.remove("url") {
                    map.insert("urls".to_string(), Value::Array(vec![Value::String(url)]));
                }
            }
        }
        "url" => {
            if !map.contains_key("url") {
                if let Some(Value::Array(urls)) = map.remove("urls") {
                    if let Some(first_url) = urls.into_iter().find_map(|value| match value {
                        Value::String(text) if !text.trim().is_empty() => Some(Value::String(text)),
                        _ => None,
                    }) {
                        map.insert("url".to_string(), first_url);
                    }
                }
            }
        }
        _ => {}
    }
}

fn build_get_request(
    endpoint_kind: EndpointKind,
    default_route_model: &str,
    raw_body: Value,
) -> CanonicalRelayRequest {
    CanonicalRelayRequest {
        protocol_family: ProtocolFamily::SearchApi,
        endpoint_kind,
        requested_model: Some(default_route_model.to_string()),
        stream: false,
        messages: vec![],
        tools: vec![],
        tool_choice: None,
        reasoning: None,
        metadata: None,
        raw_body,
        previous_response_id: None,
        explicit_session_key: None,
        extra: HashMap::new(),
    }
}

fn map_to_json_object(values: HashMap<String, String>) -> Value {
    let mut map = Map::new();
    for (key, value) in values {
        map.insert(key, Value::String(value));
    }
    Value::Object(map)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn normalize_search_uses_virtual_model_when_absent() {
        let req = normalize_search(json!({ "q": "latest rust async runtime benchmarks" })).unwrap();
        assert_eq!(req.endpoint_kind, EndpointKind::Search);
        assert_eq!(req.protocol_family, ProtocolFamily::SearchApi);
        assert_eq!(req.requested_model.as_deref(), Some(SEARCH_ROUTE_MODEL));
        assert_eq!(
            req.messages[0].text_content(),
            "latest rust async runtime benchmarks"
        );
    }

    #[test]
    fn normalize_search_accepts_tavily_query_field() {
        let req = normalize_search(json!({ "query": "Who is Leo Messi?" })).unwrap();
        assert_eq!(req.endpoint_kind, EndpointKind::Search);
        assert_eq!(req.requested_model.as_deref(), Some(SEARCH_ROUTE_MODEL));
        assert_eq!(req.messages[0].text_content(), "Who is Leo Messi?");
    }

    #[test]
    fn normalize_fetch_requires_url() {
        let req = normalize_fetch(json!({ "url": "https://example.com/foo" })).unwrap();
        assert_eq!(req.endpoint_kind, EndpointKind::Fetch);
        assert_eq!(req.requested_model.as_deref(), Some(FETCH_ROUTE_MODEL));
        assert_eq!(req.messages[0].text_content(), "https://example.com/foo");
    }

    #[test]
    fn normalize_fetch_accepts_tavily_urls_array() {
        let req = normalize_fetch(json!({
            "urls": ["https://example.com/a", "https://example.com/b"]
        }))
        .unwrap();
        assert_eq!(req.endpoint_kind, EndpointKind::Fetch);
        assert_eq!(
            req.messages[0].text_content(),
            "https://example.com/a\nhttps://example.com/b"
        );
    }

    #[test]
    fn normalize_research_requires_q() {
        let req =
            normalize_research_create(json!({ "q": "agent orchestration market map" })).unwrap();
        assert_eq!(req.endpoint_kind, EndpointKind::ResearchCreate);
        assert_eq!(req.requested_model.as_deref(), Some(RESEARCH_ROUTE_MODEL));
        assert_eq!(
            req.messages[0].text_content(),
            "agent orchestration market map"
        );
    }

    #[test]
    fn normalize_research_accepts_tavily_input_field() {
        let req = normalize_research_create(json!({
            "input": "What are the latest developments in AI?"
        }))
        .unwrap();
        assert_eq!(req.endpoint_kind, EndpointKind::ResearchCreate);
        assert_eq!(
            req.messages[0].text_content(),
            "What are the latest developments in AI?"
        );
    }

    #[test]
    fn normalize_research_list_uses_get_shape() {
        let req = normalize_research_list(HashMap::from([
            ("page".to_string(), "2".to_string()),
            ("limit".to_string(), "20".to_string()),
        ]))
        .unwrap();
        assert_eq!(req.endpoint_kind, EndpointKind::ResearchList);
        assert_eq!(req.requested_model.as_deref(), Some(RESEARCH_ROUTE_MODEL));
        assert!(req.messages.is_empty());
        assert_eq!(extract_search_api_query(&req).len(), 2);
    }

    #[test]
    fn normalize_research_get_stores_id() {
        let req = normalize_research_get("res_123").unwrap();
        assert_eq!(req.endpoint_kind, EndpointKind::ResearchGet);
        assert_eq!(research_id(&req), Some("res_123"));
    }

    #[test]
    fn normalize_balance_uses_virtual_model() {
        let req = normalize_credits_balance().unwrap();
        assert_eq!(req.endpoint_kind, EndpointKind::CreditsBalance);
        assert_eq!(req.requested_model.as_deref(), Some(BALANCE_ROUTE_MODEL));
    }

    #[test]
    fn normalize_post_request_rejects_streaming() {
        let err = normalize_fetch(json!({
            "url": "https://example.com",
            "stream": true
        }))
        .unwrap_err();
        assert_eq!(err.http_status, Some(400));
    }

    #[test]
    fn pack_search_api_strips_gateway_only_fields() {
        let req = normalize_search(json!({
            "model": SEARCH_ROUTE_MODEL,
            "q": "hello",
            "depth": "standard"
        }))
        .unwrap();
        let body = pack_search_api(&req);
        assert!(body.get("model").is_none());
        assert_eq!(body["q"], "hello");
        assert_eq!(body["depth"], "standard");
    }

    #[test]
    fn pack_search_can_rewrite_q_to_query() {
        let req = normalize_search(json!({
            "q": "exa search",
            "num_results": 5
        }))
        .unwrap();
        let body = pack_search(&req, "query");
        assert!(body.get("q").is_none());
        assert_eq!(body["query"], "exa search");
        assert_eq!(body["num_results"], 5);
    }

    #[test]
    fn pack_fetch_can_rewrite_url_to_urls() {
        let req = normalize_fetch(json!({
            "url": "https://example.com/a",
            "text": true
        }))
        .unwrap();
        let body = pack_fetch(&req, "urls");
        assert!(body.get("url").is_none());
        assert_eq!(body["urls"], json!(["https://example.com/a"]));
        assert_eq!(body["text"], true);
    }

    #[test]
    fn protocol_family_accepts_legacy_alias() {
        let req: CanonicalRelayRequest = serde_json::from_value(json!({
            "protocol_family": "linkup",
            "endpoint_kind": "search",
            "requested_model": SEARCH_ROUTE_MODEL,
            "stream": false,
            "messages": [],
            "tools": [],
            "tool_choice": null,
            "reasoning": null,
            "metadata": null,
            "raw_body": {}
        }))
        .unwrap();
        assert_eq!(req.protocol_family, ProtocolFamily::SearchApi);
    }

    #[test]
    fn virtual_route_models_use_search_api_prefix() {
        assert_eq!(SEARCH_ROUTE_MODEL, "search-search");
        assert_eq!(FETCH_ROUTE_MODEL, "search-fetch");
        assert_eq!(RESEARCH_ROUTE_MODEL, "search-research");
        assert_eq!(BALANCE_ROUTE_MODEL, "search-balance");
    }

    #[test]
    fn generic_virtual_models_resolve_to_legacy_route_model_aliases() {
        assert_eq!(
            resolve_legacy_search_route_model_alias(SEARCH_ROUTE_MODEL),
            Some(LEGACY_SEARCH_ROUTE_MODEL)
        );
        assert_eq!(
            resolve_legacy_search_route_model_alias(FETCH_ROUTE_MODEL),
            Some(LEGACY_FETCH_ROUTE_MODEL)
        );
        assert_eq!(
            resolve_legacy_search_route_model_alias(RESEARCH_ROUTE_MODEL),
            Some(LEGACY_RESEARCH_ROUTE_MODEL)
        );
        assert_eq!(
            resolve_legacy_search_route_model_alias(BALANCE_ROUTE_MODEL),
            Some(LEGACY_BALANCE_ROUTE_MODEL)
        );
        assert_eq!(
            resolve_legacy_search_route_model_alias(LEGACY_SHARED_SEARCH_ROUTE_MODEL),
            Some(LEGACY_SEARCH_ROUTE_MODEL)
        );
    }
}
