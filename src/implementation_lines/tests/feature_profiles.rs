use super::*;

macro_rules! search_protocol_profile_compile_tests {
    (
        feature = $feature:literal,
        profiles = [$($profile:literal),+ $(,)?],
        line = $line:literal,
        enabled = $enabled_test:ident,
        disabled = $disabled_test:ident
    ) => {
        #[cfg(feature = $feature)]
        #[test]
        fn $enabled_test() {
            for profile in [$($profile),+] {
                assert!(is_protocol_profile_compiled_in(profile), "{profile}");
                ensure_protocol_profile_compiled(profile)
                    .expect("search line feature enabled");
            }
        }

        #[cfg(not(feature = $feature))]
        #[test]
        fn $disabled_test() {
            for profile in [$($profile),+] {
                assert!(!is_protocol_profile_compiled_in(profile), "{profile}");
                let error = ensure_protocol_profile_compiled(profile)
                    .expect_err("search line feature disabled");
                assert_eq!(
                    error.code.as_deref(),
                    Some("gateway_provider_line_compiled_out")
                );
                assert!(error.message.contains($line));
                assert!(error.message.contains($feature));
            }
        }
    };
}

search_protocol_profile_compile_tests!(
    feature = "line-perplexity-search-official-vendor-api",
    profiles = ["perplexity_search", "perplexity-search"],
    line = "perplexity_search",
    enabled = perplexity_search_protocol_profiles_compile_when_feature_enabled,
    disabled = perplexity_search_protocol_profiles_fail_compile_check_when_feature_disabled
);

search_protocol_profile_compile_tests!(
    feature = "line-tavily-search-official-vendor-api",
    profiles = ["tavily", "tavily-search"],
    line = "tavily",
    enabled = tavily_search_protocol_profiles_compile_when_feature_enabled,
    disabled = tavily_search_protocol_profiles_fail_compile_check_when_feature_disabled
);

search_protocol_profile_compile_tests!(
    feature = "line-exa-search-official-vendor-api",
    profiles = ["exa", "exa-search"],
    line = "exa",
    enabled = exa_search_protocol_profiles_compile_when_feature_enabled,
    disabled = exa_search_protocol_profiles_fail_compile_check_when_feature_disabled
);

search_protocol_profile_compile_tests!(
    feature = "line-jina-search-official-vendor-api",
    profiles = ["jina_search", "jina-search"],
    line = "jina_search",
    enabled = jina_search_protocol_profiles_compile_when_feature_enabled,
    disabled = jina_search_protocol_profiles_fail_compile_check_when_feature_disabled
);

search_protocol_profile_compile_tests!(
    feature = "line-jina-reader-official-vendor-api",
    profiles = ["jina_reader", "jina-reader"],
    line = "jina_reader",
    enabled = jina_reader_protocol_profiles_compile_when_feature_enabled,
    disabled = jina_reader_protocol_profiles_fail_compile_check_when_feature_disabled
);

search_protocol_profile_compile_tests!(
    feature = "line-linkup-search-official-vendor-api",
    profiles = ["linkup", "linkup-search", "linkup_search"],
    line = "linkup",
    enabled = linkup_search_protocol_profiles_compile_when_feature_enabled,
    disabled = linkup_search_protocol_profiles_fail_compile_check_when_feature_disabled
);

search_protocol_profile_compile_tests!(
    feature = "line-you-search-official-vendor-api",
    profiles = ["you_search", "you-search", "you"],
    line = "you_search",
    enabled = you_search_protocol_profiles_compile_when_feature_enabled,
    disabled = you_search_protocol_profiles_fail_compile_check_when_feature_disabled
);

search_protocol_profile_compile_tests!(
    feature = "line-websearchapi-search-official-vendor-api",
    profiles = ["websearchapi", "websearchapi-search", "websearchapi_search"],
    line = "websearchapi",
    enabled = websearchapi_search_protocol_profiles_compile_when_feature_enabled,
    disabled = websearchapi_search_protocol_profiles_fail_compile_check_when_feature_disabled
);

search_protocol_profile_compile_tests!(
    feature = "line-xai-openai-official-vendor-api",
    profiles = ["xai", "xai-openai", "xai_openai"],
    line = "xai",
    enabled = xai_openai_protocol_profiles_compile_when_feature_enabled,
    disabled = xai_openai_protocol_profiles_fail_compile_check_when_feature_disabled
);

search_protocol_profile_compile_tests!(
    feature = "line-perplexity-chat-official-vendor-api",
    profiles = ["perplexity_chat", "perplexity-chat"],
    line = "perplexity_chat",
    enabled = perplexity_chat_protocol_profiles_compile_when_feature_enabled,
    disabled = perplexity_chat_protocol_profiles_fail_compile_check_when_feature_disabled
);

search_protocol_profile_compile_tests!(
    feature = "line-xfyun-openai-official-vendor-api",
    profiles = ["xfyun_openai", "xfyun-openai", "xfyun"],
    line = "xfyun_openai",
    enabled = xfyun_openai_protocol_profiles_compile_when_feature_enabled,
    disabled = xfyun_openai_protocol_profiles_fail_compile_check_when_feature_disabled
);

search_protocol_profile_compile_tests!(
    feature = "line-xfyun-native-websocket-official-vendor-api",
    profiles = [
        "xfyun_native_websocket",
        "xfyun-native-websocket",
        "xfyun_websocket"
    ],
    line = "xfyun_native_websocket",
    enabled = xfyun_native_websocket_protocol_profiles_compile_when_feature_enabled,
    disabled = xfyun_native_websocket_protocol_profiles_fail_compile_check_when_feature_disabled
);

search_protocol_profile_compile_tests!(
    feature = "line-freebuff-web-reverse-api",
    profiles = ["freebuff", "freebuff-compatible", "codebuff"],
    line = "freebuff",
    enabled = freebuff_web_reverse_protocol_profiles_compile_when_feature_enabled,
    disabled = freebuff_web_reverse_protocol_profiles_fail_compile_check_when_feature_disabled
);

search_protocol_profile_compile_tests!(
    feature = "line-producer-web-reverse-api",
    profiles = [
        "producer",
        "producer-images",
        "producer-music",
        "producer-videos"
    ],
    line = "producer",
    enabled = producer_web_reverse_protocol_profiles_compile_when_feature_enabled,
    disabled = producer_web_reverse_protocol_profiles_fail_compile_check_when_feature_disabled
);
