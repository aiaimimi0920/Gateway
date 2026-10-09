//! Stable public wire names, including backward-compatible persisted aliases.
use super::canonical::ProtocolFamily;

#[test]
fn roundtrip_protocol_family() {
    let cases = [
        (ProtocolFamily::OpenAi, "open_ai"),
        (ProtocolFamily::OpenAiRealtime, "open_ai_realtime"),
        (ProtocolFamily::Anthropic, "anthropic"),
        (
            ProtocolFamily::GeminiGenerateContent,
            "gemini_generate_content",
        ),
        (ProtocolFamily::GeminiLive, "gemini_live"),
        (ProtocolFamily::BedrockConverse, "bedrock_converse"),
        (ProtocolFamily::CohereChat, "cohere_chat"),
        (ProtocolFamily::DashScope, "dashscope_text"),
        (ProtocolFamily::DashScopeMultimodal, "dashscope_multimodal"),
        (ProtocolFamily::SearchApi, "search"),
    ];
    for (family, expected) in cases {
        let value = serde_json::to_value(family).unwrap();
        assert_eq!(value, expected);
        assert_eq!(
            serde_json::from_value::<ProtocolFamily>(value).unwrap(),
            family
        );
    }
}

#[test]
fn legacy_protocol_family_alias_deserializes_to_search() {
    for alias in ["linkup", "search_api"] {
        let decoded: ProtocolFamily = serde_json::from_value(serde_json::json!(alias)).unwrap();
        assert_eq!(decoded, ProtocolFamily::SearchApi);
    }
}
