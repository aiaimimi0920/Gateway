// ---------------------------------------------------------------------------
// routing/credential_routing.rs — Convert Redis credentials to route candidates
//
// Bridges the credential cache (redis/credential_cache.rs) with the routing
// layer (routing/candidate.rs) by converting a CredentialEntry into a
// RouteCandidate, optionally compiling through the preset system.
// ---------------------------------------------------------------------------

use crate::preset::{compile_provider_account, get_builtin_preset, AccountOverrides};
use crate::protocol::registry::{
    canonicalize_protocol_profile_key, default_protocol_profile_for_preset, infer_protocol_family,
    infer_protocol_profile,
};
use crate::redis::credential_cache::{CredentialEntry, CredentialKind};
use crate::routing::candidate::{
    canonicalize_adapter_name, ProviderAccountPayload, RouteCandidate,
    SEARCH_API_COMPATIBLE_ADAPTER,
};
use crate::routing::protocol_resolution::resolve_supported_wire_protocol_families_for_model;

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

/// Convert a [`CredentialEntry`] into a [`RouteCandidate`].
///
/// Uses the preset system if the credential's `account_payload` specifies a
/// preset name. Falls back to building a raw payload from the credential
/// fields.
///
/// Returns `None` if the credential lacks `api_key` or `api_base_url`.
pub fn credential_to_candidate(cred: &CredentialEntry, model: &str) -> Option<RouteCandidate> {
    let base_url = cred.api_base_url.as_deref()?;
    let adapter_hint = adapter_for_provider(&cred.provider);
    let api_key = match cred.api_key.as_deref() {
        Some(value) => value,
        None if adapter_hint == "gemini_canvas_compatible" => "",
        None => return None,
    };

    // Build payload - if preset specified, compile with preset
    let payload = if let Some(preset_name) = cred.preset_name() {
        if let Some(preset) = get_builtin_preset(preset_name) {
            let overrides = AccountOverrides {
                base_url: base_url.to_string(),
                api_key: api_key.to_string(),
                headers: cred.headers.clone().unwrap_or_default(),
                extra_body: cred.extra_body_fields(),
                default_model: None,
                auth_mode: None,
                anthropic_version: None,
                beta_headers: None,
                auth_header_name: None,
                auth_token: None,
                search_path: None,
                fetch_path: None,
                research_path: None,
                balance_path: None,
                search_query_field: None,
                fetch_urls_field: None,
                session_auth: cred.session_auth(),
                keepalive: cred.keepalive_config(),
                expires_at: cred.expires_at.clone(),
                runtime_state_object_key: cred.runtime_state_object_key().map(str::to_string),
                account_name: cred.account_name().map(str::to_string),
                execution_mode: cred.execution_mode(),
                endpoint_execution_modes: cred.endpoint_execution_modes(),
            };
            let mut payload = compile_provider_account(&preset, &overrides);
            payload.credential_id = Some(cred.id.clone());
            if let Some(session) = payload.session_auth.as_mut() {
                if session.expires_at.is_none() {
                    session.expires_at = cred.expires_at.clone();
                }
            }
            payload
        } else {
            // Unknown preset, build raw payload
            build_raw_payload(cred, base_url, api_key)
        }
    } else {
        build_raw_payload(cred, base_url, api_key)
    };

    let adapter = canonicalize_adapter_name(&payload.adapter);
    let protocol_profile = cred
        .preset_name()
        .map(default_protocol_profile_for_preset)
        .map(canonicalize_protocol_profile_key)
        .unwrap_or_else(|| {
            infer_protocol_profile(
                &adapter,
                payload_protocol_profile(cred.account_payload.as_ref())
                    .or(cred.preset_name())
                    .or(Some(&cred.provider)),
                Some(base_url),
            )
        });
    let protocol_family = infer_protocol_family(
        payload_protocol_family(cred.account_payload.as_ref()),
        &adapter,
        Some(protocol_profile.as_str())
            .or(cred.preset_name())
            .or(Some(&cred.provider)),
        Some(base_url),
    );
    let resolved_execution_mode =
        payload.resolve_execution_mode(crate::protocol::canonical::EndpointKind::ChatCompletions);
    let supported_protocol_families = resolve_supported_wire_protocol_families_for_model(
        cred.account_payload.as_ref(),
        Some(model),
        Some(model),
        &adapter,
        &protocol_family,
    );

    Some(RouteCandidate {
        provider_account_id: format!("cred:{}", cred.id),
        provider_credential_id: None,
        label: format!("credential-{}", cred.id),
        payload,
        protocol_family,
        protocol_profile,
        supported_protocol_families,
        adapter,
        model_alias: Some(model.to_string()),
        upstream_model: Some(model.to_string()),
        resolved_execution_mode,
        priority: match cred.kind {
            CredentialKind::UserOwned => 100,
            CredentialKind::AccountCredential => 80,
            CredentialKind::PlatformUnlimited => 60,
            CredentialKind::PlatformLimited => 40,
        },
        weight: 1,
        failure_count: 0,
        cooldown_until: None,
        routing_score: None,
        routing_health_weight: None,
        routing_capacity_weight: None,
        routing_degraded: None,
        routing_breaker_open: None,
        routing_degradation_reasons: Vec::new(),
    })
}

