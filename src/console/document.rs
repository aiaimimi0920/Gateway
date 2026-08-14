use std::collections::{HashMap, HashSet};
use std::error::Error;
use std::fmt;
use std::sync::Arc;

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use sha2::{Digest, Sha256};
use url::Url;

use super::secrets::is_sensitive_key;
use crate::routing::config::{
    compile_route_document, effective_credential_id, normalized_alias_conflicts,
    provider_default_account_id, subst_env, RouteConfigInner, RouteConfigYaml,
};

pub(crate) const MAX_CANONICAL_DOCUMENT_BYTES: usize = 16 * 1024 * 1024;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DiagnosticSeverity {
    Error,
    Warning,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct RouteConfigDiagnostic {
    pub code: String,
    pub severity: DiagnosticSeverity,
    pub path: String,
    pub message: String,
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct RouteConfigDiagnostics {
    pub diagnostics: Vec<RouteConfigDiagnostic>,
}

impl RouteConfigDiagnostics {
    pub fn requires_repair(&self) -> bool {
        self.diagnostics
            .iter()
            .any(|diagnostic| diagnostic.severity == DiagnosticSeverity::Error)
    }

    fn push_error(
        &mut self,
        code: impl Into<String>,
        path: impl Into<String>,
        message: impl Into<String>,
    ) {
        self.diagnostics.push(RouteConfigDiagnostic {
            code: code.into(),
            severity: DiagnosticSeverity::Error,
            path: path.into(),
            message: message.into(),
        });
    }

    fn sort(&mut self) {
        self.diagnostics.sort_by(|left, right| {
            left.path
                .cmp(&right.path)
                .then_with(|| left.code.cmp(&right.code))
                .then_with(|| left.message.cmp(&right.message))
        });
    }
}

impl fmt::Display for RouteConfigDiagnostics {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.diagnostics.is_empty() {
            return formatter.write_str("route configuration is valid");
        }
        for (index, diagnostic) in self.diagnostics.iter().enumerate() {
            if index > 0 {
                formatter.write_str("; ")?;
            }
            write!(
                formatter,
                "{} at {}: {}",
                diagnostic.code, diagnostic.path, diagnostic.message
            )?;
        }
        Ok(())
    }
}

impl Error for RouteConfigDiagnostics {}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct RouteConfigInspection {
    pub diagnostics: RouteConfigDiagnostics,
    pub requires_repair: bool,
}

#[derive(Clone, Eq, PartialEq)]
pub struct CanonicalRouteDocument {
    canonical_json: Vec<u8>,
    canonical_yaml: Vec<u8>,
    document_digest: String,
    yaml_digest: String,
}

impl fmt::Debug for CanonicalRouteDocument {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("CanonicalRouteDocument")
            .field("canonical_json_len", &self.canonical_json.len())
            .field("canonical_yaml_len", &self.canonical_yaml.len())
            .field("document_digest", &self.document_digest)
            .field("yaml_digest", &self.yaml_digest)
            .finish()
    }
}

impl CanonicalRouteDocument {
    pub fn canonical_json(&self) -> &[u8] {
        &self.canonical_json
    }

    pub fn canonical_yaml(&self) -> &[u8] {
        &self.canonical_yaml
    }

    pub fn document_digest(&self) -> &str {
        &self.document_digest
    }

    pub fn yaml_digest(&self) -> &str {
        &self.yaml_digest
    }
}

#[derive(Debug, thiserror::Error)]
pub enum CanonicalDocumentError {
    #[error("failed to serialize route configuration as JSON: {0}")]
    Json(#[from] serde_json::Error),
    #[error("failed to serialize route configuration as YAML: {0}")]
    Yaml(#[from] serde_yaml::Error),
    #[error(
        "canonical document size limit exceeded for {format}: {actual_bytes} bytes exceeds {max_bytes} bytes"
    )]
    TooLarge {
        format: &'static str,
        actual_bytes: usize,
        max_bytes: usize,
    },
}

