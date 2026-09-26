//! Console document contracts share YAML fixtures in one integration-test target.

#[path = "console_document_contract/validation.rs"]
mod validation;

#[path = "console_document_contract/canonical_revision.rs"]
mod canonical_revision;

#[path = "console_document_contract/redaction.rs"]
mod redaction;

#[path = "console_document_contract/secret_operations.rs"]
mod secret_operations;

#[path = "console_document_contract/secret_identity.rs"]
mod secret_identity;

#[path = "console_document_contract/secret_arrays.rs"]
mod secret_arrays;

#[path = "console_document_contract/secret_pointers.rs"]
mod secret_pointers;

#[path = "console_document_contract/resource_limits.rs"]
mod resource_limits;

#[path = "console_document_contract/urls.rs"]
mod urls;

use neuro_gateway::routing::config::RouteConfigYaml;

fn document(yaml: &str) -> RouteConfigYaml {
    serde_yaml::from_str(yaml).expect("fixture YAML must parse")
}

fn provider_document(provider_id: &str, api_key: &str) -> RouteConfigYaml {
    document(&format!(
        r#"
providers:
  - id: {provider_id}
    base_url: https://example.com
    api_key: {api_key:?}
model_routes: []
aliases: {{}}
"#
    ))
}

fn diagnostic_codes(error: &neuro_gateway::console::document::RouteConfigDiagnostics) -> Vec<&str> {
    error
        .diagnostics
        .iter()
        .map(|diagnostic| diagnostic.code.as_str())
        .collect()
}