// ---------------------------------------------------------------------------
// Internal helpers
// ---------------------------------------------------------------------------

fn build_raw_payload(
    cred: &CredentialEntry,
    base_url: &str,
    api_key: &str,
) -> ProviderAccountPayload {
    let eb = cred.extra_body_fields();
    ProviderAccountPayload {
        adapter: adapter_for_provider(&cred.provider),
        base_url: base_url.to_string(),
        api_key: api_key.to_string(),
        credential_id: Some(cred.id.clone()),
        expires_at: cred.expires_at.clone(),
        runtime_state_object_key: cred.runtime_state_object_key().map(str::to_string),
        account_name: cred.account_name().map(str::to_string),
        execution_mode: cred.execution_mode(),
        endpoint_execution_modes: cred.endpoint_execution_modes(),
        default_model: None,
        headers: cred.headers.clone().unwrap_or_default(),
        auth_mode: None,
        anthropic_version: None,
        beta_headers: None,
        auth_header_name: None,
        auth_token: None,
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
        extra_body: if eb.is_empty() { None } else { Some(eb) },
        session_auth: cred.session_auth().map(|mut session| {
            if session.expires_at.is_none() {
                session.expires_at = cred.expires_at.clone();
            }
            session
        }),
        keepalive: cred.keepalive_config(),
    }
}

fn adapter_for_provider(provider: &str) -> String {
    match provider {
        "anthropic" => "anthropic_compatible".to_string(),
        "grok" => "grok_compatible".to_string(),
        "freebuff" | "freebuff.com" | "codebuff" | "codebuff.com" => {
            "freebuff_compatible".to_string()
        }
        "gemini-business" | "gemini_business" | "google-business" | "google_business" => {
            "gemini_business_compatible".to_string()
        }
        "chataibot" | "chataibot.pro" | "chataibot_image" | "chataibot-image" => {
            "chataibot_compatible".to_string()
        }
        "lumalabs" | "lumalabs.ai" | "luma" | "luma-ai" | "uni-1" | "ray-2" | "ray2"
        | "music-v1" | "luma-video" | "luma-audio" => "lumalabs_compatible".to_string(),
        "suno" | "suno.ai" | "suno.com" => "suno_compatible".to_string(),
        "gemini-canvas" | "gemini_canvas" | "gemini-canvas-browser" | "gemini_canvas_browser" => {
            "gemini_canvas_compatible".to_string()
        }
        "gemini-canvas-browser-relay"
        | "gemini_canvas_browser_relay"
        | "gemini-canvas-web-reverse"
        | "gemini_canvas_web_reverse" => "gemini_canvas_web_reverse_compatible".to_string(),
        "linkup" | "perplexity-search" | "tavily" | "you" | "exa" | "jina" | "jina.ai"
        | "jina-search" | "jina-reader" | "websearchapi" | "websearchapi.ai" => {
            SEARCH_API_COMPATIBLE_ADAPTER.to_string()
        }
        _ => "openai_compatible".to_string(),
    }
}

