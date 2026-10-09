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
        discovered_protocols: Vec::new(),
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
mod tests;
