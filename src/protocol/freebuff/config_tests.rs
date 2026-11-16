//! Runtime configuration contracts, independent of network/session ownership.
use super::tests::make_payload;
use super::*;

#[test]
fn config_validation_preserves_auth_base_and_agent_error_order() {
    let mut payload = make_payload();
    payload.api_key = " ".into();
    payload.base_url = " ".into();
    payload.extra_body = None;
    let error = FreeBuffRuntimeConfig::from_payload(&payload, "missing").unwrap_err();
    assert_eq!(error.code.as_deref(), Some("freebuff_missing_auth_token"));
    payload.api_key = "synthetic-test-token".into();
    let error = FreeBuffRuntimeConfig::from_payload(&payload, "missing").unwrap_err();
    assert_eq!(error.code.as_deref(), Some("freebuff_missing_base_url"));
    payload.base_url = "https://example.test".into();
    let error = FreeBuffRuntimeConfig::from_payload(&payload, "missing").unwrap_err();
    assert_eq!(error.code.as_deref(), Some("freebuff_missing_agent_id"));
}

#[test]
fn config_defaults_aliases_path_overrides_and_lower_bounds_are_preserved() {
    let mut payload = make_payload();
    payload.api_key = " synthetic-test-token ".into();
    payload.extra_body = Some(HashMap::from([
        ("FREEBUFF_AGENT_ID".into(), json!(" fallback ")),
        ("freebuff-agent-runs-path".into(), json!(" /runs ")),
        ("freebuffStartRunPath".into(), json!("/start")),
        ("freebuffSessionPath".into(), json!("/session")),
        ("freebuffRunRotationSecs".into(), json!(-1)),
        ("freebuffSessionPollIntervalMs".into(), json!(" 1 ")),
        ("freebuffSessionPollTimeoutMs".into(), json!(0)),
        ("freebuffCostMode".into(), json!(" FREE ")),
    ]));
    let config = FreeBuffRuntimeConfig::from_payload(&payload, "model").unwrap();
    assert_eq!(config.base_url, "https://www.codebuff.com");
    assert_eq!(config.auth_token, "synthetic-test-token");
    assert_eq!(config.agent_id, "fallback");
    assert_eq!(config.start_run_path, "/start");
    assert_eq!(config.finish_run_path, "/runs");
    assert_eq!(config.session_path, "/session");
    assert_eq!(
        config.chat_completions_path,
        FREEBUFF_DEFAULT_CHAT_COMPLETIONS_PATH
    );
    assert_eq!(config.rotation_interval, Duration::from_secs(60));
    assert_eq!(config.session_poll_interval, Duration::from_millis(250));
    assert_eq!(config.session_poll_timeout, Duration::from_millis(250));
    assert_eq!(config.user_agent, FREEBUFF_DEFAULT_USER_AGENT);
    assert!(config.requires_free_session());
}

#[test]
fn model_selection_keeps_direct_trim_lowercase_and_fallback_precedence() {
    let mut payload = make_payload();
    payload.extra_body = Some(HashMap::from([
        ("freebuffAgentId".into(), json!("fallback")),
        (
            "freebuffModelAgentMap".into(),
            json!({
                " Model ": "exact", "Model":"trimmed", "model":{"agents":["",{"agent_id":"nested"}]}
            }),
        ),
    ]));
    for (model, expected) in [
        (" Model ", "exact"),
        ("Model", "trimmed"),
        ("MODEL", "nested"),
        ("other", "fallback"),
    ] {
        assert_eq!(
            FreeBuffRuntimeConfig::from_payload(&payload, model)
                .unwrap()
                .agent_id,
            expected
        );
    }
    let choices = json!(["a", "b", null, ""]);
    for _ in 0..20 {
        assert!(matches!(
            extract_agent_id(&choices).as_deref(),
            Some("a" | "b")
        ));
    }
}

#[test]
fn credential_bucket_identity_does_not_include_raw_auth_token() {
    let mut payload = make_payload();
    payload.credential_id = Some(" stable-id ".into());
    let config = FreeBuffRuntimeConfig::from_payload(&payload, "z-ai/glm-5.1").unwrap();
    assert_eq!(config.session_bucket_key, credential_subject_id(&payload));
    assert_eq!(
        config.bucket_key,
        format!("{}::base2-free", config.session_bucket_key)
    );
    assert!(!config.session_bucket_key.contains("stable-id"));
    payload.credential_id = None;
    let first = credential_subject_id(&payload);
    assert!(first.starts_with("subject:"));
    assert_eq!(first.len(), "subject:".len() + 64);
    assert!(!first.contains(&payload.api_key));
    payload.api_key.push('x');
    assert_ne!(credential_subject_id(&payload), first);
}

#[test]
fn config_debug_does_not_expose_credential_or_connection_details() {
    let mut payload = make_payload();
    payload.api_key = "SYNTHETIC_DEBUG_SECRET".into();
    payload.base_url = "https://user:synthetic-pass@example.test/path?token=synthetic-query".into();
    let config = FreeBuffRuntimeConfig::from_payload(&payload, "z-ai/glm-5.1").unwrap();
    let debug = format!("{config:?}");
    assert!(!debug.contains("SYNTHETIC_DEBUG_SECRET"));
    assert!(!debug.contains("synthetic-pass"));
    assert!(!debug.contains("synthetic-query"));
    assert!(debug.contains("[REDACTED_SECRET]"));
}

#[test]
fn freebuff_business_target_cannot_fall_back_to_legacy_agent_or_unsigned_plan() {
    let mut payload = make_payload();
    payload.base_url = "https://freebuff.com".into();
    payload.extra_body.as_mut().unwrap().insert(
        "freebuffBusiness".into(),
        json!({"origin":"https://codebuff.com","client":"cli"}),
    );
    let error = FreeBuffRuntimeConfig::from_payload(&payload, "z-ai/glm-5.1").unwrap_err();
    assert_eq!(
        error.code.as_deref(),
        Some("freebuff_business_admission_required")
    );
    let error =
        build_request_plan(&payload, &super::tests::make_request(), "m-stable", false).unwrap_err();
    assert_eq!(
        error.code.as_deref(),
        Some("freebuff_business_admission_required")
    );
}

#[test]
fn freebuff_business_target_is_not_a_caller_wire_field_or_a_legacy_mode_override() {
    let payload = make_payload();
    let mut request = super::tests::make_request();
    for key in ["freebuffBusiness", "FREEBUFF_BUSINESS", "freebuff-business"] {
        assert!(is_reserved_payload_extra_key(key));
        request.extra.insert(
            key.into(),
            json!({"origin":"https://evil.invalid","client":"cli"}),
        );
    }
    let body = build_request_plan(&payload, &request, "z-ai/glm-5.1", false)
        .unwrap()
        .body
        .unwrap();
    assert!(!body.to_string().contains("evil.invalid"));
    assert!(!body.to_string().contains("freebuffBusiness"));
    assert!(FreeBuffRuntimeConfig::from_payload(&payload, "z-ai/glm-5.1").is_ok());
}