fn payload_protocol_family(payload: Option<&serde_json::Value>) -> Option<&str> {
    payload
        .and_then(|value| {
            value
                .get("protocolFamily")
                .or_else(|| value.get("protocol_family"))
        })
        .and_then(|value| value.as_str())
}

fn payload_protocol_profile(payload: Option<&serde_json::Value>) -> Option<&str> {
    payload
        .and_then(|value| {
            value
                .get("protocolProfile")
                .or_else(|| value.get("protocol_profile"))
        })
        .and_then(|value| value.as_str())
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::protocol::registry::{
        EXA_SEARCH_FAMILY, JINA_READER_FAMILY, JINA_SEARCH_FAMILY, LINKUP_SEARCH_FAMILY,
        OPENAI_CHAT_FAMILY, PERPLEXITY_SEARCH_FAMILY, WEBSEARCHAPI_SEARCH_FAMILY,
        YOU_SEARCH_FAMILY,
    };
    use serde_json::json;
    use std::collections::HashMap;

    fn make_credential(
        id: &str,
        kind: CredentialKind,
        provider: &str,
        api_key: Option<&str>,
        base_url: Option<&str>,
        payload: Option<serde_json::Value>,
    ) -> CredentialEntry {
        CredentialEntry {
            id: id.to_string(),
            kind,
            project_id: "proj-1".to_string(),
            user_id: "user-1".to_string(),
            provider: provider.to_string(),
            api_key: api_key.map(String::from),
            api_base_url: base_url.map(String::from),
            headers: None,
            account_payload: payload,
            quota_total_tokens: None,
            quota_remaining_tokens: None,
            expires_at: None,
            created_at: "2024-01-01T00:00:00.000Z".to_string(),
            updated_at: "2024-01-01T00:00:00.000Z".to_string(),
        }
    }

    // ── credential_to_candidate without preset ──────────────────────────

    #[test]
    fn converts_openai_credential_without_preset() {
        let cred = make_credential(
            "cred-1",
            CredentialKind::UserOwned,
            "openai",
            Some("sk-test"),
            Some("https://api.openai.com"),
            None,
        );
        let candidate = credential_to_candidate(&cred, "gpt-4o").unwrap();
        assert_eq!(candidate.provider_account_id, "cred:cred-1");
        assert_eq!(candidate.label, "credential-cred-1");
        assert_eq!(candidate.adapter, "openai_compatible");
        assert_eq!(candidate.protocol_family, OPENAI_CHAT_FAMILY);
        assert_eq!(candidate.payload.api_key, "sk-test");
        assert_eq!(candidate.payload.base_url, "https://api.openai.com");
        assert_eq!(candidate.payload.credential_id.as_deref(), Some("cred-1"));
        assert_eq!(candidate.upstream_model.as_deref(), Some("gpt-4o"));
        assert_eq!(candidate.priority, 100); // UserOwned
    }

    #[test]
    fn converts_anthropic_credential_without_preset() {
        let cred = make_credential(
            "cred-2",
            CredentialKind::AccountCredential,
            "anthropic",
            Some("sk-ant-test"),
            Some("https://api.anthropic.com"),
            None,
        );
        let candidate = credential_to_candidate(&cred, "claude-sonnet-4-6").unwrap();
        assert_eq!(candidate.adapter, "anthropic_compatible");
        assert_eq!(candidate.protocol_family, "anthropic");
        assert_eq!(candidate.priority, 80); // AccountCredential
    }

    // ── credential_to_candidate with preset ─────────────────────────────

    #[test]
    fn converts_credential_with_codex_preset() {
        let cred = make_credential(
            "cred-3",
            CredentialKind::PlatformUnlimited,
            "codex",
            Some("tok_xxx"),
            Some("https://chatgpt.com/backend-api/codex"),
            Some(json!({
                "preset": "codex",
                "supported_models": ["gpt-5-codex"]
            })),
        );
        let candidate = credential_to_candidate(&cred, "gpt-5-codex").unwrap();
        assert_eq!(candidate.payload.adapter, "openai_compatible");
        // Codex preset injects User-Agent and Originator headers
        assert!(candidate.payload.headers.contains_key("User-Agent"));
        assert!(candidate.payload.headers.contains_key("Originator"));
        // Codex preset injects "store": false
        let extra = candidate.payload.extra_body.as_ref().unwrap();
        assert_eq!(extra.get("store"), Some(&json!(false)));
        assert_eq!(candidate.priority, 60); // PlatformUnlimited
    }

    #[test]
    fn converts_credential_with_unknown_preset_falls_back_to_raw() {
        let cred = make_credential(
            "cred-4",
            CredentialKind::PlatformLimited,
            "openai",
            Some("sk-test"),
            Some("https://api.example.com"),
            Some(json!({
                "preset": "nonexistent-preset-xyz"
            })),
        );
        let candidate = credential_to_candidate(&cred, "gpt-4o").unwrap();
        // Should still work, falling back to raw payload
        assert_eq!(candidate.payload.adapter, "openai_compatible");
        assert_eq!(candidate.payload.api_key, "sk-test");
        assert_eq!(candidate.priority, 40); // PlatformLimited
    }

    #[test]
    fn converts_grok_credential_to_grok_adapter() {
        let cred = make_credential(
            "cred-grok",
            CredentialKind::AccountCredential,
            "grok",
            Some("sso-token"),
            Some("https://grok.com"),
            Some(json!({
                "preset": "grok"
            })),
        );
        let candidate = credential_to_candidate(&cred, "grok-3").unwrap();
        assert_eq!(candidate.adapter, "grok_compatible");
        assert_eq!(candidate.protocol_family, "openai");
        assert_eq!(candidate.payload.adapter, "grok_compatible");
        assert_eq!(
            candidate
                .payload
                .session_auth
                .as_ref()
                .map(|cfg| cfg.primary_cookie_name()),
            Some("sso")
        );
        assert_eq!(
            candidate.payload.chat_completions_path.as_deref(),
            Some("/rest/app-chat/conversations/new")
        );
    }

    #[cfg(feature = "line-qwen-web-reverse")]
    #[test]
    fn converts_qwen_web_historical_preset_alias_to_qwen_web_candidate() {
        let cred = make_credential(
            "cred-qwen-web-alias",
            CredentialKind::AccountCredential,
            "qwen",
            Some("qwen-session"),
            Some("https://chat.qwen.ai"),
            Some(json!({
                "preset": "qwen-web"
            })),
        );
        let candidate = credential_to_candidate(&cred, "qwen-web-model").unwrap();
        assert_eq!(candidate.payload.adapter, "qwen_web_compatible");
        assert_eq!(candidate.adapter, "qwen_web_compatible");
        assert_eq!(candidate.protocol_profile, "qwen_web_chat");
        assert_eq!(candidate.protocol_family, "qwen_web_chat");
    }

    #[test]
    fn converts_freebuff_credential_to_freebuff_adapter() {
        let cred = make_credential(
            "cred-freebuff",
            CredentialKind::AccountCredential,
            "freebuff",
            Some("fb-token"),
            Some("https://codebuff.com"),
            Some(json!({
                "extra_body": {
                    "freebuffAgentId": "base2-free"
                }
            })),
        );
        let candidate = credential_to_candidate(&cred, "z-ai/glm-5.1").unwrap();
        assert_eq!(candidate.adapter, "freebuff_compatible");
        assert_eq!(candidate.protocol_family, "freebuff");
        assert_eq!(candidate.payload.adapter, "freebuff_compatible");
        assert_eq!(
            candidate
                .payload
                .extra_body
                .as_ref()
                .and_then(|extra| extra.get("freebuffAgentId"))
                .and_then(|value| value.as_str()),
            Some("base2-free")
        );
    }

    #[test]
    fn converts_linkup_credential_to_search_adapter() {
        let cred = make_credential(
            "cred-linkup",
            CredentialKind::AccountCredential,
            "linkup",
            Some("linkup-key"),
            Some("https://api.linkup.so"),
            Some(json!({
                "preset": "linkup"
            })),
        );
        let candidate = credential_to_candidate(&cred, "linkup-search").unwrap();
        assert_eq!(candidate.adapter, SEARCH_API_COMPATIBLE_ADAPTER);
        assert_eq!(candidate.protocol_family, LINKUP_SEARCH_FAMILY);
        assert_eq!(candidate.payload.search_path.as_deref(), Some("/v1/search"));
        assert_eq!(candidate.payload.fetch_path.as_deref(), Some("/v1/fetch"));
        assert_eq!(
            candidate.payload.research_path.as_deref(),
            Some("/v1/research")
        );
        assert_eq!(
            candidate.payload.balance_path.as_deref(),
            Some("/v1/credits/balance")
        );
    }

    #[test]
    fn converts_exa_credential_to_search_fetch_adapter() {
        let cred = make_credential(
            "cred-exa",
            CredentialKind::AccountCredential,
            "exa",
            Some("exa-key"),
            Some("https://api.exa.ai"),
            Some(json!({
                "preset": "exa"
            })),
        );
        let candidate = credential_to_candidate(&cred, "exa-search").unwrap();
        assert_eq!(candidate.adapter, SEARCH_API_COMPATIBLE_ADAPTER);
        assert_eq!(candidate.protocol_family, EXA_SEARCH_FAMILY);
        assert_eq!(candidate.payload.search_path.as_deref(), Some("/search"));
        assert_eq!(candidate.payload.fetch_path.as_deref(), Some("/contents"));
        assert!(candidate.payload.research_path.is_none());
        assert!(candidate.payload.balance_path.is_none());
        assert_eq!(
            candidate.payload.search_query_field.as_deref(),
            Some("query")
        );
        assert_eq!(candidate.payload.fetch_urls_field.as_deref(), Some("urls"));
    }

    #[test]
    fn converts_jina_search_credential_to_search_adapter() {
        let cred = make_credential(
            "cred-jina-search",
            CredentialKind::AccountCredential,
            "jina",
            Some("jina-key"),
            Some("https://s.jina.ai"),
            Some(json!({
                "preset": "jina-search"
            })),
        );
        let candidate = credential_to_candidate(&cred, "jina-search").unwrap();
        assert_eq!(candidate.adapter, SEARCH_API_COMPATIBLE_ADAPTER);
        assert_eq!(candidate.protocol_family, JINA_SEARCH_FAMILY);
        assert_eq!(candidate.payload.search_path.as_deref(), Some("/search"));
        assert!(candidate.payload.fetch_path.is_none());
        assert_eq!(candidate.payload.search_query_field.as_deref(), Some("q"));
        assert_eq!(
            candidate.payload.headers.get("Accept").map(String::as_str),
            Some("application/json")
        );
    }

    #[test]
    fn converts_jina_reader_credential_to_fetch_adapter() {
        let cred = make_credential(
            "cred-jina-reader",
            CredentialKind::AccountCredential,
            "jina-reader",
            Some("jina-key"),
            Some("https://r.jina.ai"),
            Some(json!({
                "preset": "jina-reader"
            })),
        );
        let candidate = credential_to_candidate(&cred, "jina-fetch").unwrap();
        assert_eq!(candidate.adapter, SEARCH_API_COMPATIBLE_ADAPTER);
        assert_eq!(candidate.protocol_family, JINA_READER_FAMILY);
        assert!(candidate.payload.search_path.is_none());
        assert_eq!(candidate.payload.fetch_path.as_deref(), Some("/"));
        assert_eq!(candidate.payload.fetch_urls_field.as_deref(), Some("url"));
        assert_eq!(
            candidate.payload.headers.get("Accept").map(String::as_str),
            Some("application/json")
        );
    }

    #[test]
    fn converts_websearchapi_credential_to_search_adapter() {
        let cred = make_credential(
            "cred-websearchapi",
            CredentialKind::AccountCredential,
            "websearchapi",
            Some("websearchapi-key"),
            Some("https://api.websearchapi.ai"),
            Some(json!({
                "preset": "websearchapi"
            })),
        );
        let candidate = credential_to_candidate(&cred, "websearchapi-search").unwrap();
        assert_eq!(candidate.adapter, SEARCH_API_COMPATIBLE_ADAPTER);
        assert_eq!(candidate.protocol_family, WEBSEARCHAPI_SEARCH_FAMILY);
        assert_eq!(candidate.payload.search_path.as_deref(), Some("/ai-search"));
        assert_eq!(
            candidate.payload.search_query_field.as_deref(),
            Some("query")
        );
        assert!(candidate.payload.fetch_path.is_none());
        assert!(candidate.payload.research_path.is_none());
        assert!(candidate.payload.balance_path.is_none());
        assert!(candidate.payload.fetch_urls_field.is_none());
    }

    #[test]
    fn converts_gemini_business_credential_to_custom_image_adapter() {
        let cred = make_credential(
            "cred-gemini-business",
            CredentialKind::AccountCredential,
            "gemini-business",
            Some("jwt-token"),
            Some("https://biz-discoveryengine.googleapis.com/v1alpha"),
            Some(json!({
                "preset": "gemini-business",
                "extra_body": {
                    "configId": "cfg-123",
                    "session": "projects/demo/sessions/123"
                }
            })),
        );
        let candidate = credential_to_candidate(&cred, "nano-banana-pro").unwrap();
        assert_eq!(candidate.adapter, "gemini_business_compatible");
        assert_eq!(candidate.protocol_family, "gemini_business_images");
        assert_eq!(
            candidate
                .payload
                .session_auth
                .as_ref()
                .and_then(|cfg| cfg.header_name()),
            Some("Authorization")
        );
        let extra = candidate.payload.extra_body.as_ref().unwrap();
        assert_eq!(extra.get("configId"), Some(&json!("cfg-123")));
        assert_eq!(
            extra.get("session"),
            Some(&json!("projects/demo/sessions/123"))
        );
    }

    #[test]
    fn converts_chataibot_credential_to_cookie_image_adapter() {
        let cred = make_credential(
            "cred-chataibot",
            CredentialKind::AccountCredential,
            "chataibot.pro",
            Some("jwt-token"),
            Some("https://chataibot.pro"),
            Some(json!({
                "preset": "chataibot"
            })),
        );
        let candidate = credential_to_candidate(&cred, "google-nano-banana").unwrap();
        assert_eq!(candidate.adapter, "chataibot_compatible");
        assert_eq!(candidate.protocol_family, "chataibot_images");
        assert_eq!(
            candidate
                .payload
                .session_auth
                .as_ref()
                .map(|cfg| cfg.primary_cookie_name()),
            Some("token")
        );
        assert_eq!(
            candidate
                .payload
                .session_auth
                .as_ref()
                .and_then(|cfg| cfg.secondary_cookie_name()),
            None
        );
    }

    #[test]
    fn converts_lumalabs_credential_to_cookie_image_adapter() {
        let cred = make_credential(
            "cred-lumalabs",
            CredentialKind::AccountCredential,
            "lumalabs.ai",
            Some("wos-session-token"),
            Some("https://app.lumalabs.ai"),
            Some(json!({
                "preset": "lumalabs",
                "extra_body": {
                    "realmId": "4675f69b-4aa5-4b25-bfc7-c480d8efd537"
                }
            })),
        );
        let candidate = credential_to_candidate(&cred, "uni-1").unwrap();
        assert_eq!(candidate.adapter, "lumalabs_compatible");
        assert_eq!(candidate.protocol_family, "lumalabs_images");
        assert_eq!(
            candidate
                .payload
                .session_auth
                .as_ref()
                .map(|cfg| cfg.primary_cookie_name()),
            Some("wos-session")
        );
        assert_eq!(
            candidate
                .payload
                .extra_body
                .as_ref()
                .and_then(|extra| extra.get("realmId")),
            Some(&json!("4675f69b-4aa5-4b25-bfc7-c480d8efd537"))
        );
    }

    #[test]
    fn lumalabs_video_model_hint_maps_to_same_adapter_family() {
        let cred = make_credential(
            "cred-lumalabs-video",
            CredentialKind::AccountCredential,
            "ray-2",
            Some("wos-session-token"),
            Some("https://app.lumalabs.ai"),
            Some(json!({
                "preset": "lumalabs",
                "extra_body": {
                    "realmId": "4675f69b-4aa5-4b25-bfc7-c480d8efd537"
                }
            })),
        );
        let candidate = credential_to_candidate(&cred, "ray-2").unwrap();
        assert_eq!(candidate.adapter, "lumalabs_compatible");
        assert_eq!(candidate.protocol_family, "lumalabs_images");
        assert_eq!(candidate.protocol_profile, "lumalabs");
    }

    #[test]
    fn converts_you_credential_to_search_adapter() {
        let cred = make_credential(
            "cred-you",
            CredentialKind::AccountCredential,
            "you",
            Some("you-key"),
            Some("https://ydc-index.io"),
            Some(json!({
                "preset": "you"
            })),
        );
        let candidate = credential_to_candidate(&cred, "you-search").unwrap();
        assert_eq!(candidate.adapter, SEARCH_API_COMPATIBLE_ADAPTER);
        assert_eq!(candidate.protocol_family, YOU_SEARCH_FAMILY);
        assert_eq!(candidate.payload.search_path.as_deref(), Some("/v1/search"));
        assert_eq!(
            candidate.payload.auth_header_name.as_deref(),
            Some("X-API-Key")
        );
    }

    #[test]
    fn raw_perplexity_search_provider_uses_search_adapter_mapping() {
        let cred = make_credential(
            "cred-pplx-search",
            CredentialKind::AccountCredential,
            "perplexity-search",
            Some("pplx-key"),
            Some("https://api.perplexity.ai"),
            None,
        );
        let candidate = credential_to_candidate(&cred, "perplexity-search").unwrap();
        assert_eq!(candidate.adapter, SEARCH_API_COMPATIBLE_ADAPTER);
        assert_eq!(candidate.protocol_family, PERPLEXITY_SEARCH_FAMILY);
    }

    // ── returns None when required fields are missing ────────────────────

    #[test]
    fn returns_none_when_api_key_missing() {
        let cred = make_credential(
            "cred-5",
            CredentialKind::UserOwned,
            "openai",
            None, // no api_key
            Some("https://api.openai.com"),
            None,
        );
        assert!(credential_to_candidate(&cred, "gpt-4o").is_none());
    }

    #[test]
    fn returns_none_when_base_url_missing() {
        let cred = make_credential(
            "cred-6",
            CredentialKind::UserOwned,
            "openai",
            Some("sk-test"),
            None, // no base_url
            None,
        );
        assert!(credential_to_candidate(&cred, "gpt-4o").is_none());
    }

    // ── priority ordering ───────────────────────────────────────────────

    #[test]
    fn priority_user_owned_highest() {
        let user = make_credential(
            "u",
            CredentialKind::UserOwned,
            "openai",
            Some("k"),
            Some("https://a.com"),
            None,
        );
        let account = make_credential(
            "a",
            CredentialKind::AccountCredential,
            "openai",
            Some("k"),
            Some("https://a.com"),
            None,
        );
        let unlimited = make_credential(
            "p",
            CredentialKind::PlatformUnlimited,
            "openai",
            Some("k"),
            Some("https://a.com"),
            None,
        );
        let limited = make_credential(
            "l",
            CredentialKind::PlatformLimited,
            "openai",
            Some("k"),
            Some("https://a.com"),
            None,
        );

        let cu = credential_to_candidate(&user, "m").unwrap();
        let ca = credential_to_candidate(&account, "m").unwrap();
        let cp = credential_to_candidate(&unlimited, "m").unwrap();
        let cl = credential_to_candidate(&limited, "m").unwrap();

        assert!(cu.priority > ca.priority);
        assert!(ca.priority > cp.priority);
        assert!(cp.priority > cl.priority);
    }

    // ── extra_body from account_payload ──────────────────────────────────

    #[test]
    fn extra_body_forwarded_from_credential() {
        let cred = make_credential(
            "cred-eb",
            CredentialKind::UserOwned,
            "openai",
            Some("sk-test"),
            Some("https://api.openai.com"),
            Some(json!({
                "extra_body": {"temperature": 0.7, "top_p": 0.9}
            })),
        );
        let candidate = credential_to_candidate(&cred, "gpt-4o").unwrap();
        let extra = candidate.payload.extra_body.as_ref().unwrap();
        assert_eq!(extra.len(), 2);
        assert!(extra.contains_key("temperature"));
        assert!(extra.contains_key("top_p"));
    }

    #[test]
    fn session_metadata_forwarded_from_credential_payload() {
        let cred = make_credential(
            "cred-session",
            CredentialKind::AccountCredential,
            "grok",
            Some("sso-token"),
            Some("https://grok.com"),
            Some(json!({
                "preset": "grok",
                "session_auth": {
                    "transport": "cookie",
                    "primary_cookie_name": "custom-sso",
                    "secondary_cookie_name": "custom-sso-rw",
                    "expires_at": "2099-01-01T00:00:00.000Z"
                },
                "keepalive": {
                    "service_url": "http://grok-keeper:8080",
                    "refresh_before_secs": 120
                }
            })),
        );
        let candidate = credential_to_candidate(&cred, "grok-3").unwrap();
        assert_eq!(
            candidate
                .payload
                .session_auth
                .as_ref()
                .map(|cfg| cfg.primary_cookie_name()),
            Some("custom-sso")
        );
        assert_eq!(
            candidate
                .payload
                .keepalive
                .as_ref()
                .map(|cfg| cfg.service_url.as_str()),
            Some("http://grok-keeper:8080")
        );
    }

    // ── custom headers forwarded ────────────────────────────────────────

    #[test]
    fn custom_headers_forwarded_from_credential() {
        let mut headers = HashMap::new();
        headers.insert("X-Custom-Header".to_string(), "value".to_string());
        let mut cred = make_credential(
            "cred-h",
            CredentialKind::UserOwned,
            "openai",
            Some("sk-test"),
            Some("https://api.openai.com"),
            None,
        );
        cred.headers = Some(headers);
        let candidate = credential_to_candidate(&cred, "gpt-4o").unwrap();
        assert_eq!(
            candidate.payload.headers.get("X-Custom-Header"),
            Some(&"value".to_string())
        );
    }

    #[test]
    fn gemini_canvas_provider_maps_to_browser_state_adapter_and_runtime_key() {
        let cred = make_credential(
            "cred-canvas",
            CredentialKind::AccountCredential,
            "gemini-canvas",
            Some(""),
            Some("https://gemini.google.com"),
            Some(json!({
                "runtime_state_object_key": "objects/gemini-canvas/auth-1.json",
                "account_name": "canvas-main"
            })),
        );
        let candidate = credential_to_candidate(&cred, "gemini-3-flash-preview").unwrap();
        assert_eq!(candidate.adapter, "gemini_canvas_compatible");
        assert_eq!(candidate.protocol_family, "gemini_canvas_images");
        assert_eq!(
            candidate.payload.runtime_state_object_key.as_deref(),
            Some("objects/gemini-canvas/auth-1.json")
        );
        assert_eq!(
            candidate.payload.account_name.as_deref(),
            Some("canvas-main")
        );
    }

    #[test]
    fn gemini_canvas_provider_allows_missing_api_key_when_runtime_state_exists() {
        let cred = make_credential(
            "cred-canvas-no-key",
            CredentialKind::AccountCredential,
            "gemini-canvas",
            None,
            Some("https://gemini.google.com"),
            Some(json!({
                "runtime_state_object_key": "objects/gemini-canvas/auth-2.json"
            })),
        );
        let candidate = credential_to_candidate(&cred, "gemini-2.5-flash-image-preview")
            .expect("gemini canvas should not require api_key");
        assert_eq!(candidate.payload.api_key, "");
        assert_eq!(
            candidate.payload.runtime_state_object_key.as_deref(),
            Some("objects/gemini-canvas/auth-2.json")
        );
    }
}
