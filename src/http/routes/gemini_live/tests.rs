use super::*;
use crate::protocol::canonical::CanonicalToolCall;

#[test]
fn build_setup_body_omits_null_optional_fields() {
    let body = build_gemini_setup_body(
        &json!({
            "model": "models/freebuff-glm-5-1-live",
            "generationConfig": {
                "temperature": 0
            }
        }),
        "freebuff-glm-5-1-live",
    );

    let object = body.as_object().unwrap();
    assert_eq!(object.get("model"), Some(&json!("freebuff-glm-5-1-live")));
    assert_eq!(object.get("contents"), Some(&json!([])));
    assert_eq!(object.get("tools"), Some(&json!([])));
    assert!(!object.contains_key("thinkingConfig"));
    assert!(!object.contains_key("toolConfig"));
    assert!(!object.contains_key("system_instruction"));
}

#[test]
fn build_turn_body_omits_null_optional_fields() {
    let session = GeminiLiveSession {
        model: "freebuff-glm-5-1-live".to_string(),
        tools: Vec::new(),
        tool_choice: None,
        reasoning: None,
        system_instruction: None,
        messages: vec![CanonicalMessage {
            role: MessageRole::User,
            content: vec![ContentPart::Text {
                text: "hello".to_string(),
            }],
            name: None,
            tool_call_id: None,
            tool_calls: vec![],
        }],
    };

    let body = build_gemini_live_turn_body(&session);
    let object = body.as_object().unwrap();
    assert_eq!(object.get("model"), Some(&json!("freebuff-glm-5-1-live")));
    assert!(object.contains_key("contents"));
    assert!(!object.contains_key("toolConfig"));
    assert!(!object.contains_key("thinkingConfig"));
    assert!(!object.contains_key("system_instruction"));
    assert!(!object.contains_key("tools"));
}

#[test]
fn setup_tool_config_fallback_preserves_explicit_precedence_and_false_values() {
    let mut setup = json!({
        "toolConfig": null,
        "systemInstruction": {"parts": [{"text": "Be precise"}]},
        "generationConfig": {
            "toolConfig": {"functionCallingConfig": {"mode": "ANY"}},
            "thinkingConfig": {"includeThoughts": false, "thinkingBudget": 0}
        }
    });
    let fallback = build_gemini_setup_body(&setup, "gemini-live");
    assert_eq!(
        fallback["toolConfig"]["functionCallingConfig"]["mode"],
        "ANY"
    );
    assert_eq!(
        fallback["thinkingConfig"],
        setup["generationConfig"]["thinkingConfig"]
    );
    assert_eq!(fallback["system_instruction"], setup["systemInstruction"]);

    setup["toolConfig"] = json!({"functionCallingConfig": {"mode": "NONE"}});
    let explicit = build_gemini_setup_body(&setup, "gemini-live");
    assert_eq!(
        explicit["toolConfig"]["functionCallingConfig"]["mode"],
        "NONE"
    );
}

#[test]
fn live_turn_preserves_multimodal_parts_and_excludes_system_history() {
    let mut session = GeminiLiveSession::new("gemini-live".into());
    let raw = json!({"inlineData": {"mimeType": "audio/pcm", "data": "AA=="}});
    session.messages = vec![
        message(
            MessageRole::System,
            vec![ContentPart::Text {
                text: "system".into(),
            }],
        ),
        message(
            MessageRole::User,
            vec![
                ContentPart::Text {
                    text: "describe".into(),
                },
                ContentPart::Json {
                    value: json!({"count": 2}),
                },
                ContentPart::ImageUrl {
                    image_url: "https://example.test/image.png".into(),
                    detail: None,
                },
                ContentPart::Raw { value: raw.clone() },
            ],
        ),
        message(MessageRole::User, vec![]),
    ];
    session.system_instruction = Some(json!({"parts": [{"text": "system"}]}));

    let body = build_gemini_live_turn_body(&session);
    assert_eq!(
        body["contents"],
        json!([
            {"role": "user", "parts": [
                {"text": "describe"},
                {"text": "{\"count\":2}"},
                {"fileData": {"fileUri": "https://example.test/image.png", "mimeType": "image/png"}},
                raw,
            ]},
            {"role": "user", "parts": [{"text": ""}]},
        ])
    );
    assert_eq!(
        body["system_instruction"],
        session.system_instruction.unwrap()
    );
}

#[test]
fn live_turn_preserves_tool_ids_order_and_malformed_argument_fallback() {
    let mut assistant = message(
        MessageRole::Assistant,
        vec![ContentPart::Text {
            text: "checking".into(),
        }],
    );
    assistant.tool_calls = [Some("{\"city\":\"Paris\"}"), Some("{"), None]
        .into_iter()
        .enumerate()
        .map(|(index, arguments)| CanonicalToolCall {
            id: Some(format!("call-{index}")),
            call_type: "function".into(),
            name: Some("weather".into()),
            arguments: arguments.map(str::to_owned),
            raw: HashMap::new(),
        })
        .collect();
    let mut response = message(
        MessageRole::Tool,
        vec![ContentPart::Json {
            value: json!({"temperature": 18}),
        }],
    );
    response.tool_call_id = Some("call-0".into());
    response.name = Some("weather".into());
    let mut session = GeminiLiveSession::new("gemini-live".into());
    session.messages = vec![assistant, response];

    assert_eq!(
        build_gemini_live_turn_body(&session)["contents"],
        json!([
            {"role": "model", "parts": [
                {"text": "checking"},
                {"functionCall": {"id": "call-0", "name": "weather", "args": {"city": "Paris"}}},
                {"functionCall": {"id": "call-1", "name": "weather", "args": {}}},
                {"functionCall": {"id": "call-2", "name": "weather", "args": {}}},
            ]},
            {"role": "user", "parts": [{"functionResponse": {
                "id": "call-0", "name": "weather", "response": {"temperature": 18}
            }}]},
        ])
    );
}

fn message(role: MessageRole, content: Vec<ContentPart>) -> CanonicalMessage {
    CanonicalMessage {
        role,
        content,
        name: None,
        tool_call_id: None,
        tool_calls: vec![],
    }
}