#[derive(Clone)]
pub struct ValidatedRouteDocument {
    document: RouteConfigYaml,
    canonical_json: Vec<u8>,
    canonical_yaml: Vec<u8>,
    document_digest: String,
    yaml_digest: String,
    diagnostics: RouteConfigDiagnostics,
    compiled: Arc<RouteConfigInner>,
}

impl fmt::Debug for ValidatedRouteDocument {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ValidatedRouteDocument")
            .field("provider_count", &self.document.providers.len())
            .field("route_count", &self.document.model_routes.len())
            .field("document_digest", &self.document_digest)
            .field("yaml_digest", &self.yaml_digest)
            .field("diagnostics", &self.diagnostics)
            .finish()
    }
}

impl ValidatedRouteDocument {
    pub fn document(&self) -> &RouteConfigYaml {
        &self.document
    }

    pub fn canonical_json(&self) -> &[u8] {
        &self.canonical_json
    }

    pub fn canonical_yaml(&self) -> &[u8] {
        &self.canonical_yaml
    }

    pub fn document_digest(&self) -> &str {
        &self.document_digest
    }

    pub fn yaml_digest(&self) -> &str {
        &self.yaml_digest
    }

    pub fn diagnostics(&self) -> &RouteConfigDiagnostics {
        &self.diagnostics
    }

    #[allow(dead_code)]
    pub(crate) fn compiled(&self) -> Arc<RouteConfigInner> {
        self.compiled.clone()
    }
}

pub fn inspect_route_document(document: &RouteConfigYaml) -> RouteConfigInspection {
    let mut diagnostics = collect_document_diagnostics(document);
    if let Err(error) = compile_route_document(document.clone()) {
        diagnostics.push_error(
            "route_config_compile_failed",
            "/providers",
            error.to_string(),
        );
    }
    diagnostics.sort();
    RouteConfigInspection {
        requires_repair: diagnostics.requires_repair(),
        diagnostics,
    }
}

pub fn validate_route_document(
    document: RouteConfigYaml,
) -> Result<ValidatedRouteDocument, RouteConfigDiagnostics> {
    let inspection = inspect_route_document(&document);
    if inspection.requires_repair {
        return Err(inspection.diagnostics);
    }

    let normalized = materialize_credential_ids(document);
    let compiled = compile_route_document(normalized.clone()).map_err(|error| {
        let mut diagnostics = RouteConfigDiagnostics::default();
        diagnostics.push_error(
            "route_config_compile_failed",
            "/providers",
            error.to_string(),
        );
        diagnostics
    })?;
    let canonical = canonicalize_route_document(&normalized).map_err(|error| {
        let mut diagnostics = RouteConfigDiagnostics::default();
        let code = if matches!(&error, CanonicalDocumentError::TooLarge { .. }) {
            "route_config_document_too_large"
        } else {
            "route_config_canonicalization_failed"
        };
        diagnostics.push_error(code, "", error.to_string());
        diagnostics
    })?;

    Ok(ValidatedRouteDocument {
        document: normalized,
        canonical_json: canonical.canonical_json,
        canonical_yaml: canonical.canonical_yaml,
        document_digest: canonical.document_digest,
        yaml_digest: canonical.yaml_digest,
        diagnostics: inspection.diagnostics,
        compiled: Arc::new(compiled),
    })
}

#[cfg(test)]
mod tests {
    use super::validate_route_document;
    use crate::routing::config::{
        ModelRoute, ProviderConfigYaml, ProviderCredentialYaml, RouteConfigYaml,
    };
    use serde_json::Value;
    use std::collections::HashMap;

