//! Rate-limit policy selection and canonical tenant-scoped Redis keys.

use super::{RateLimitDimensions, RateLimitRule, RateLimitScope, RateLimitScopeKey};
use crate::db::{GatewayRateLimitDefinition, GatewayRoutePolicyConfig};
use crate::error::GatewayError;
use crate::protocol::canonical::EndpointKind;
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::fmt::Write;

pub fn build_request_rate_limit_rules(
    config: &GatewayRoutePolicyConfig,
    dimensions: &RateLimitDimensions<'_>,
) -> Result<Vec<RateLimitRule>, GatewayError> {
    if config.rate_limit_enforcement_version.as_deref() != Some("v1") {
        return Ok(Vec::new());
    }
    let keys = build_rate_limit_scope_keys(dimensions)?;
    let mut rules = Vec::new();

    if let Some(definition) = project_rate_limit_definition(config) {
        push_rule(&mut rules, &keys, RateLimitScope::Project, &definition);
    }
    if let Some(definition) = config.api_key_rate_limit.as_ref() {
        push_rule(&mut rules, &keys, RateLimitScope::AccessKey, definition);
    }
    if let Some(definition) =
        find_rate_limit_definition(config.model_rate_limits.as_ref(), [dimensions.model, None])
    {
        push_rule(&mut rules, &keys, RateLimitScope::Model, definition);
    }
    if let Some(definition) = find_rate_limit_definition(
        config.endpoint_rate_limits.as_ref(),
        [Some(dimensions.endpoint), dimensions.endpoint_policy_alias],
    ) {
        push_rule(&mut rules, &keys, RateLimitScope::Endpoint, definition);
    }

    Ok(rules)
}

pub fn build_provider_attempt_rate_limit_rule(
    config: &GatewayRoutePolicyConfig,
    dimensions: &RateLimitDimensions<'_>,
) -> Result<Option<RateLimitRule>, GatewayError> {
    if config.rate_limit_enforcement_version.as_deref() != Some("v1") {
        return Ok(None);
    }
    let Some(definition) = config.provider_attempt_rate_limit.as_ref() else {
        return Ok(None);
    };
    let keys = build_rate_limit_scope_keys(dimensions)?;
    Ok(rule_for_scope(
        &keys,
        RateLimitScope::ProviderAttempt,
        definition,
    ))
}

pub fn build_rate_limit_scope_keys(
    dimensions: &RateLimitDimensions<'_>,
) -> Result<Vec<RateLimitScopeKey>, GatewayError> {
    let tenant_id = required_component(dimensions.tenant_id, "tenant_id", false)?;
    let project_id = required_component(dimensions.project_id, "project_id", false)?;
    let endpoint = required_component(dimensions.endpoint, "endpoint", true)?;
    let namespace_hash = rate_limit_namespace_hash(&tenant_id, &project_id);
    let mut keys = vec![scope_key(
        RateLimitScope::Project,
        &namespace_hash,
        &[
            ("tenant", tenant_id.as_str()),
            ("project", project_id.as_str()),
        ],
    )];

    if let Some(access_key_id) =
        optional_component(dimensions.access_key_id, "access_key_id", false)?
    {
        keys.push(scope_key(
            RateLimitScope::AccessKey,
            &namespace_hash,
            &[
                ("tenant", tenant_id.as_str()),
                ("project", project_id.as_str()),
                ("access_key", access_key_id.as_str()),
            ],
        ));
    }

    if let Some(model) = optional_component(dimensions.model, "model", true)? {
        keys.push(scope_key(
            RateLimitScope::Model,
            &namespace_hash,
            &[
                ("tenant", tenant_id.as_str()),
                ("project", project_id.as_str()),
                ("model", model.as_str()),
            ],
        ));
    }

    keys.push(scope_key(
        RateLimitScope::Endpoint,
        &namespace_hash,
        &[
            ("tenant", tenant_id.as_str()),
            ("project", project_id.as_str()),
            ("endpoint", endpoint.as_str()),
        ],
    ));

    if let Some(provider_account_id) =
        optional_component(dimensions.provider_account_id, "provider_account_id", false)?
    {
        keys.push(scope_key(
            RateLimitScope::ProviderAttempt,
            &namespace_hash,
            &[
                ("tenant", tenant_id.as_str()),
                ("project", project_id.as_str()),
                ("provider", provider_account_id.as_str()),
            ],
        ));
    }

    Ok(keys)
}

