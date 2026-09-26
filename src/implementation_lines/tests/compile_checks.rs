use super::*;

#[test]
fn media_web_reverse_protocol_profiles_compile_when_feature_enabled() {
    let cases = [
        (
            ["suno", "suno-music", "suno-videos"].as_slice(),
            cfg!(feature = "line-suno-web-reverse-api"),
        ),
        (
            ["udio", "udio-music", "udio-videos"].as_slice(),
            cfg!(feature = "line-udio-web-reverse-api"),
        ),
        (
            ["lumalabs", "luma-labs", "lumalabs-videos"].as_slice(),
            cfg!(feature = "line-lumalabs-web-reverse-api"),
        ),
    ];
    for (profiles, enabled) in cases {
        if !enabled {
            continue;
        }
        for profile in profiles {
            assert!(is_protocol_profile_compiled_in(profile), "{profile}");
            ensure_protocol_profile_compiled(profile).expect("media platform line enabled");
        }
    }
}

#[test]
fn media_web_reverse_protocol_profiles_fail_compile_check_when_feature_disabled() {
    let cases = [
        (
            ["suno", "suno-music", "suno-videos"].as_slice(),
            "suno",
            "line-suno-web-reverse-api",
            cfg!(feature = "line-suno-web-reverse-api"),
        ),
        (
            ["udio", "udio-music", "udio-videos"].as_slice(),
            "udio",
            "line-udio-web-reverse-api",
            cfg!(feature = "line-udio-web-reverse-api"),
        ),
        (
            ["lumalabs", "luma-labs", "lumalabs-videos"].as_slice(),
            "lumalabs",
            "line-lumalabs-web-reverse-api",
            cfg!(feature = "line-lumalabs-web-reverse-api"),
        ),
    ];
    for (profiles, line, feature, enabled) in cases {
        if enabled {
            continue;
        }
        for profile in profiles {
            assert!(!is_protocol_profile_compiled_in(profile), "{profile}");
            let error = ensure_protocol_profile_compiled(profile)
                .expect_err("media platform line disabled");
            assert_eq!(
                error.code.as_deref(),
                Some("gateway_provider_line_compiled_out")
            );
            assert!(error.message.contains(line));
            assert!(error.message.contains(feature));
        }
    }
}

#[test]
fn compiled_out_error_uses_stable_code() {
    let error = compiled_out_error_for_line(RefactoredImplementationLine::AIStudioWebReverse);
    assert_eq!(
        error.code.as_deref(),
        Some("gateway_provider_line_compiled_out")
    );
    assert_eq!(error.http_status, Some(409));
}

#[cfg(not(feature = "line-qwen-official-api"))]
#[test]
fn qwen_official_payload_fails_compiled_check_when_feature_disabled() {
    let payload = make_payload(
        "openai_compatible",
        "https://dashscope.aliyuncs.com/compatible-mode/v1",
    );
    let error = ensure_payload_compiled(&payload).expect_err("qwen official line disabled");
    assert_eq!(
        error.code.as_deref(),
        Some("gateway_provider_line_compiled_out")
    );
    assert!(error.message.contains("qwen_official_api"));
    assert!(error.message.contains("line-qwen-official-api"));
}

#[cfg(feature = "line-qwen-official-api")]
#[test]
fn qwen_official_protocol_profiles_compile_when_feature_enabled() {
    for profile in [
        "qwen_dashscope_openai",
        "qwen_coding_plan_openai",
        "qwen_coding_plan_anthropic",
        "qwen_official_api",
    ] {
        assert!(is_protocol_profile_compiled_in(profile), "{profile}");
        ensure_protocol_profile_compiled(profile).expect("qwen official line enabled");
    }
}

#[cfg(not(feature = "line-qwen-official-api"))]
#[test]
fn qwen_official_protocol_profiles_fail_compile_check_when_feature_disabled() {
    for profile in [
        "qwen_dashscope_openai",
        "qwen_coding_plan_openai",
        "qwen_coding_plan_anthropic",
        "qwen_official_api",
    ] {
        assert!(!is_protocol_profile_compiled_in(profile), "{profile}");
        let error =
            ensure_protocol_profile_compiled(profile).expect_err("qwen official line disabled");
        assert_eq!(
            error.code.as_deref(),
            Some("gateway_provider_line_compiled_out")
        );
        assert!(error.message.contains("qwen_official_api"));
        assert!(error.message.contains("line-qwen-official-api"));
    }
}

#[cfg(not(feature = "line-qwen-web-reverse"))]
#[test]
fn qwen_web_payload_fails_compiled_check_when_feature_disabled() {
    let payload = make_payload("qwen_web_compatible", "https://chat.qwen.ai");
    let error = ensure_payload_compiled(&payload).expect_err("qwen web line disabled");
    assert_eq!(
        error.code.as_deref(),
        Some("gateway_provider_line_compiled_out")
    );
    assert!(error.message.contains("qwen_web_reverse"));
    assert!(error.message.contains("line-qwen-web-reverse"));
}

