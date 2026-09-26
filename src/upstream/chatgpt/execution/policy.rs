use serde_json::Value;

use crate::protocol::chatgpt::web_reverse as surface;
use crate::routing::candidate::ProviderAccountPayload;

pub(super) fn cached_f_conversation_requirements(
    payload: &ProviderAccountPayload,
) -> Option<surface::ChatRequirements> {
    let token = extra_body_string(
        payload,
        &[
            "chatgptWebSentinelChatRequirementsToken",
            "chatgptSentinelChatRequirementsToken",
            "openaiSentinelChatRequirementsToken",
        ],
    )?;
    Some(surface::ChatRequirements {
        token,
        proof_token: extra_body_string(
            payload,
            &[
                "chatgptWebSentinelProofToken",
                "chatgptSentinelProofToken",
                "openaiSentinelProofToken",
            ],
        ),
        turnstile_token: extra_body_string(
            payload,
            &[
                "chatgptWebSentinelTurnstileToken",
                "chatgptSentinelTurnstileToken",
                "openaiSentinelTurnstileToken",
            ],
        ),
        so_token: extra_body_string(
            payload,
            &[
                "chatgptWebSentinelSoToken",
                "chatgptSentinelSoToken",
                "openaiSentinelSoToken",
            ],
        ),
    })
}

pub(super) fn cached_f_conversation_turn_trace_id(
    payload: &ProviderAccountPayload,
) -> Option<String> {
    extra_body_string(
        payload,
        &[
            "chatgptWebTurnTraceId",
            "chatgptTurnTraceId",
            "openaiTurnTraceId",
            "turnTraceId",
        ],
    )
    .or_else(|| {
        payload
            .headers
            .iter()
            .find(|(name, value)| {
                name.eq_ignore_ascii_case("x-oai-turn-trace-id") && !value.trim().is_empty()
            })
            .map(|(_, value)| value.trim().to_string())
    })
}

pub(super) fn should_post_prepare(
    payload: &ProviderAccountPayload,
    using_cached_material: bool,
) -> bool {
    extra_body_bool(
        payload,
        &[
            "chatgptWebPrepareBeforeConversation",
            "chatgptPrepareBeforeConversation",
            "prepareBeforeConversation",
        ],
    )
    .unwrap_or(!using_cached_material)
}

pub(super) fn conversation_path(
    payload: &ProviderAccountPayload,
    using_cached_material: bool,
) -> String {
    if let Some(path) = extra_body_string(
        payload,
        &[
            "conversationPath",
            "chatgptConversationPath",
            "chatgptWebConversationPath",
        ],
    ) {
        return path;
    }
    if !using_cached_material && dynamic_backend_conversation_enabled(payload) {
        return surface::CHATGPT_WEB_DEFAULT_CONVERSATION_PATH.to_string();
    }
    surface::CHATGPT_WEB_DEFAULT_F_CONVERSATION_PATH.to_string()
}

fn dynamic_backend_conversation_enabled(payload: &ProviderAccountPayload) -> bool {
    if extra_body_bool(
        payload,
        &[
            "chatgptWebDynamicBackendConversation",
            "chatgptDynamicBackendConversation",
            "dynamicBackendConversation",
            "preferDynamicConversation",
        ],
    )
    .unwrap_or(false)
    {
        return true;
    }
    match extra_body_string(
        payload,
        &[
            "chatgptWebConversationMode",
            "chatgptConversationMode",
            "conversationMode",
        ],
    )
    .map(|value| value.trim().to_ascii_lowercase())
    .as_deref()
    {
        Some(
            "dynamic_backend_conversation"
            | "backend_conversation"
            | "browserless_dynamic_backend_conversation"
            | "chatgpt2api",
        ) => true,
        _ => false,
    }
}

pub(super) fn conversation_prepare_path(
    payload: &ProviderAccountPayload,
    conversation_path: &str,
) -> Option<String> {
    if let Some(path) = extra_body_string(
        payload,
        &[
            "conversationPreparePath",
            "chatgptConversationPreparePath",
            "chatgptWebConversationPreparePath",
        ],
    ) {
        return Some(path);
    }
    (conversation_path == surface::CHATGPT_WEB_DEFAULT_F_CONVERSATION_PATH)
        .then(|| surface::CHATGPT_WEB_DEFAULT_F_CONVERSATION_PREPARE_PATH.to_string())
}

fn extra_body_bool(payload: &ProviderAccountPayload, keys: &[&str]) -> Option<bool> {
    let body = payload.extra_body.as_ref()?;
    for key in keys {
        let Some(value) = body.get(*key) else {
            continue;
        };
        if let Some(value) = value.as_bool() {
            return Some(value);
        }
        if let Some(value) = value.as_str() {
            match value.trim().to_ascii_lowercase().as_str() {
                "1" | "true" | "yes" | "on" | "enabled" => return Some(true),
                "0" | "false" | "no" | "off" | "disabled" | "never" => return Some(false),
                _ => {}
            }
        }
    }
    None
}

pub(super) fn extra_body_string(payload: &ProviderAccountPayload, keys: &[&str]) -> Option<String> {
    let extra_body = payload.extra_body.as_ref()?;
    for key in keys {
        let value = extra_body
            .get(*key)
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|entry| !entry.is_empty())
            .map(str::to_string);
        if value.is_some() {
            return value;
        }
    }
    None
}