    #[test]
    fn account_groups_accept_explicit_and_provider_default_account_members() {
        let document = RouteConfigYaml {
            providers: vec![
                ProviderConfigYaml {
                    id: "managed-provider".to_string(),
                    label: Some("Managed OpenAI".to_string()),
                    vendor_key: None,
                    vendor_name: None,
                    preset: Some("openai".to_string()),
                    base_url: "https://api.example.com/v1".to_string(),
                    api_key: String::new(),
                    auth_token: None,
                    headers: HashMap::new(),
                    extra_body: HashMap::new(),
                    session_auth: None,
                    keepalive: None,
                    expires_at: None,
                    runtime_state_object_key: None,
                    account_name: None,
                    execution_mode: None,
                    endpoint_execution_modes: None,
                    default_model: None,
                    adapter: None,
                    protocol_family: None,
                    protocol_profile: None,
                    supported_models: vec![],
                    responses_path: None,
                    chat_completions_path: None,
                    completions_path: None,
                    embeddings_path: None,
                    audio_transcriptions_path: None,
                    audio_speech_path: None,
                    messages_path: None,
                    search_path: None,
                    fetch_path: None,
                    research_path: None,
                    balance_path: None,
                    search_query_field: None,
                    fetch_urls_field: None,
                    model_map: HashMap::new(),
                    pool_target_size: None,
                    auto_refill_enabled: false,
                    auto_prune_enabled: false,
                    credential_automation_driver_id: None,
                    credential_identity_categories: vec![],
                    credentials: vec![ProviderCredentialYaml {
                        id: Some("acc-prod-1".to_string()),
                        base_url: None,
                        api_key: Some("sk-prod-a".to_string()),
                        auth_token: None,
                        headers: HashMap::new(),
                        extra_body: HashMap::new(),
                        session_auth: None,
                        keepalive: None,
                        expires_at: None,
                        runtime_state_object_key: None,
                        account_name: Some("生产账号 A".to_string()),
                        credential_identity_category_id: None,
                        enabled: None,
                        execution_mode: None,
                        endpoint_execution_modes: None,
                        supported_models: vec!["gpt-5.4".to_string()],
                        refresh_token: None,
                        refresh_endpoint: None,
                        refresh_client_id: None,
                        token_expires_in_secs: None,
                    }],
                },
                ProviderConfigYaml {
                    id: "fallback-provider".to_string(),
                    label: Some("Fallback".to_string()),
                    vendor_key: None,
                    vendor_name: None,
                    preset: None,
                    base_url: "https://fallback.example.com/v1".to_string(),
                    api_key: "provider-level-key".to_string(),
                    auth_token: None,
                    headers: HashMap::new(),
                    extra_body: HashMap::new(),
                    session_auth: None,
                    keepalive: None,
                    expires_at: None,
                    runtime_state_object_key: None,
                    account_name: None,
                    execution_mode: None,
                    endpoint_execution_modes: None,
                    default_model: None,
                    adapter: None,
                    protocol_family: None,
                    protocol_profile: None,
                    supported_models: vec![],
                    responses_path: None,
                    chat_completions_path: None,
                    completions_path: None,
                    embeddings_path: None,
                    audio_transcriptions_path: None,
                    audio_speech_path: None,
                    messages_path: None,
                    search_path: None,
                    fetch_path: None,
                    research_path: None,
                    balance_path: None,
                    search_query_field: None,
                    fetch_urls_field: None,
                    model_map: HashMap::new(),
                    pool_target_size: None,
                    auto_refill_enabled: false,
                    auto_prune_enabled: false,
                    credential_automation_driver_id: None,
                    credential_identity_categories: vec![],
                    credentials: vec![],
                },
            ],
            model_routes: vec![ModelRoute {
                pattern: "gpt-5.4".to_string(),
                provider_ids: vec!["managed-provider".to_string()],
                priority: 100,
            }],
            aliases: HashMap::new(),
            account_groups: vec![crate::routing::config::AccountGroupYaml {
                id: "group-vip".to_string(),
                name: "VIP".to_string(),
                description: None,
                billing_multiplier: Some(1.5),
                enabled: Some(true),
                notes: None,
                provider_credential_ids: vec![
                    "acc-prod-1".to_string(),
                    "fallback-provider::default".to_string(),
                ],
            }],
        };

        let validated = validate_route_document(document).expect("document should validate");
        let account_groups_value =
            serde_json::to_value(validated.document()).expect("validated document JSON");
        let groups = account_groups_value
            .get("account_groups")
            .and_then(Value::as_array)
            .expect("account_groups should serialize");
        assert_eq!(groups.len(), 1);
    }

