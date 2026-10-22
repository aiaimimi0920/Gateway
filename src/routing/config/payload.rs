//! Provider and credential overrides merged with preset payloads.

#[cfg(test)]
mod native_identity_tests {
    use super::super::*;

    #[test]
    fn auth_mode_roundtrips_and_reaches_each_credential_independent_of_style() {
        for preset in [Some("anthropic"), None] {
            let document: RouteConfigYaml = serde_json::from_value(serde_json::json!({
                "providers":[{"id":"fixture", "preset":preset, "adapter":"anthropic_compatible",
                    "base_url":"https://example.test", "auth_mode":"bearer",
                    "request_client_profile":"cpa", "credentials":[{"id":"fixture","api_key":"synthetic"}]}],
                "model_routes":[]
            })).unwrap();
            assert_eq!(
                serde_json::to_value(&document).unwrap()["providers"][0]["auth_mode"],
                "bearer"
            );
            let compiled = compile_yaml(document).unwrap();
            assert_eq!(
                compiled.providers[0].payload.auth_mode.as_deref(),
                Some("bearer")
            );
            assert_eq!(
                compiled.providers[0].credential_pool[0]
                    .payload
                    .auth_mode
                    .as_deref(),
                Some("bearer")
            );
        }
    }

    #[test]
    fn native_codex_profile_distinguishes_preset_identity_from_explicit_override() {
        for explicit in [false, true] {
            let headers = if explicit {
                serde_json::json!({"User-Agent":"operator-agent", "Originator":"operator"})
            } else {
                serde_json::json!({})
            };
            let document: RouteConfigYaml = serde_json::from_value(serde_json::json!({
                "providers":[{"id":"oauth", "preset":"chatgpt-codex-oauth-official-api",
                    "base_url":"https://chatgpt.com/backend-api/codex", "credentials":[{"id":"fixture","api_key":"fixture"}],
                    "request_client_profile":"codex_cli", "headers":headers}], "model_routes":[]
            }))
            .unwrap();
            let compiled = compile_yaml(document).unwrap();
            let payload = &compiled.providers[0].credential_pool[0].payload;
            assert_eq!(payload.headers.contains_key("User-Agent"), explicit);
            assert_eq!(payload.headers.contains_key("Originator"), explicit);
            if explicit {
                assert_eq!(payload.headers["User-Agent"], "operator-agent");
            }
        }
    }
}

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
            auth_mode: cfg.auth_mode.clone(),
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
        // Preset fallback identity is not an operator override. An explicitly
        // native Codex lane can retain the caller's software identity instead.
        if cfg.request_client_profile
            == Some(crate::protocol::request_profile::RequestClientProfile::CodexCli)
        {
            compiled.headers.retain(|name, _| {
                !matches!(
                    name.to_ascii_lowercase().as_str(),
                    "user-agent" | "originator"
                ) || overrides
                    .headers
                    .keys()
                    .any(|key| key.eq_ignore_ascii_case(name))
            });
        }
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
            request_client_profile: None,
            discovered_protocols: Vec::new(),
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
            auth_mode: cfg.auth_mode.clone(),
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
