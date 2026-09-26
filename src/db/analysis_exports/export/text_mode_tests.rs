use super::{
    build_analysis_dataset_jsonl, build_export_row, normalize_text_mode, GatewayAnalysisSampleView,
};
use serde_json::{json, Value};

fn sample() -> GatewayAnalysisSampleView {
    GatewayAnalysisSampleView {
        request_audit_id: "audit-fixture".to_string(),
        response_id: "response-fixture".to_string(),
        project_id: "project-fixture".to_string(),
        route_policy_id: None,
        session_id: None,
        provider_account_id: None,
        protocol_family: "openai".to_string(),
        endpoint_kind: "chat".to_string(),
        requested_model: None,
        resolved_model: None,
        status: "completed".to_string(),
        stream: false,
        created_at: "2026-09-13T00:00:00Z".to_string(),
        completed_at: None,
        prompt_tokens: Some(2),
        completion_tokens: Some(3),
        total_tokens: Some(5),
        cache_creation_input_tokens: None,
        cache_read_input_tokens: None,
        analysis_profile: None,
        request_artifact_object_key: Some("fixture/request.json".to_string()),
        response_artifact_object_key: Some("fixture/response.json".to_string()),
        route_trace: None,
    }
}

fn representations(
    request: &Value,
    response: &str,
    mode: Option<&str>,
    limit: usize,
) -> [Value; 2] {
    let response_artifact = json!({"result": {"text": response}});
    let row = build_export_row(
        &sample(),
        Some(request),
        Some(&response_artifact),
        normalize_text_mode(mode),
        limit,
    );
    let dataset = build_analysis_dataset_jsonl(std::slice::from_ref(&row)).expect("encode JSONL");
    assert_eq!(dataset.last(), Some(&b'\n'));
    assert_eq!(dataset.iter().filter(|&&byte| byte == b'\n').count(), 1);
    [
        serde_json::to_value(row).expect("encode API row"),
        serde_json::from_slice(&dataset).expect("decode JSONL row"),
    ]
}

fn sensitive_request() -> Value {
    json!({"canonicalRequest": {"messages": [
        {"role": "system", "content": [{"text": "system-fixture@example.invalid"}]},
        {"role": "user", "content": [{"value": "user-fixture@example.invalid"}]},
        {"role": "assistant", "toolCalls": [{"name": "fixture_tool"}], "content": [
            {"image_url": {"url": "https://example.invalid/photo?token=fixture-image-value"}}
        ]},
        {"role": "tool", "name": "fixture-tool", "tool_call_id": "call-fixture", "content": [
            {"type": "json", "value": {"email": "tool-fixture@example.invalid"}}
        ]}
    ]}})
}

#[test]
fn none_hides_flattened_and_structured_text() {
    for row in representations(
        &sensitive_request(),
        "response-fixture@example.invalid",
        Some("none"),
        4000,
    ) {
        assert!(row["requestText"].is_null());
        assert!(row["responseText"].is_null());
        assert_eq!(row["requestTextTruncated"], false);
        assert_eq!(row["responseTextTruncated"], false);
        let messages = row["requestMessages"].as_array().expect("message array");
        assert_eq!(messages.len(), 4);
        for (message, role) in messages.iter().zip(["system", "user", "assistant", "tool"]) {
            assert_eq!(message["role"], role);
            assert_eq!(message["text"], "");
        }
        assert_eq!(messages[2]["toolCallCount"], 1);
        assert_eq!(messages[3]["name"], "fixture-tool");
        assert_eq!(messages[3]["toolCallId"], "call-fixture");
        assert!(!row.to_string().contains("@example.invalid"));
        assert!(!row.to_string().contains("fixture-image-value"));
    }
}

#[test]
fn preview_redacts_every_message_in_json_and_jsonl() {
    for mode in [None, Some("preview_redacted"), Some("unknown")] {
        for row in representations(
            &sensitive_request(),
            "response-fixture@example.invalid",
            mode,
            4000,
        ) {
            assert_eq!(row["responseText"], "[REDACTED_EMAIL]");
            let messages = row["requestMessages"].as_array().expect("message array");
            assert_eq!(messages.len(), 4);
            for message in messages {
                assert!(message["text"]
                    .as_str()
                    .expect("message text")
                    .contains("[REDACTED"));
            }
            let serialized = row.to_string();
            for original in [
                "system-fixture@",
                "user-fixture@",
                "tool-fixture@",
                "fixture-image-value",
            ] {
                assert!(
                    !serialized.contains(original),
                    "raw text escaped the policy: {original}"
                );
            }
        }
    }
}

#[test]
fn full_mode_preserves_message_text_and_metadata() {
    let content = "full-fixture@example.invalid";
    let request = json!({"canonicalRequest": {"messages": [{
        "role": "tool", "name": "full-tool", "tool_call_id": "call-full",
        "tool_calls": [{"name": "fixture_tool"}], "content": [{"text": content}]
    }]}});
    for row in representations(&request, content, Some("full"), 4000) {
        assert_eq!(row["requestText"], content);
        assert_eq!(row["responseText"], content);
        assert_eq!(row["requestTextTruncated"], false);
        assert_eq!(row["responseTextTruncated"], false);
        assert_eq!(
            row["requestMessages"],
            json!([{
                "role": "tool", "name": "full-tool", "toolCallId": "call-full",
                "text": content, "toolCallCount": 1
            }])
        );
    }
}

#[test]
fn full_and_preview_bound_multibyte_message_text() {
    let content = "A\u{e9}\u{1f642}BC";
    let request = json!({"canonicalRequest": {"messages": [
        {"role": "user", "content": [{"text": content}]}
    ]}});
    for mode in ["full", "preview_redacted"] {
        for row in representations(&request, content, Some(mode), 3) {
            assert_eq!(row["requestText"], "A\u{e9}\u{1f642}");
            assert_eq!(row["responseText"], "A\u{e9}\u{1f642}");
            assert_eq!(row["requestMessages"][0]["text"], "A\u{e9}\u{1f642}");
            assert_eq!(row["requestTextTruncated"], true);
            assert_eq!(row["responseTextTruncated"], true);
        }
    }
}

#[test]
fn zero_budget_suppresses_message_text_without_discarding_metadata() {
    let request = json!({"canonicalRequest": {"messages": [
        {"role": "user", "name": "zero-budget-user", "content": [{"text": "nonempty"}]}
    ]}});
    for mode in ["full", "preview_redacted"] {
        for row in representations(&request, "nonempty", Some(mode), 0) {
            assert_eq!(row["requestText"], "");
            assert_eq!(row["responseText"], "");
            assert_eq!(row["requestTextTruncated"], true);
            assert_eq!(row["responseTextTruncated"], true);
            assert_eq!(
                row["requestMessages"],
                json!([{
                    "role": "user", "name": "zero-budget-user", "toolCallId": null,
                    "text": "", "toolCallCount": 0
                }])
            );
        }
    }
}

#[test]
fn preview_redacts_before_limiting_each_message() {
    let content = "preview-fixture@example.invalid trailing";
    let request = json!({"canonicalRequest": {"messages": [
        {"role": "user", "content": [{"text": content}]}
    ]}});
    for row in representations(&request, content, Some("preview_redacted"), 16) {
        assert_eq!(row["requestText"], "[REDACTED_EMAIL]");
        assert_eq!(row["responseText"], "[REDACTED_EMAIL]");
        assert_eq!(row["requestMessages"][0]["text"], "[REDACTED_EMAIL]");
        assert_eq!(row["requestTextTruncated"], true);
        assert_eq!(row["responseTextTruncated"], true);
    }
}