#[cfg(feature = "line-qwen-web-reverse")]
#[test]
fn qwen_web_protocol_profiles_compile_when_feature_enabled() {
    for profile in [
        "qwen_web_chat",
        "qwen_web",
        "qwen-web",
        "qwen-webui",
        "qwen-webui-replay",
        "qwen-webui-replay-live",
    ] {
        assert!(is_protocol_profile_compiled_in(profile), "{profile}");
        ensure_protocol_profile_compiled(profile).expect("qwen web line enabled");
    }
}

#[cfg(not(feature = "line-qwen-web-reverse"))]
#[test]
fn qwen_web_protocol_profiles_fail_compile_check_when_feature_disabled() {
    for profile in [
        "qwen_web_chat",
        "qwen_web",
        "qwen-web",
        "qwen-webui",
        "qwen-webui-replay",
        "qwen-webui-replay-live",
    ] {
        assert!(!is_protocol_profile_compiled_in(profile), "{profile}");
        let error = ensure_protocol_profile_compiled(profile).expect_err("qwen web line disabled");
        assert_eq!(
            error.code.as_deref(),
            Some("gateway_provider_line_compiled_out")
        );
        assert!(error.message.contains("qwen_web_reverse"));
        assert!(error.message.contains("line-qwen-web-reverse"));
    }
}

#[cfg(feature = "line-nvidia-openai-official-vendor-api")]
#[test]
fn nvidia_protocol_profiles_compile_when_feature_enabled() {
    for profile in ["nvidia", "nvidia-openai", "nvidia_nim"] {
        assert!(is_protocol_profile_compiled_in(profile), "{profile}");
        ensure_protocol_profile_compiled(profile).expect("nvidia line enabled");
    }
}

#[cfg(not(feature = "line-nvidia-openai-official-vendor-api"))]
#[test]
fn nvidia_payload_fails_compiled_check_when_feature_disabled() {
    let payload = make_payload("openai_compatible", "https://integrate.api.nvidia.com/v1");
    let error = ensure_payload_compiled(&payload).expect_err("nvidia line disabled");
    assert_eq!(
        error.code.as_deref(),
        Some("gateway_provider_line_compiled_out")
    );
    assert!(error.message.contains("nvidia"));
    assert!(error
        .message
        .contains("line-nvidia-openai-official-vendor-api"));
}

#[cfg(not(feature = "line-nvidia-openai-official-vendor-api"))]
#[test]
fn nvidia_protocol_profiles_fail_compile_check_when_feature_disabled() {
    for profile in ["nvidia", "nvidia-openai", "nvidia_nim"] {
        assert!(!is_protocol_profile_compiled_in(profile), "{profile}");
        let error = ensure_protocol_profile_compiled(profile).expect_err("nvidia line disabled");
        assert_eq!(
            error.code.as_deref(),
            Some("gateway_provider_line_compiled_out")
        );
        assert!(error.message.contains("nvidia"));
        assert!(error
            .message
            .contains("line-nvidia-openai-official-vendor-api"));
    }
}

#[cfg(feature = "line-grok-web-reverse-api")]
#[test]
fn grok_protocol_profiles_compile_when_feature_enabled() {
    for profile in ["grok_web", "grok", "grok-web"] {
        assert!(is_protocol_profile_compiled_in(profile), "{profile}");
        ensure_protocol_profile_compiled(profile).expect("grok line enabled");
    }
}

#[cfg(not(feature = "line-grok-web-reverse-api"))]
#[test]
fn grok_payload_fails_compiled_check_when_feature_disabled() {
    let payload = make_payload("grok_compatible", "https://grok.com");
    let error = ensure_payload_compiled(&payload).expect_err("grok line disabled");
    assert_eq!(
        error.code.as_deref(),
        Some("gateway_provider_line_compiled_out")
    );
    assert!(error.message.contains("grok_web"));
    assert!(error.message.contains("line-grok-web-reverse-api"));
}

#[cfg(not(feature = "line-grok-web-reverse-api"))]
#[test]
fn grok_protocol_profiles_fail_compile_check_when_feature_disabled() {
    for profile in ["grok_web", "grok", "grok-web"] {
        assert!(!is_protocol_profile_compiled_in(profile), "{profile}");
        let error = ensure_protocol_profile_compiled(profile).expect_err("grok line disabled");
        assert_eq!(
            error.code.as_deref(),
            Some("gateway_provider_line_compiled_out")
        );
        assert!(error.message.contains("grok_web"));
        assert!(error.message.contains("line-grok-web-reverse-api"));
    }
}
