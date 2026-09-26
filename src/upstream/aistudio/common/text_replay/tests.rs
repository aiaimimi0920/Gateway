use super::*;

#[test]
fn build_program_owned_text_replay_plan_extracts_model_and_prompt() {
    let body = json!({
        "contents": [{
            "role": "user",
            "parts": [{"text":"Reply with exactly OK"}]
        }]
    });
    let plan = build_program_owned_text_replay_plan(
        "https://generativelanguage.googleapis.com/v1beta/models/gemini-3-flash-preview:generateContent",
        &body,
    )
    .expect("plan");
    assert_eq!(plan.model, "gemini-3-flash-preview");
    assert_eq!(plan.prompt_text, "Reply with exactly OK");
    assert!(plan.deterministic_response.is_none());
}

#[test]
fn build_program_owned_text_replay_plan_builds_deterministic_tool_bridge() {
    let body = json!({
        "contents": [{
            "role": "user",
            "parts": [{"text":"Use weather for Hangzhou"}]
        }],
        "tools": [{
            "functionDeclarations": [{
                "name": "weather",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "city": { "type": "string" }
                    }
                }
            }]
        }],
        "toolConfig": {
            "functionCallingConfig": {
                "mode": "ANY"
            }
        }
    });
    let plan = build_program_owned_text_replay_plan(
        "https://generativelanguage.googleapis.com/v1beta/models/gemini-3-flash-preview:generateContent",
        &body,
    )
    .expect("plan");
    assert_eq!(
        plan.transport_owner,
        Some(AISTUDIO_DETERMINISTIC_TOOL_BRIDGE_OWNER)
    );
    let response = plan.deterministic_response.expect("deterministic response");
    assert_eq!(
        response["candidates"][0]["content"]["parts"][0]["functionCall"]["name"],
        json!("weather")
    );
    assert_eq!(
        response["candidates"][0]["content"]["parts"][0]["functionCall"]["args"]["city"],
        json!("Hangzhou")
    );
}

#[test]
fn build_program_owned_text_replay_plan_builds_deterministic_roundtrip_bridge() {
    let body = json!({
        "contents": [{
            "role": "user",
            "parts": [{
                "functionResponse": {
                    "name": "weather",
                    "response": {
                        "city": "Hangzhou",
                        "condition": "sunny"
                    }
                }
            }]
        }]
    });
    let plan = build_program_owned_text_replay_plan(
        "https://generativelanguage.googleapis.com/v1beta/models/gemini-3-flash-preview:generateContent",
        &body,
    )
    .expect("plan");
    assert_eq!(
        plan.transport_owner,
        Some(AISTUDIO_DETERMINISTIC_ROUNDTRIP_BRIDGE_OWNER)
    );
    let response = plan.deterministic_response.expect("deterministic response");
    assert_eq!(
        response["candidates"][0]["content"]["parts"][0]["text"],
        json!("The current weather in Hangzhou is sunny.")
    );
}

#[test]
fn extract_code_assistant_generation_id_reads_first_array_string() {
    assert_eq!(
        extract_code_assistant_generation_id(r#"["generation-123",{"ok":true}]"#),
        Some("generation-123".to_string())
    );
}

#[test]
fn extract_final_text_from_code_assistant_stream_reads_last_model_text() {
    let body = json!([[["draft", "final answer"], "model"], [["ignored"], "user"]]);
    assert_eq!(
        extract_final_text_from_code_assistant_stream(body.to_string().as_str()),
        Some("final answer".to_string())
    );
}

#[test]
fn build_code_assistant_offline_request_body_uses_expected_slots() {
    let body = build_code_assistant_offline_request_body(
        "Reply with exactly OK.",
        "models/gemini-3-flash-preview",
        "app-123",
        "!opaque",
    );
    assert_eq!(body[1], Value::String("!opaque".to_string()));
    assert_eq!(
        body[7],
        Value::String("models/gemini-3-flash-preview".to_string())
    );
    assert_eq!(body[11], Value::String("app-123".to_string()));
    assert_eq!(body[20], Value::String("app-123".to_string()));
}

#[test]
fn build_stream_request_body_uses_generation_id_and_app_id() {
    let body = build_stream_code_assistant_offline_generation_request_body("gen-123", "app-123");
    assert_eq!(body, json!(["gen-123", null, null, "app-123"]));
}
