//! Synthetic saved-account fixture; no real client, bearer or upstream request.
use neuro_gateway::routing::config::RouteConfigYaml;
use serde_json::{json, Value};

pub fn provider() -> Value {
    json!({"id":"luma-mcp","base_url":"https://agents.lumalabs.ai/mcp",
        "adapter":"lumalabs_mcp_readonly","protocol_profile":"lumalabs_mcp_readonly",
        "protocol_family":"lumalabs_mcp","auth_mode":"luma_mcp_oauth",
        "explicit_credentials_only":true,"credentials":[{
            "id":"saved-mcp","enabled":false,"api_key":"synthetic-access",
            "luma_oauth":{"issuer":"https://agents.lumalabs.ai",
                "resource":"https://agents.lumalabs.ai/mcp",
                "token_endpoint":"https://auth.lumalabs.ai/oauth2/token",
                "client_id":"registered-client","redirect_uri":"http://127.0.0.1:54421/callback"}}]})
}
pub fn document(provider: Value) -> RouteConfigYaml {
    serde_json::from_value(json!({"providers":[provider]})).unwrap()
}
