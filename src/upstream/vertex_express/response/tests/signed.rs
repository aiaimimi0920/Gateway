//! Signatures belong to original parts, including empty trailing text, not flat answer text.
use super::*;

fn response(parts: Value, reason: &str) -> Vec<u8> {
    event(json!({"candidates":[{"content":{"parts":parts},"finishReason":reason}]}))
}

#[test]
fn vertex_express_signed_turn_preserves_part_boundaries_and_public_tool_identity() {
    let parts = json!([
        {"text":"summary","thought":true,"thoughtSignature":"c3VtbWFyeQ=="},
        {"text":"A","thought":false,"thoughtSignature":"YQ=="},
        {"functionCall":{"name":"lookup","args":{"x":1}},"thoughtSignature":"Yg=="},
        {"text":"B"},
        {"functionCall":{"name":"lookup","args":{"x":1}}},
        {"text":"","thoughtSignature":"Yw=="}
    ]);
    let parsed = parse(&response(parts, "STOP"), "model").unwrap();
    assert_eq!(parsed.text, "AB");
    assert_eq!(parsed.tool_calls.len(), 2);
    let value = serde_json::to_value(&parsed).unwrap();
    let turn = &value["vertex_turn"];
    assert_eq!(turn["source"], "vertex_express_api");
    assert_eq!(turn["model"], "model");
    let parts = turn["parts"]
        .as_array()
        .expect("typed ordered assistant parts");
    assert_eq!(parts.len(), 6);
    assert_eq!(parts[0]["thought"], true);
    assert_eq!(parts[0]["text"], "summary");
    assert_eq!(parts[1]["thought"], false);
    assert_eq!(
        parts[2]["public_id"],
        parsed.tool_calls[0].id.as_ref().unwrap().as_str()
    );
    assert_eq!(
        parts[4]["public_id"],
        parsed.tool_calls[1].id.as_ref().unwrap().as_str()
    );
    assert_eq!(parts[5]["text"], "");
    assert_eq!(parts[5]["thought_signature"], "Yw==");
}

#[test]
fn vertex_express_signed_turn_collects_empty_signature_in_last_frame() {
    let raw = [
        event(json!({"candidates":[{"content":{"parts":[{"text":"answer"}]}}]})),
        response(json!([{"text":"","thoughtSignature":"YWJj"}]), "STOP"),
    ]
    .concat();
    let parsed = parse(&raw, "model").unwrap();
    assert_eq!(parsed.text, "answer");
    let value = serde_json::to_value(parsed).unwrap();
    assert_eq!(
        value["vertex_turn"]["parts"][1]["thought_signature"],
        "YWJj"
    );
}

#[test]
fn vertex_express_signed_turn_rejects_malformed_or_unrepresentable_signed_parts() {
    for part in [
        json!({"text":"private","thoughtSignature":4}),
        json!({"text":"private","thoughtSignature":"not base64!"}),
        json!({"thoughtSignature":"YWJj"}),
        json!({"inlineData":{"mimeType":"image/png","data":"YWJj"},"thoughtSignature":"YWJj"}),
        json!({"text":"private","unknown":"private","thoughtSignature":"YWJj"}),
        json!({"functionCall":{"name":"lookup","partialArgs":[{"jsonPath":"$.x","stringValue":"a"}]},"thoughtSignature":"YWJj"}),
    ] {
        let error = parse(&response(json!([part]), "STOP"), "model").unwrap_err();
        assert_eq!(error.http_status, Some(502));
        assert!(!error.retryable && !error.message.contains("private"));
        assert!(matches!(error.fallback_hint, FallbackHint::Abort { .. }));
    }
}

#[test]
fn vertex_express_signed_turn_safety_filter_discards_all_state_and_summary() {
    let parsed = parse(
        &response(
            json!([{"text":"summary","thought":true,"thoughtSignature":"YWJj"}]),
            "SAFETY",
        ),
        "model",
    )
    .unwrap();
    assert!(parsed.text.is_empty());
    assert!(serde_json::to_value(parsed)
        .unwrap()
        .get("vertex_turn")
        .is_none());
}
