use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;

/// Protocol family of the incoming request.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ProtocolFamily {
    #[serde(rename = "open_ai")]
    OpenAi,
    #[serde(rename = "open_ai_realtime")]
    OpenAiRealtime,
    #[serde(rename = "anthropic")]
    Anthropic,
    #[serde(rename = "gemini_generate_content")]
    GeminiGenerateContent,
    #[serde(rename = "gemini_live")]
    GeminiLive,
    #[serde(rename = "bedrock_converse")]
    BedrockConverse,
    #[serde(rename = "cohere_chat", alias = "cohere_chat_v2")]
    CohereChat,
    #[serde(rename = "search", alias = "search_api", alias = "linkup")]
    SearchApi,
}

/// Which API endpoint was called.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EndpointKind {
    ChatCompletions,
    Completions,
    Embeddings,
    ImagesGenerations,
    ImagesEdits,
    MusicGenerations,
    VideosGenerations,
    AudioTranscriptions,
    AudioSpeech,
    Messages,
    Responses,
    Search,
    Fetch,
    ResearchCreate,
    ResearchList,
    ResearchGet,
    CreditsBalance,
}

/// Role in a conversation turn.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MessageRole {
    System,
    User,
    Assistant,
    Tool,
}

/// A single content part within a message.
///
/// The `#[serde(tag = "type")]` discriminant matches the wire format used by
/// both OpenAI and Anthropic APIs, and mirrors the legacy TypeScript
/// `CanonicalContentPart` union type.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ContentPart {
    Text {
        text: String,
    },
    ImageUrl {
        image_url: String,
        detail: Option<String>,
    },
    Json {
        value: Value,
    },
    Raw {
        value: Value,
    },
}

impl ContentPart {
    /// Return the text payload if this part is a [`ContentPart::Text`] variant,
    /// otherwise return `None`.
    pub fn as_text(&self) -> Option<&str> {
        match self {
            ContentPart::Text { text } => Some(text.as_str()),
            ContentPart::Raw { value } => value
                .get("type")
                .and_then(|kind| kind.as_str())
                .filter(|kind| *kind == "text")
                .and_then(|_| value.get("text"))
                .and_then(|text| text.as_str()),
            _ => None,
        }
    }
}

/// A tool call emitted by the assistant.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CanonicalToolCall {
    pub id: Option<String>,
    /// The call type (e.g. `"function"`).
    #[serde(rename = "type")]
    pub call_type: String,
    pub name: Option<String>,
    /// JSON-encoded argument string, matching the OpenAI wire format.
    pub arguments: Option<String>,
    /// Original vendor-specific fields, preserved verbatim.
    pub raw: HashMap<String, Value>,
}

/// A single message in the conversation history.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CanonicalMessage {
    pub role: MessageRole,
    pub content: Vec<ContentPart>,
    pub name: Option<String>,
    pub tool_call_id: Option<String>,
    pub tool_calls: Vec<CanonicalToolCall>,
}

impl CanonicalMessage {
    /// Join all [`ContentPart::Text`] parts into a single string separated by
    /// newlines.  Non-text parts are silently skipped.
    pub fn text_content(&self) -> String {
        self.content
            .iter()
            .filter_map(|p| p.as_text())
            .collect::<Vec<_>>()
            .join("\n")
    }
}

/// A tool definition provided by the caller.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CanonicalTool {
    /// Tool type string (e.g. `"function"`).
    #[serde(rename = "type")]
    pub tool_type: String,
    pub name: Option<String>,
    pub description: Option<String>,
    /// JSON Schema object describing the tool's input parameters.
    pub input_schema: Option<Value>,
    /// Original vendor-specific fields, preserved verbatim.
    pub raw: HashMap<String, Value>,
}

/// Token counts reported by the upstream provider.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct TokenUsage {
    pub prompt_tokens: u64,
    pub completion_tokens: u64,
    pub total_tokens: u64,
    pub cache_creation_input_tokens: Option<u64>,
    pub cache_read_input_tokens: Option<u64>,
}

