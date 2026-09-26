use super::*;
use crate::protocol::registry::{LINKUP_SEARCH_FAMILY, PERPLEXITY_SEARCH_FAMILY};
use crate::protocol::search_api::{
    BALANCE_ROUTE_MODEL, FETCH_ROUTE_MODEL, LEGACY_BALANCE_ROUTE_MODEL, LEGACY_FETCH_ROUTE_MODEL,
    LEGACY_RESEARCH_ROUTE_MODEL, LEGACY_SEARCH_ROUTE_MODEL, RESEARCH_ROUTE_MODEL,
    SEARCH_ROUTE_MODEL,
};
use crate::routing::candidate::SEARCH_API_COMPATIBLE_ADAPTER;
use serde_json::json;

mod account_groups;
mod api_presets;
mod browser_presets;
mod candidates;
mod credentials;
mod probes;
mod refresh;
mod rules;

fn make_store_from_yaml(yaml: &str) -> RouteConfigStore {
    let config: RouteConfigYaml = serde_yaml::from_str(yaml).expect("parse yaml");
    RouteConfigStore::from_unchecked_document(
        config,
        ActiveConfigSource::Database,
        RevisionActor::Bootstrap,
        0,
        None,
        None,
    )
    .expect("compile")
}

fn multi_provider_store() -> RouteConfigStore {
    let yaml = r#"
providers:
  - id: anthropic-default
    preset: anthropic
    base_url: "https://api.anthropic.com"
    api_key: "k-ant"
  - id: openai-default
    preset: openai
    base_url: "https://api.openai.com"
    api_key: "k-oai"
  - id: codex-main
    preset: codex
    base_url: "https://chatgpt.com/backend-api/codex"
    api_key: "tok"
    supported_models:
      - gpt-5.4
      - gpt-5.4-mini
      - gpt-5.3-codex
      - gpt-5.2

model_routes:
  - pattern: "claude-*"
    provider_ids: [anthropic-default]
    priority: 10
  - pattern: "gpt-*"
    provider_ids: [openai-default, codex-main]
    priority: 10
  - pattern: "gpt-5.3-codex*"
    provider_ids: [codex-main]
    priority: 20

aliases:
  sonnet: claude-sonnet-4-6
  codex: gpt-5-codex
  gpt-5: gpt-5.4
  gpt-5-mini: gpt-5.4-mini
  gpt-5-codex: gpt-5.3-codex
  gpt-5-classic: gpt-5.2
"#;
    make_store_from_yaml(yaml)
}

fn oauth_fixture(endpoint: &str, api_key: &str) -> RouteConfigYaml {
    serde_yaml::from_str(&format!(
        r#"
providers:
  - id: oauth-provider
    base_url: https://example.com/v1
    credentials:
      - id: oauth-credential
        api_key: {api_key}
        refresh_token: refresh-token
        refresh_endpoint: {endpoint}
        token_expires_in_secs: 3600
model_routes:
  - pattern: oauth-model
    provider_ids: [oauth-provider]
aliases: {{}}
"#
    ))
    .expect("oauth fixture must parse")
}
