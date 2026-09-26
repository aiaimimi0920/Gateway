//! Provider and credential overrides merged with preset payloads.

use super::*;

/// Build a `ProviderAccountPayload` from a `ProviderConfigYaml` (with preset
/// merging if applicable).  `api_key_override` lets callers supply a
/// credential-specific key that replaces the provider-level key.
pub(super) fn build_base_payload(
    cfg: &ProviderConfigYaml,
    base_url_override: Option<&str>,
    api_key_override: Option<&str>,
    auth_token_override: Option<&str>,
    header_overrides: Option<&HashMap<String, String>>,
    extra_body_overrides: Option<&HashMap<String, Value>>,
    session_auth_override: Option<&SessionAuthConfig>,
    keepalive_override: Option<&KeepaliveConfig>,
    expires_at_override: Option<&str>,
    runtime_state_object_key_override: Option<&str>,
    account_name_override: Option<&str>,
    execution_mode_override: Option<ProviderExecutionMode>,
    endpoint_execution_modes_override: Option<&HashMap<String, ProviderExecutionMode>>,
    credential_id: Option<&str>,
) -> Result<ProviderAccountPayload, anyhow::Error> {
    let api_key = api_key_override.unwrap_or(&cfg.api_key).to_string();
    let auth_token = auth_token_override
        .map(str::to_string)
        .or_else(|| cfg.auth_token.clone());

    // Merge headers: provider base, then overrides on top.
    let mut merged_headers = cfg.headers.clone();
    if let Some(overrides) = header_overrides {
        for (k, v) in overrides {
            merged_headers.insert(k.clone(), v.clone());
        }
    }

    // Merge extra_body: provider base, then overrides on top.
    let mut merged_extra_body = cfg.extra_body.clone();
    if let Some(overrides) = extra_body_overrides {
        for (k, v) in overrides {
            merged_extra_body.insert(k.clone(), v.clone());
        }
    }

    if let Some(preset_id) = &cfg.preset {
        let preset = get_builtin_preset(preset_id).ok_or_else(|| {
            anyhow::anyhow!("Unknown preset '{}' for provider '{}'", preset_id, cfg.id)
        })?;

        let overrides = AccountOverrides {
            base_url: base_url_override.unwrap_or(&cfg.base_url).to_string(),
            api_key,
            headers: merged_headers,
            extra_body: merged_extra_body,
            default_model: cfg.default_model.clone(),
            auth_mode: None,
            anthropic_version: None,
            beta_headers: None,
            auth_header_name: None,
            auth_token,
            search_path: None,
            fetch_path: None,
            research_path: None,
            balance_path: None,
            search_query_field: None,
            fetch_urls_field: None,
            session_auth: session_auth_override
                .cloned()
                .or_else(|| cfg.session_auth.clone()),
            keepalive: keepalive_override
                .cloned()
                .or_else(|| cfg.keepalive.clone()),
            expires_at: expires_at_override
                .map(str::to_string)
                .or_else(|| cfg.expires_at.clone()),
            runtime_state_object_key: runtime_state_object_key_override
                .map(str::to_string)
                .or_else(|| cfg.runtime_state_object_key.clone()),
            account_name: account_name_override
                .map(str::to_string)
                .or_else(|| cfg.account_name.clone()),
            execution_mode: execution_mode_override.or(cfg.execution_mode),
            endpoint_execution_modes: endpoint_execution_modes_override
                .cloned()
                .or_else(|| cfg.endpoint_execution_modes.clone()),
        };

        let mut compiled = compile_provider_account(&preset, &overrides);
        compiled.credential_id = credential_id.map(str::to_string);

        if let Some(adapter_override) = &cfg.adapter {
            compiled.adapter = canonicalize_adapter_name(adapter_override);
        }
        // YAML path overrides win over preset defaults.
        if cfg.responses_path.is_some() {
            compiled.responses_path = cfg.responses_path.clone();
        }
        if cfg.chat_completions_path.is_some() {
            compiled.chat_completions_path = cfg.chat_completions_path.clone();
        }
        if cfg.completions_path.is_some() {
            compiled.completions_path = cfg.completions_path.clone();
        }
        if cfg.embeddings_path.is_some() {
            compiled.embeddings_path = cfg.embeddings_path.clone();
        }
        if cfg.audio_transcriptions_path.is_some() {
            compiled.audio_transcriptions_path = cfg.audio_transcriptions_path.clone();
        }
        if cfg.audio_speech_path.is_some() {
            compiled.audio_speech_path = cfg.audio_speech_path.clone();
        }
        if cfg.messages_path.is_some() {
            compiled.messages_path = cfg.messages_path.clone();
        }
        if cfg.search_path.is_some() {
            compiled.search_path = cfg.search_path.clone();
        }
        if cfg.fetch_path.is_some() {
            compiled.fetch_path = cfg.fetch_path.clone();
        }
        if cfg.research_path.is_some() {
            compiled.research_path = cfg.research_path.clone();
        }
        if cfg.balance_path.is_some() {
            compiled.balance_path = cfg.balance_path.clone();
        }
        if cfg.search_query_field.is_some() {
            compiled.search_query_field = cfg.search_query_field.clone();
        }
        if cfg.fetch_urls_field.is_some() {
            compiled.fetch_urls_field = cfg.fetch_urls_field.clone();
        }

        Ok(compiled)
    } else {
        let adapter = canonicalize_adapter_name(
            cfg.adapter
                .clone()
                .unwrap_or_else(|| "openai_compatible".to_string())
                .as_str(),
        );

        let extra_body_opt = if merged_extra_body.is_empty() {
            None
        } else {
            Some(merged_extra_body)
        };

        Ok(ProviderAccountPayload {
            adapter,
            base_url: base_url_override.unwrap_or(&cfg.base_url).to_string(),
            api_key,
            credential_id: credential_id.map(str::to_string),
            expires_at: expires_at_override
                .map(str::to_string)
                .or_else(|| cfg.expires_at.clone()),
            runtime_state_object_key: runtime_state_object_key_override
                .map(str::to_string)
                .or_else(|| cfg.runtime_state_object_key.clone()),
            account_name: account_name_override
                .map(str::to_string)
                .or_else(|| cfg.account_name.clone()),
            execution_mode: execution_mode_override.or(cfg.execution_mode),
            endpoint_execution_modes: endpoint_execution_modes_override
                .cloned()
                .or_else(|| cfg.endpoint_execution_modes.clone()),
            default_model: cfg.default_model.clone(),
            headers: merged_headers,
            auth_mode: None,
            anthropic_version: None,
            beta_headers: None,
            auth_header_name: None,
            auth_token,
            responses_path: cfg.responses_path.clone(),
            chat_completions_path: cfg.chat_completions_path.clone(),
            completions_path: cfg.completions_path.clone(),
            embeddings_path: cfg.embeddings_path.clone(),
            audio_transcriptions_path: cfg.audio_transcriptions_path.clone(),
            audio_speech_path: cfg.audio_speech_path.clone(),
            messages_path: cfg.messages_path.clone(),
            search_path: cfg.search_path.clone(),
            fetch_path: cfg.fetch_path.clone(),
            research_path: cfg.research_path.clone(),
            balance_path: cfg.balance_path.clone(),
            search_query_field: cfg.search_query_field.clone(),
            fetch_urls_field: cfg.fetch_urls_field.clone(),
            extra_body: extra_body_opt,
            session_auth: session_auth_override
                .cloned()
                .or_else(|| cfg.session_auth.clone()),
            keepalive: keepalive_override
                .cloned()
                .or_else(|| cfg.keepalive.clone()),
        })
    }
}