    #[test]
    fn account_groups_reject_unknown_account_members() {
        let document = RouteConfigYaml {
            providers: vec![ProviderConfigYaml {
                id: "managed-provider".to_string(),
                label: Some("Managed OpenAI".to_string()),
                vendor_key: None,
                vendor_name: None,
                preset: Some("openai".to_string()),
                base_url: "https://api.example.com/v1".to_string(),
                api_key: String::new(),
                auth_token: None,
                headers: HashMap::new(),
                extra_body: HashMap::new(),
                session_auth: None,
                keepalive: None,
                expires_at: None,
                runtime_state_object_key: None,
                account_name: None,
                execution_mode: None,
                endpoint_execution_modes: None,
                default_model: None,
                adapter: None,
                protocol_family: None,
                protocol_profile: None,
                supported_models: vec![],
                responses_path: None,
                chat_completions_path: None,
                completions_path: None,
                embeddings_path: None,
                audio_transcriptions_path: None,
                audio_speech_path: None,
                messages_path: None,
                search_path: None,
                fetch_path: None,
                research_path: None,
                balance_path: None,
                search_query_field: None,
                fetch_urls_field: None,
                model_map: HashMap::new(),
                pool_target_size: None,
                auto_refill_enabled: false,
                auto_prune_enabled: false,
                credential_automation_driver_id: None,
                credential_identity_categories: vec![],
                credentials: vec![ProviderCredentialYaml {
                    id: Some("acc-prod-1".to_string()),
                    base_url: None,
                    api_key: Some("sk-prod-a".to_string()),
                    auth_token: None,
                    headers: HashMap::new(),
                    extra_body: HashMap::new(),
                    session_auth: None,
                    keepalive: None,
                    expires_at: None,
                    runtime_state_object_key: None,
                    account_name: Some("生产账号 A".to_string()),
                    credential_identity_category_id: None,
                    enabled: None,
                    execution_mode: None,
                    endpoint_execution_modes: None,
                    supported_models: vec!["gpt-5.4".to_string()],
                    refresh_token: None,
                    refresh_endpoint: None,
                    refresh_client_id: None,
                    token_expires_in_secs: None,
                }],
            }],
            model_routes: vec![],
            aliases: HashMap::new(),
            account_groups: vec![crate::routing::config::AccountGroupYaml {
                id: "group-vip".to_string(),
                name: "VIP".to_string(),
                description: None,
                billing_multiplier: Some(1.2),
                enabled: Some(true),
                notes: None,
                provider_credential_ids: vec!["missing-account".to_string()],
            }],
        };

        let diagnostics = validate_route_document(document).expect_err("group should fail");
        assert!(
            diagnostics
                .diagnostics
                .iter()
                .any(|entry| entry.code == "account_group_member_unknown"),
            "expected account_group_member_unknown diagnostics, got {diagnostics:?}"
        );
    }
}

pub fn canonicalize_route_document(
    document: &RouteConfigYaml,
) -> Result<CanonicalRouteDocument, CanonicalDocumentError> {
    let value = serde_json::to_value(document)?;
    let sorted = sort_json_value(value);
    let canonical_json = serde_json::to_vec(&sorted)?;
    ensure_canonical_size("JSON", canonical_json.len())?;
    let mut yaml = serde_yaml::to_string(&sorted)?.replace("\r\n", "\n");
    if !yaml.ends_with('\n') {
        yaml.push('\n');
    }
    let canonical_yaml = yaml.into_bytes();
    ensure_canonical_size("YAML", canonical_yaml.len())?;

    Ok(CanonicalRouteDocument {
        document_digest: sha256_hex(&canonical_json),
        yaml_digest: sha256_hex(&canonical_yaml),
        canonical_json,
        canonical_yaml,
    })
}

fn ensure_canonical_size(
    format: &'static str,
    actual_bytes: usize,
) -> Result<(), CanonicalDocumentError> {
    if actual_bytes > MAX_CANONICAL_DOCUMENT_BYTES {
        return Err(CanonicalDocumentError::TooLarge {
            format,
            actual_bytes,
            max_bytes: MAX_CANONICAL_DOCUMENT_BYTES,
        });
    }
    Ok(())
}