/// The canonical, vendor-neutral relay request.
///
/// This is the intermediate representation that all protocol adapters
/// (OpenAI, Anthropic) normalise into before the request enters the
/// routing/filtering pipeline. It mirrors the legacy TypeScript
/// `CanonicalRelayRequest` shape and
/// adds the `extra` escape hatch from the Rust NeuroLoom codebase.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CanonicalRelayRequest {
    /// Which protocol family the request originated from.
    pub protocol_family: ProtocolFamily,
    /// The specific endpoint that was called.
    pub endpoint_kind: EndpointKind,
    /// The model identifier requested by the caller, if any.
    pub requested_model: Option<String>,
    /// Whether the caller wants a streaming response.
    pub stream: bool,
    /// The full conversation history in canonical form.
    pub messages: Vec<CanonicalMessage>,
    /// Tool definitions available to the model.
    pub tools: Vec<CanonicalTool>,
    /// Tool-choice directive (vendor-specific JSON structure, preserved as-is).
    pub tool_choice: Option<Value>,
    /// Reasoning / thinking configuration blob (vendor-specific).
    pub reasoning: Option<Value>,
    /// Caller-supplied metadata key-value pairs.
    pub metadata: Option<HashMap<String, Value>>,
    /// The raw, unparsed request body — kept so adapters can re-pack it for
    /// the upstream without round-trip loss.
    pub raw_body: Value,
    /// Responses API: the ID of the response this continues.
    pub previous_response_id: Option<String>,
    /// Explicit session key extracted from caller-supplied session/user fields.
    pub explicit_session_key: Option<String>,
    /// Vendor-specific parameter escape hatch.
    ///
    /// Key-value pairs here are merged into the packed upstream request body,
    /// allowing callers to pass provider-specific parameters without changing
    /// the canonical type.
    #[serde(default)]
    pub extra: HashMap<String, Value>,
}

impl CanonicalRelayRequest {
    /// Concatenate all text content from all messages, separated by newlines.
    ///
    /// Useful as a quick plaintext view for content-filtering purposes.
    pub fn messages_text(&self) -> String {
        let mut parts: Vec<&str> = Vec::new();
        for msg in &self.messages {
            for part in &msg.content {
                if let Some(text) = part.as_text() {
                    parts.push(text);
                }
            }
        }
        parts.join("\n")
    }

    /// Return the text of the first system message, if one exists.
    pub fn system_message(&self) -> Option<&str> {
        self.messages
            .iter()
            .find(|m| m.role == MessageRole::System)
            .and_then(|m| m.content.first())
            .and_then(|p| p.as_text())
    }
}

/// The canonical response produced after relaying to an upstream provider.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CanonicalRelayResponse {
    /// The model name reported by the upstream provider.
    pub model: String,
    /// Assembled text content of the response.
    pub text: String,
    /// Token usage figures, if reported by the upstream.
    pub usage: Option<TokenUsage>,
    /// Any tool calls the model requested.
    pub tool_calls: Vec<CanonicalToolCall>,
    /// The HTTP status code returned by the upstream provider.
    pub upstream_status: Option<u16>,
    /// The finish/stop reason string (e.g. `"stop"`, `"tool_calls"`).
    pub finish_reason: Option<String>,
}

