//! Redis conversion must resolve flow before deriving routing metadata.
use super::*;

#[test]
fn call_flow_redis_raw_and_preset_keep_effective_format_and_profile() {
    for preset in [None, Some("openai")] {
        let mut raw = json!({"call_flow":{"call_adapter":"anthropic",
            "fingerprint_adapter":"claude_cli","special_adapter":"none"}});
        if let Some(preset) = preset {
            raw["preset"] = json!(preset);
        }
        let credential = make_credential(
            "flow",
            CredentialKind::UserOwned,
            "openai",
            Some("fixture"),
            Some("https://fixture.test"),
            Some(raw),
        );
        let candidate = credential_to_candidate(&credential, "fixture").unwrap();
        assert!(candidate.payload.call_flow.is_some(), "preset={preset:?}");
        assert_eq!(candidate.adapter, "anthropic_compatible");
        assert_eq!(candidate.payload.adapter, candidate.adapter);
        assert_eq!(
            candidate.payload.request_client_profile,
            Some(crate::protocol::request_profile::RequestClientProfile::ClaudeCli)
        );
        assert_eq!(candidate.protocol_family, "anthropic_messages");
        assert_eq!(
            candidate.supported_protocol_families,
            ["anthropic_messages"]
        );
    }
}

#[test]
fn call_flow_redis_rejects_invalid_or_model_blocked_policy_without_legacy_fallback() {
    for raw in [
        json!({"call_flow":{"call_adapter":"unknown"}}),
        json!({"callFlow":{"special_adapter":"chatgpt_web"}}),
        json!({"call_flow":{"call_adapter":"anthropic"},
            "blocked_protocol_families_by_model":{"fixture":["anthropic_messages"]}}),
    ] {
        let credential = make_credential(
            "flow",
            CredentialKind::UserOwned,
            "openai",
            Some("fixture"),
            Some("https://fixture.test"),
            Some(raw),
        );
        assert!(credential_to_candidate(&credential, "fixture").is_none());
    }
}

#[test]
fn call_flow_redis_does_not_trust_unvalidated_discovery_targets() {
    let raw = json!({"callFlow":{"call_adapter":"auto","fingerprint_adapter":"claude_cli"},
        "discovered_protocols":[{"protocol":"responses","api_base":"https://untrusted.invalid",
            "verified_models":["fixture"]}]});
    let credential = make_credential(
        "flow",
        CredentialKind::UserOwned,
        "openai",
        Some("fixture"),
        Some("https://fixture.test"),
        Some(raw),
    );
    let candidate = credential_to_candidate(&credential, "fixture").unwrap();
    assert!(candidate.payload.call_flow.is_some());
    assert!(candidate.payload.discovered_protocols.is_empty());
    assert_eq!(candidate.payload.base_url, "https://fixture.test");
}