pub(crate) fn materialize_credential_ids(mut document: RouteConfigYaml) -> RouteConfigYaml {
    for provider in &mut document.providers {
        let provider_id = provider.id.clone();
        for (index, credential) in provider.credentials.iter_mut().enumerate() {
            if credential.id.is_none() {
                credential.id = Some(format!("{provider_id}-cred-{index}"));
            }
        }
    }
    document
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum AccountIdentityKind {
    ProviderDefault,
    Credential,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct AccountIdentityOrigin {
    kind: AccountIdentityKind,
    provider_index: usize,
    credential_index: Option<usize>,
}

fn register_account_identity(
    account_ids: &mut HashMap<String, AccountIdentityOrigin>,
    account_id: String,
    origin: AccountIdentityOrigin,
    current_path: &str,
    diagnostics: &mut RouteConfigDiagnostics,
) {
    match account_ids.entry(account_id.clone()) {
        std::collections::hash_map::Entry::Vacant(entry) => {
            entry.insert(origin);
        }
        std::collections::hash_map::Entry::Occupied(entry) if entry.get().kind != origin.kind => {
            let first = entry.get();
            let first_path = match first.kind {
                AccountIdentityKind::ProviderDefault => {
                    format!(
                        "/providers/{}/id (provider default account)",
                        first.provider_index
                    )
                }
                AccountIdentityKind::Credential => format!(
                    "/providers/{}/credentials/{}/id",
                    first.provider_index,
                    first.credential_index.unwrap_or_default()
                ),
            };
            diagnostics.push_error(
                "account_identity_duplicate",
                format!("{current_path}/id"),
                format!("account identity '{account_id}' conflicts with {first_path}"),
            );
        }
        std::collections::hash_map::Entry::Occupied(_) => {}
    }
}

fn collect_document_diagnostics(document: &RouteConfigYaml) -> RouteConfigDiagnostics {
    let mut diagnostics = RouteConfigDiagnostics::default();
    let mut provider_ids = HashMap::<&str, usize>::new();
    let mut credential_ids = HashMap::<String, (usize, usize)>::new();
    let mut account_ids = HashMap::<String, AccountIdentityOrigin>::new();

    for (provider_index, provider) in document.providers.iter().enumerate() {
        let provider_path = format!("/providers/{provider_index}");
        let normalized_provider_id = provider.id.trim();
        if normalized_provider_id.is_empty() {
            diagnostics.push_error(
                "provider_id_empty",
                format!("{provider_path}/id"),
                "provider ID must not be empty",
            );
        } else if normalized_provider_id != provider.id {
            diagnostics.push_error(
                "provider_id_whitespace",
                format!("{provider_path}/id"),
                "provider ID must not have leading or trailing whitespace",
            );
        }
        if let Some(first_index) = provider_ids.insert(provider.id.as_str(), provider_index) {
            diagnostics.push_error(
                "provider_id_duplicate",
                format!("{provider_path}/id"),
                format!(
                    "provider ID '{}' duplicates /providers/{first_index}/id",
                    provider.id
                ),
            );
        }
        let websocket_transport = provider_uses_websocket_transport(provider);
        validate_provider_url(
            &provider.base_url,
            "provider_base_url_invalid",
            format!("{provider_path}/base_url"),
            websocket_transport,
            &mut diagnostics,
        );
        if let Some(keepalive) = &provider.keepalive {
            validate_http_url(
                &keepalive.service_url,
                "provider_keepalive_url_invalid",
                format!("{provider_path}/keepalive/serviceUrl"),
                &mut diagnostics,
            );
        }

        if provider.credentials.is_empty() {
            let default_account_id = provider_default_account_id(normalized_provider_id);
            register_account_identity(
                &mut account_ids,
                default_account_id,
                AccountIdentityOrigin {
                    kind: AccountIdentityKind::ProviderDefault,
                    provider_index,
                    credential_index: None,
                },
                &provider_path,
                &mut diagnostics,
            );
        }

        for (credential_index, credential) in provider.credentials.iter().enumerate() {
            let credential_path = format!("{provider_path}/credentials/{credential_index}");
            if let Some(raw_id) = credential.id.as_deref() {
                let normalized_id = raw_id.trim();
                if normalized_id.is_empty() {
                    diagnostics.push_error(
                        "credential_id_empty",
                        format!("{credential_path}/id"),
                        "credential ID must not be empty",
                    );
                } else if normalized_id != raw_id {
                    diagnostics.push_error(
                        "credential_id_whitespace",
                        format!("{credential_path}/id"),
                        "credential ID must not have leading or trailing whitespace",
                    );
                }
            }
            let credential_id =
                effective_credential_id(normalized_provider_id, credential_index, credential)
                    .trim()
                    .to_string();
            let duplicate_origin =
                credential_ids.insert(credential_id.clone(), (provider_index, credential_index));
            let credential_is_duplicate = duplicate_origin.is_some();
            if let Some((first_provider, first_credential)) = duplicate_origin {
                diagnostics.push_error(
                    "credential_id_duplicate",
                    format!("{credential_path}/id"),
                    format!(
                        "credential ID '{credential_id}' duplicates /providers/{first_provider}/credentials/{first_credential}/id"
                    ),
                );
            }
            if !credential_is_duplicate {
                register_account_identity(
                    &mut account_ids,
                    credential_id,
                    AccountIdentityOrigin {
                        kind: AccountIdentityKind::Credential,
                        provider_index,
                        credential_index: Some(credential_index),
                    },
                    &credential_path,
                    &mut diagnostics,
                );
            }
            if let Some(base_url) = &credential.base_url {
                validate_provider_url(
                    base_url,
                    "credential_base_url_invalid",
                    format!("{credential_path}/base_url"),
                    websocket_transport,
                    &mut diagnostics,
                );
            }
            if let Some(keepalive) = &credential.keepalive {
                validate_http_url(
                    &keepalive.service_url,
                    "credential_keepalive_url_invalid",
                    format!("{credential_path}/keepalive/serviceUrl"),
                    &mut diagnostics,
                );
            }
            if let Some(refresh_endpoint) = &credential.refresh_endpoint {
                validate_literal_http_url(
                    refresh_endpoint,
                    "credential_refresh_endpoint_invalid",
                    format!("{credential_path}/refresh_endpoint"),
                    &mut diagnostics,
                );
            }
            if let Some(expires_in_secs) = credential.token_expires_in_secs {
                if !token_expiry_supported(expires_in_secs) {
                    diagnostics.push_error(
                        "credential_token_expiry_invalid",
                        format!("{credential_path}/token_expires_in_secs"),
                        "token expiry cannot be represented safely by the runtime clock",
                    );
                }
            }
        }
    }

    let known_providers: HashSet<&str> = document
        .providers
        .iter()
        .map(|provider| provider.id.as_str())
        .collect();
    for (route_index, route) in document.model_routes.iter().enumerate() {
        let route_path = format!("/model_routes/{route_index}");
        if route.pattern.trim().is_empty() {
            diagnostics.push_error(
                "route_pattern_empty",
                format!("{route_path}/pattern"),
                "route pattern must not be empty",
            );
        }
        if route.provider_ids.is_empty() {
            diagnostics.push_error(
                "route_provider_ids_empty",
                format!("{route_path}/provider_ids"),
                "route must reference at least one provider",
            );
        }
        for (provider_index, provider_id) in route.provider_ids.iter().enumerate() {
            if !known_providers.contains(provider_id.as_str()) {
                diagnostics.push_error(
                    "route_provider_unknown",
                    format!("{route_path}/provider_ids/{provider_index}"),
                    format!("route references unknown provider '{provider_id}'"),
                );
            }
        }
    }

    let mut account_group_ids = HashMap::<&str, usize>::new();
    for (group_index, group) in document.account_groups.iter().enumerate() {
        let group_path = format!("/account_groups/{group_index}");
        let group_id = group.id.trim();
        if group_id.is_empty() {
            diagnostics.push_error(
                "account_group_id_empty",
                format!("{group_path}/id"),
                "account group ID must not be empty",
            );
        } else if let Some(first_index) = account_group_ids.insert(group_id, group_index) {
            diagnostics.push_error(
                "account_group_id_duplicate",
                format!("{group_path}/id"),
                format!(
                    "account group ID '{group_id}' duplicates /account_groups/{first_index}/id"
                ),
            );
        }

        if group.name.trim().is_empty() {
            diagnostics.push_error(
                "account_group_name_empty",
                format!("{group_path}/name"),
                "account group name must not be empty",
            );
        }

        if let Some(multiplier) = group.billing_multiplier {
            if !multiplier.is_finite() || multiplier < 0.0 {
                diagnostics.push_error(
                    "account_group_billing_multiplier_invalid",
                    format!("{group_path}/billing_multiplier"),
                    "account group billing multiplier must be a finite number greater than or equal to 0",
                );
            }
        }

        let mut member_ids = HashSet::<&str>::new();
        for (member_index, member_id) in group.provider_credential_ids.iter().enumerate() {
            let member_path = format!("{group_path}/provider_credential_ids/{member_index}");
            let trimmed_member_id = member_id.trim();
            if trimmed_member_id.is_empty() {
                diagnostics.push_error(
                    "account_group_member_empty",
                    member_path,
                    "account group member ID must not be empty",
                );
                continue;
            }
            if !member_ids.insert(trimmed_member_id) {
                diagnostics.push_error(
                    "account_group_member_duplicate",
                    member_path.clone(),
                    format!(
                        "account group member '{trimmed_member_id}' is duplicated within the same group"
                    ),
                );
            }
            if !account_ids.contains_key(trimmed_member_id) {
                diagnostics.push_error(
                    "account_group_member_unknown",
                    member_path,
                    format!("account group references unknown account '{trimmed_member_id}'"),
                );
            }
        }
    }

    collect_alias_diagnostics(&document.aliases, &mut diagnostics);
    for (_normalized, _first_key, second_key, targets) in
        normalized_alias_conflicts(&document.aliases)
    {
        diagnostics.push_error(
            "alias_normalized_collision",
            format!("/aliases/{}", encode_pointer_segment(&second_key)),
            format!("normalized alias key conflicts with another alias ({targets})"),
        );
    }
    diagnostics.sort();
    diagnostics
}

fn validate_http_url(
    value: &str,
    code: &str,
    path: String,
    diagnostics: &mut RouteConfigDiagnostics,
) {
    let resolved = subst_env(value);
    validate_http_url_inner(&resolved, code, path, diagnostics);
}

fn validate_literal_http_url(
    value: &str,
    code: &str,
    path: String,
    diagnostics: &mut RouteConfigDiagnostics,
) {
    validate_http_url_inner(value, code, path, diagnostics);
}

fn validate_http_url_inner(
    value: &str,
    code: &str,
    path: String,
    diagnostics: &mut RouteConfigDiagnostics,
) {
    let Ok(url) = Url::parse(value) else {
        diagnostics.push_error(code, path, "URL must use HTTP or HTTPS and include a host");
        return;
    };
    if !matches!(url.scheme(), "http" | "https") || url.host_str().is_none() {
        diagnostics.push_error(
            code,
            path.clone(),
            "URL must use HTTP or HTTPS and include a host",
        );
    }
    validate_url_safety(&url, &path, code, diagnostics);
}

fn validate_provider_url(
    value: &str,
    code: &str,
    path: String,
    websocket_transport: bool,
    diagnostics: &mut RouteConfigDiagnostics,
) {
    let resolved = subst_env(value);
    let Ok(url) = Url::parse(&resolved) else {
        let expected = if websocket_transport {
            "HTTP(S) or WebSocket (WS/WSS)"
        } else {
            "HTTP or HTTPS"
        };
        diagnostics.push_error(
            code,
            path,
            format!("URL must use {expected} and include a host"),
        );
        return;
    };
    let scheme_allowed = matches!(url.scheme(), "http" | "https")
        || (websocket_transport && matches!(url.scheme(), "ws" | "wss"));
    if !scheme_allowed || url.host_str().is_none() {
        let expected = if websocket_transport {
            "HTTP(S) or WebSocket (WS/WSS)"
        } else {
            "HTTP or HTTPS"
        };
        diagnostics.push_error(
            code,
            path.clone(),
            format!("URL must use {expected} and include a host"),
        );
    }
    validate_url_safety(&url, &path, code, diagnostics);
}

fn validate_url_safety(
    url: &Url,
    path: &str,
    code: &str,
    diagnostics: &mut RouteConfigDiagnostics,
) {
    let field = code.strip_suffix("_invalid").unwrap_or(code);
    if !url.username().is_empty() || url.password().is_some() {
        diagnostics.push_error(
            format!("{field}_userinfo_forbidden"),
            path,
            "URL userinfo is not permitted in route configuration",
        );
    }
    if url
        .query_pairs()
        .any(|(key, _)| is_sensitive_key(key.as_ref()))
    {
        diagnostics.push_error(
            format!("{field}_query_secret_forbidden"),
            path,
            "URL query keys classified as sensitive are not permitted",
        );
    }
    if url.fragment().is_some() {
        diagnostics.push_error(
            format!("{field}_fragment_forbidden"),
            path,
            "URL fragments are not permitted in route configuration",
        );
    }
}

fn token_expiry_supported(seconds: u64) -> bool {
    let duration = std::time::Duration::from_secs(seconds);
    std::time::Instant::now().checked_add(duration).is_some()
        && std::time::SystemTime::now().checked_add(duration).is_some()
}

fn provider_uses_websocket_transport(
    provider: &crate::routing::config::ProviderConfigYaml,
) -> bool {
    [
        provider.preset.as_deref(),
        provider.adapter.as_deref(),
        provider.protocol_family.as_deref(),
        provider.protocol_profile.as_deref(),
    ]
    .into_iter()
    .flatten()
    .any(|value| {
        let normalized = value.trim().to_ascii_lowercase().replace('_', "-");
        normalized == "xfyun-websocket"
            || normalized == "xfyun-native-websocket"
            || normalized.contains("websocket")
    })
}

fn collect_alias_diagnostics(
    aliases: &HashMap<String, String>,
    diagnostics: &mut RouteConfigDiagnostics,
) {
    let mut keys: Vec<&str> = aliases.keys().map(String::as_str).collect();
    keys.sort_unstable();
    let mut reported = HashSet::<String>::new();
    let mut visited = HashSet::<String>::new();

    for start in keys {
        if visited.contains(start) {
            continue;
        }
        let mut positions = HashMap::<String, usize>::new();
        let mut chain = Vec::<String>::new();
        let mut current = start.to_string();
        while aliases.contains_key(&current) && !visited.contains(&current) {
            if let Some(position) = positions.get(&current).copied() {
                let mut cycle = chain[position..].to_vec();
                cycle.sort();
                let signature = cycle.join("\0");
                if reported.insert(signature) {
                    let alias = cycle.first().cloned().unwrap_or(current);
                    diagnostics.push_error(
                        "alias_cycle",
                        format!("/aliases/{}", encode_pointer_segment(&alias)),
                        format!("alias cycle includes {}", cycle.join(" -> ")),
                    );
                }
                break;
            }
            positions.insert(current.clone(), chain.len());
            chain.push(current.clone());
            let Some(next) = aliases.get(&current) else {
                break;
            };
            current = next.clone();
        }
        visited.extend(chain);
    }
}

fn sort_json_value(value: Value) -> Value {
    match value {
        Value::Object(map) => {
            let mut entries: Vec<(String, Value)> = map.into_iter().collect();
            entries.sort_by(|left, right| left.0.cmp(&right.0));
            let mut sorted = Map::new();
            for (key, value) in entries {
                sorted.insert(key, sort_json_value(value));
            }
            Value::Object(sorted)
        }
        Value::Array(values) => Value::Array(values.into_iter().map(sort_json_value).collect()),
        other => other,
    }
}

fn sha256_hex(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    hex::encode(hasher.finalize())
}

pub(crate) fn encode_pointer_segment(segment: &str) -> String {
    segment.replace('~', "~0").replace('/', "~1")
}
