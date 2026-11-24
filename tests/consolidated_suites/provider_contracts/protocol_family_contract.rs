use neuro_gateway::protocol::registry::{
    OPENAI_AUDIO_SPEECH_FAMILY, OPENAI_CHAT_FAMILY, OPENAI_EMBEDDINGS_FAMILY,
    OPENAI_RESPONSES_FAMILY,
};
use neuro_gateway::routing::protocol_resolution::resolve_supported_wire_protocol_families_for_model;
use serde_json::{json, Value};

fn resolve(payload: &Value, alias: Option<&str>, upstream: Option<&str>) -> Vec<String> {
    resolve_supported_wire_protocol_families_for_model(
        Some(payload),
        alias,
        upstream,
        "openai_compatible",
        OPENAI_CHAT_FAMILY,
    )
}

#[test]
fn model_rules_match_alias_or_upstream_without_substring_matches() {
    let payload = json!({
        "protocol_families": ["embeddings"],
        "protocol_families_by_model": {
            " Alias-* ": ["responses"],
            "UPSTREAM-EXACT": {"families": "chat"}
        }
    });
    assert_eq!(
        resolve(&payload, Some(" ALIAS-One "), None),
        vec![OPENAI_RESPONSES_FAMILY]
    );
    assert_eq!(
        resolve(&payload, None, Some(" upstream-exact ")),
        vec![OPENAI_CHAT_FAMILY]
    );
    assert_eq!(
        resolve(
            &payload,
            Some("prefixalias-one"),
            Some("upstream-exact-suffix")
        ),
        vec![OPENAI_EMBEDDINGS_FAMILY]
    );
}

#[test]
fn global_and_model_blocks_override_allow_rules_after_canonicalization() {
    let payload = json!({
        "protocolFamilies": ["chat, responses\nembeddings", "cohere", "chat"],
        "excluded_protocol_families": " EMBEDDINGS ",
        "blockedProtocolFamiliesByModel": {"gpt-*": [" OPENAI_RESPONSES "]}
    });
    assert_eq!(
        resolve(&payload, Some("gpt-demo"), None),
        vec![OPENAI_CHAT_FAMILY]
    );
}

#[test]
fn family_parsing_preserves_first_seen_order_across_aliases_and_fields() {
    let payload = json!({
        "protocolFamilies": [
            " RESPONSES, chat\n responses ",
            ["embeddings", false, null, 17]
        ],
        "allowedProtocolFamilies": "audio_speech, chat"
    });
    assert_eq!(
        resolve(&payload, None, None),
        vec![
            OPENAI_RESPONSES_FAMILY,
            OPENAI_CHAT_FAMILY,
            OPENAI_EMBEDDINGS_FAMILY,
            OPENAI_AUDIO_SPEECH_FAMILY
        ]
    );
}

#[test]
fn unsupported_or_fully_blocked_explicit_rules_do_not_restore_surface_defaults() {
    let unsupported = json!({"protocolFamilies": ["cohere", "unknown-family"]});
    assert!(resolve(&unsupported, None, None).is_empty());
    let blocked = json!({
        "protocolFamilies": ["chat", "responses"],
        "blockedProtocolFamilies": "openai_chat, openai_responses"
    });
    assert!(resolve(&blocked, None, None).is_empty());
}

#[test]
fn empty_or_nonfamily_model_values_keep_the_global_fallback() {
    let payload = json!({
        "protocolFamily": "embeddings",
        "protocolFamiliesByModel": {"demo": [null, false, 4], "empty": []}
    });
    assert_eq!(
        resolve(&payload, Some("demo"), None),
        vec![OPENAI_EMBEDDINGS_FAMILY]
    );
    assert_eq!(
        resolve(&payload, Some("empty"), None),
        vec![OPENAI_EMBEDDINGS_FAMILY]
    );
}