pub fn endpoint_rate_limit_identity(endpoint_kind: EndpointKind) -> (&'static str, &'static str) {
    match endpoint_kind {
        EndpointKind::ChatCompletions => ("chat_completions", "post /v1/chat/completions"),
        EndpointKind::Completions => ("completions", "post /v1/completions"),
        EndpointKind::Embeddings => ("embeddings", "post /v1/embeddings"),
        EndpointKind::ImagesGenerations => ("images_generations", "post /v1/images/generations"),
        EndpointKind::ImagesEdits => ("images_edits", "post /v1/images/edits"),
        EndpointKind::MusicGenerations => ("music_generations", "post /v1/music/generations"),
        EndpointKind::VideosGenerations => ("videos_generations", "post /v1/videos/generations"),
        EndpointKind::AudioTranscriptions => {
            ("audio_transcriptions", "post /v1/audio/transcriptions")
        }
        EndpointKind::AudioSpeech => ("audio_speech", "post /v1/audio/speech"),
        EndpointKind::Messages => ("messages", "post /v1/messages"),
        EndpointKind::Responses => ("responses", "post /v1/responses"),
        EndpointKind::Search => ("search", "post /v1/search"),
        EndpointKind::Fetch => ("fetch", "post /v1/fetch"),
        EndpointKind::ResearchCreate => ("research_create", "post /v1/research"),
        EndpointKind::ResearchList => ("research_list", "get /v1/research"),
        EndpointKind::ResearchGet => ("research_get", "get /v1/research/:id"),
        EndpointKind::CreditsBalance => ("credits_balance", "get /v1/credits/balance"),
    }
}

fn project_rate_limit_definition(
    config: &GatewayRoutePolicyConfig,
) -> Option<GatewayRateLimitDefinition> {
    match (
        config.rate_limit_window_seconds,
        config.rate_limit_max_requests,
    ) {
        (Some(window_seconds), Some(max_requests)) if window_seconds > 0 && max_requests > 0 => {
            Some(GatewayRateLimitDefinition {
                window_seconds,
                max_requests,
            })
        }
        _ => None,
    }
}

fn find_rate_limit_definition<'a, const N: usize>(
    definitions: Option<&'a HashMap<String, GatewayRateLimitDefinition>>,
    candidates: [Option<&str>; N],
) -> Option<&'a GatewayRateLimitDefinition> {
    let definitions = definitions?;
    for candidate in candidates.into_iter().flatten() {
        let candidate = candidate.trim().to_ascii_lowercase();
        if candidate.is_empty() {
            continue;
        }
        if let Some(definition) = definitions.get(&candidate) {
            return Some(definition);
        }
        if let Some((_, definition)) = definitions
            .iter()
            .find(|(key, _)| key.trim().eq_ignore_ascii_case(&candidate))
        {
            return Some(definition);
        }
    }
    None
}

fn push_rule(
    rules: &mut Vec<RateLimitRule>,
    keys: &[RateLimitScopeKey],
    scope: RateLimitScope,
    definition: &GatewayRateLimitDefinition,
) {
    if let Some(rule) = rule_for_scope(keys, scope, definition) {
        rules.push(rule);
    }
}

fn rule_for_scope(
    keys: &[RateLimitScopeKey],
    scope: RateLimitScope,
    definition: &GatewayRateLimitDefinition,
) -> Option<RateLimitRule> {
    if definition.window_seconds <= 0 || definition.max_requests <= 0 {
        return None;
    }
    let key = keys.iter().find(|entry| entry.scope == scope)?;
    Some(RateLimitRule {
        scope,
        key: key.key.clone(),
        window_seconds: definition.window_seconds as u64,
        max_requests: definition.max_requests as u64,
    })
}

fn rate_limit_namespace_hash(tenant_id: &str, project_id: &str) -> String {
    let canonical = format!(
        "tenant:{}:{tenant_id};project:{}:{project_id};",
        tenant_id.len(),
        project_id.len()
    );
    format!("{:x}", Sha256::digest(canonical.as_bytes()))
}

fn scope_key(
    scope: RateLimitScope,
    namespace_hash: &str,
    components: &[(&str, &str)],
) -> RateLimitScopeKey {
    let mut canonical = String::new();
    for (label, value) in components {
        let _ = write!(&mut canonical, "{label}:{}:{value};", value.len());
    }
    let digest = Sha256::digest(canonical.as_bytes());
    RateLimitScopeKey {
        scope,
        key: format!(
            "gateway:rate-limit:v1:{{{namespace_hash}}}:{}:{digest:x}",
            scope.as_str()
        ),
    }
}

fn required_component(value: &str, label: &str, lowercase: bool) -> Result<String, GatewayError> {
    optional_component(Some(value), label, lowercase)?.ok_or_else(|| {
        GatewayError::bad_request(format!("rate-limit {label} must not be empty"))
            .with_code("rate_limit_scope_invalid")
    })
}

fn optional_component(
    value: Option<&str>,
    label: &str,
    lowercase: bool,
) -> Result<Option<String>, GatewayError> {
    let Some(value) = value else {
        return Ok(None);
    };
    let value = value.trim();
    if value.is_empty() {
        return Err(
            GatewayError::bad_request(format!("rate-limit {label} must not be empty"))
                .with_code("rate_limit_scope_invalid"),
        );
    }
    Ok(Some(if lowercase {
        value.to_ascii_lowercase()
    } else {
        value.to_string()
    }))
}