/// A single chunk in a streaming response.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StreamChunk {
    /// Incremental text delta for this chunk.
    pub delta_text: Option<String>,
    /// Incremental tool-call delta for this chunk.
    pub delta_tool_call: Option<CanonicalToolCall>,
    /// Model name (usually only present in the first chunk).
    pub model: Option<String>,
    /// Usage figures (usually only present in the final chunk).
    pub usage: Option<TokenUsage>,
    /// Finish reason (only set on the terminal chunk).
    pub finish_reason: Option<String>,
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    // ------------------------------------------------------------------
    // Helpers
    // ------------------------------------------------------------------

    fn make_request(messages: Vec<CanonicalMessage>) -> CanonicalRelayRequest {
        CanonicalRelayRequest {
            protocol_family: ProtocolFamily::OpenAi,
            endpoint_kind: EndpointKind::ChatCompletions,
            requested_model: Some("gpt-4o".to_string()),
            stream: false,
            messages,
            tools: vec![],
            tool_choice: None,
            reasoning: None,
            metadata: None,
            raw_body: json!({}),
            previous_response_id: None,
            explicit_session_key: None,
            extra: HashMap::new(),
        }
    }

    fn text_msg(role: MessageRole, text: &str) -> CanonicalMessage {
        CanonicalMessage {
            role,
            content: vec![ContentPart::Text {
                text: text.to_string(),
            }],
            name: None,
            tool_call_id: None,
            tool_calls: vec![],
        }
    }

    // ------------------------------------------------------------------
    // Serialisation round-trip
    // ------------------------------------------------------------------

    #[test]
    fn roundtrip_protocol_family() {
        let cases = [
            (ProtocolFamily::OpenAi, r#""open_ai""#),
            (ProtocolFamily::OpenAiRealtime, r#""open_ai_realtime""#),
            (ProtocolFamily::Anthropic, r#""anthropic""#),
            (
                ProtocolFamily::GeminiGenerateContent,
                r#""gemini_generate_content""#,
            ),
            (ProtocolFamily::GeminiLive, r#""gemini_live""#),
            (ProtocolFamily::BedrockConverse, r#""bedrock_converse""#),
            (ProtocolFamily::CohereChat, r#""cohere_chat""#),
            (ProtocolFamily::SearchApi, r#""search""#),
        ];
        for (family, expected_json) in cases {
            let json = serde_json::to_string(&family).unwrap();
            assert_eq!(json, expected_json);
            let decoded: ProtocolFamily = serde_json::from_str(&json).unwrap();
            assert_eq!(decoded, family);
        }
    }

    #[test]
    fn legacy_protocol_family_alias_deserializes_to_search() {
        let decoded: ProtocolFamily = serde_json::from_str(r#""linkup""#).unwrap();
        assert_eq!(decoded, ProtocolFamily::SearchApi);
        let decoded: ProtocolFamily = serde_json::from_str(r#""search_api""#).unwrap();
        assert_eq!(decoded, ProtocolFamily::SearchApi);
    }

    #[test]
    fn roundtrip_endpoint_kind() {
        let cases = [
            (EndpointKind::ChatCompletions, r#""chat_completions""#),
            (EndpointKind::Completions, r#""completions""#),
            (EndpointKind::Embeddings, r#""embeddings""#),
            (EndpointKind::ImagesGenerations, r#""images_generations""#),
            (EndpointKind::ImagesEdits, r#""images_edits""#),
            (EndpointKind::MusicGenerations, r#""music_generations""#),
            (EndpointKind::VideosGenerations, r#""videos_generations""#),
            (
                EndpointKind::AudioTranscriptions,
                r#""audio_transcriptions""#,
            ),
            (EndpointKind::AudioSpeech, r#""audio_speech""#),
            (EndpointKind::Messages, r#""messages""#),
            (EndpointKind::Responses, r#""responses""#),
            (EndpointKind::Search, r#""search""#),
            (EndpointKind::Fetch, r#""fetch""#),
            (EndpointKind::ResearchCreate, r#""research_create""#),
            (EndpointKind::ResearchList, r#""research_list""#),
            (EndpointKind::ResearchGet, r#""research_get""#),
            (EndpointKind::CreditsBalance, r#""credits_balance""#),
        ];
        for (kind, expected_json) in cases {
            let json = serde_json::to_string(&kind).unwrap();
            assert_eq!(json, expected_json);
            let decoded: EndpointKind = serde_json::from_str(&json).unwrap();
            assert_eq!(decoded, kind);
        }
    }

    #[test]
    fn roundtrip_message_role() {
        let cases = [
            (MessageRole::System, r#""system""#),
            (MessageRole::User, r#""user""#),
            (MessageRole::Assistant, r#""assistant""#),
            (MessageRole::Tool, r#""tool""#),
        ];
        for (role, expected_json) in cases {
            let json = serde_json::to_string(&role).unwrap();
            assert_eq!(json, expected_json);
            let decoded: MessageRole = serde_json::from_str(&json).unwrap();
            assert_eq!(decoded, role);
        }
    }

    #[test]
    fn roundtrip_content_part_text() {
        let part = ContentPart::Text {
            text: "hello world".to_string(),
        };
        let json = serde_json::to_string(&part).unwrap();
        // Must contain the discriminant field
        assert!(json.contains(r#""type":"text""#));
        let decoded: ContentPart = serde_json::from_str(&json).unwrap();
        assert_eq!(decoded.as_text(), Some("hello world"));
    }

    #[test]
    fn roundtrip_content_part_image_url() {
        let part = ContentPart::ImageUrl {
            image_url: "https://example.com/img.png".to_string(),
            detail: Some("high".to_string()),
        };
        let json = serde_json::to_string(&part).unwrap();
        assert!(json.contains(r#""type":"image_url""#));
        let decoded: ContentPart = serde_json::from_str(&json).unwrap();
        // Image variants don't return text
        assert_eq!(decoded.as_text(), None);
    }

    #[test]
    fn roundtrip_full_request() {
        let req = make_request(vec![
            text_msg(MessageRole::System, "You are a helpful assistant."),
            text_msg(MessageRole::User, "Hello!"),
        ]);
        let json = serde_json::to_string(&req).unwrap();
        let decoded: CanonicalRelayRequest = serde_json::from_str(&json).unwrap();
        assert_eq!(decoded.messages.len(), 2);
        assert_eq!(decoded.requested_model.as_deref(), Some("gpt-4o"));
        assert!(!decoded.stream);
    }

    #[test]
    fn roundtrip_token_usage_default() {
        let usage = TokenUsage::default();
        let json = serde_json::to_string(&usage).unwrap();
        let decoded: TokenUsage = serde_json::from_str(&json).unwrap();
        assert_eq!(decoded.prompt_tokens, 0);
        assert_eq!(decoded.completion_tokens, 0);
        assert_eq!(decoded.total_tokens, 0);
    }

    #[test]
    fn roundtrip_stream_chunk() {
        let chunk = StreamChunk {
            delta_text: Some("delta".to_string()),
            delta_tool_call: None,
            model: Some("gpt-4o".to_string()),
            usage: None,
            finish_reason: None,
        };
        let json = serde_json::to_string(&chunk).unwrap();
        let decoded: StreamChunk = serde_json::from_str(&json).unwrap();
        assert_eq!(decoded.delta_text.as_deref(), Some("delta"));
        assert_eq!(decoded.model.as_deref(), Some("gpt-4o"));
    }

    // ------------------------------------------------------------------
    // messages_text()
    // ------------------------------------------------------------------

    #[test]
    fn messages_text_joins_all_text_parts() {
        let req = make_request(vec![
            text_msg(MessageRole::System, "System prompt."),
            text_msg(MessageRole::User, "User message."),
            text_msg(MessageRole::Assistant, "Assistant reply."),
        ]);
        let text = req.messages_text();
        assert_eq!(text, "System prompt.\nUser message.\nAssistant reply.");
    }

    #[test]
    fn messages_text_skips_non_text_parts() {
        let msg = CanonicalMessage {
            role: MessageRole::User,
            content: vec![
                ContentPart::Text {
                    text: "before".to_string(),
                },
                ContentPart::Json {
                    value: json!({"key": "val"}),
                },
                ContentPart::Text {
                    text: "after".to_string(),
                },
            ],
            name: None,
            tool_call_id: None,
            tool_calls: vec![],
        };
        let req = make_request(vec![msg]);
        assert_eq!(req.messages_text(), "before\nafter");
    }

    #[test]
    fn messages_text_empty_when_no_messages() {
        let req = make_request(vec![]);
        assert_eq!(req.messages_text(), "");
    }

    // ------------------------------------------------------------------
    // system_message()
    // ------------------------------------------------------------------

    #[test]
    fn system_message_finds_first_system() {
        let req = make_request(vec![
            text_msg(MessageRole::System, "Be concise."),
            text_msg(MessageRole::User, "Hello?"),
        ]);
        assert_eq!(req.system_message(), Some("Be concise."));
    }

    #[test]
    fn system_message_returns_none_when_absent() {
        let req = make_request(vec![text_msg(MessageRole::User, "Hello?")]);
        assert_eq!(req.system_message(), None);
    }

    #[test]
    fn system_message_returns_none_for_non_text_system_content() {
        let msg = CanonicalMessage {
            role: MessageRole::System,
            content: vec![ContentPart::Json {
                value: json!({"instructions": "be brief"}),
            }],
            name: None,
            tool_call_id: None,
            tool_calls: vec![],
        };
        let req = make_request(vec![msg]);
        // First content part is Json, not Text — should return None
        assert_eq!(req.system_message(), None);
    }

    // ------------------------------------------------------------------
    // extra field
    // ------------------------------------------------------------------

    #[test]
    fn extra_field_round_trips_arbitrary_json() {
        let mut extra = HashMap::new();
        extra.insert("temperature".to_string(), json!(0.7));
        extra.insert("top_p".to_string(), json!(0.9));
        extra.insert(
            "vendor_flag".to_string(),
            json!({"nested": true, "count": 3}),
        );

        let mut req = make_request(vec![text_msg(MessageRole::User, "hi")]);
        req.extra = extra.clone();

        let json = serde_json::to_string(&req).unwrap();
        let decoded: CanonicalRelayRequest = serde_json::from_str(&json).unwrap();

        assert_eq!(decoded.extra["temperature"], json!(0.7));
        assert_eq!(decoded.extra["top_p"], json!(0.9));
        assert_eq!(
            decoded.extra["vendor_flag"],
            json!({"nested": true, "count": 3})
        );
    }

    #[test]
    fn extra_field_defaults_to_empty_map_when_absent() {
        // Build a JSON string that deliberately omits the `extra` field
        let json_str = r#"{
            "protocol_family": "open_ai",
            "endpoint_kind": "chat_completions",
            "requested_model": null,
            "stream": false,
            "messages": [],
            "tools": [],
            "tool_choice": null,
            "reasoning": null,
            "metadata": null,
            "raw_body": {},
            "previous_response_id": null,
            "explicit_session_key": null
        }"#;
        let decoded: CanonicalRelayRequest = serde_json::from_str(json_str).unwrap();
        assert!(decoded.extra.is_empty());
    }

    // ------------------------------------------------------------------
    // CanonicalMessage::text_content()
    // ------------------------------------------------------------------

    #[test]
    fn text_content_joins_text_parts_only() {
        let msg = CanonicalMessage {
            role: MessageRole::User,
            content: vec![
                ContentPart::Text {
                    text: "part one".to_string(),
                },
                ContentPart::Raw {
                    value: json!("ignored"),
                },
                ContentPart::Text {
                    text: "part two".to_string(),
                },
            ],
            name: None,
            tool_call_id: None,
            tool_calls: vec![],
        };
        assert_eq!(msg.text_content(), "part one\npart two");
    }

    #[test]
    fn text_content_empty_for_no_content() {
        let msg = CanonicalMessage {
            role: MessageRole::User,
            content: vec![],
            name: None,
            tool_call_id: None,
            tool_calls: vec![],
        };
        assert_eq!(msg.text_content(), "");
    }

    // ------------------------------------------------------------------
    // ContentPart::as_text()
    // ------------------------------------------------------------------

    #[test]
    fn as_text_returns_str_for_text_variant() {
        let part = ContentPart::Text {
            text: "hello".to_string(),
        };
        assert_eq!(part.as_text(), Some("hello"));
    }

    #[test]
    fn as_text_returns_none_for_other_variants() {
        let image = ContentPart::ImageUrl {
            image_url: "http://x.com/img.jpg".to_string(),
            detail: None,
        };
        let json_part = ContentPart::Json { value: json!(42) };
        let raw = ContentPart::Raw { value: json!(null) };
        assert_eq!(image.as_text(), None);
        assert_eq!(json_part.as_text(), None);
        assert_eq!(raw.as_text(), None);
    }
}
