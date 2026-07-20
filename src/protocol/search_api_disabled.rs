use std::collections::HashMap;

use serde_json::{Map, Value};

use crate::error::GatewayError;
use crate::implementation_lines;
use crate::protocol::canonical::{CanonicalRelayRequest, EndpointKind, ProtocolFamily};

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

pub fn normalize_search(_body: Value) -> Result<CanonicalRelayRequest, GatewayError> {
    Err(implementation_lines::search_api_family_compiled_out_error(
        "search normalize requested",
    ))
}

pub fn normalize_fetch(_body: Value) -> Result<CanonicalRelayRequest, GatewayError> {
    Err(implementation_lines::search_api_family_compiled_out_error(
        "fetch normalize requested",
    ))
}

pub fn normalize_research_create(_body: Value) -> Result<CanonicalRelayRequest, GatewayError> {
    Err(implementation_lines::search_api_family_compiled_out_error(
        "research create normalize requested",
    ))
}

pub fn normalize_research_list(
    _query_params: HashMap<String, String>,
) -> Result<CanonicalRelayRequest, GatewayError> {
    Err(implementation_lines::search_api_family_compiled_out_error(
        "research list normalize requested",
    ))
}

pub fn normalize_research_get(_research_id: &str) -> Result<CanonicalRelayRequest, GatewayError> {
    Err(implementation_lines::search_api_family_compiled_out_error(
        "research get normalize requested",
    ))
}

pub fn normalize_credits_balance() -> Result<CanonicalRelayRequest, GatewayError> {
    Err(implementation_lines::search_api_family_compiled_out_error(
        "credits balance normalize requested",
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

#[allow(dead_code)]
fn _protocol_family_marker() -> ProtocolFamily {
    ProtocolFamily::SearchApi
}
