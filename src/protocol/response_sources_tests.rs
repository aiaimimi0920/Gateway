//! The Gateway source-list extension is not inline citation annotations.
use super::canonical::CanonicalRelayResponse;
use super::{openai::unpack_openai_response, responses::unpack_responses_response};
use serde_json::{json, Value};

pub(super) fn sources() -> Value {
    json!([
        {"name":"原始标题", "url":"https://example.org/a?q=1#part", "snippet":"摘录 [1]"},
        {"name":"", "url":"http://example.net/b", "snippet":""},
        {"name":"原始标题", "url":"https://example.org/a?q=1#part", "snippet":"摘录 [1]"}
    ])
}

pub(super) fn chat() -> Value {
    json!({"model":"unknown", "choices":[{"message":{"role":"assistant",
        "content":"答案 [1]\n未改写。"}, "finish_reason":"stop"}], "gateway_sources":sources()})
}

pub(super) fn response() -> Value {
    json!({"model":"unknown", "status":"completed", "output":[{"type":"message",
        "role":"assistant", "content":[{"type":"output_text", "text":"答案 [1]\n未改写。"}]}],
        "gateway_sources":sources()})
}

#[test]
fn response_sources_json_unpack_preserves_sources_without_inventing_usage() {
    for result in [
        unpack_openai_response(&chat()),
        unpack_responses_response(&response()),
    ] {
        let canonical = result.unwrap();
        assert_eq!(canonical.text, "答案 [1]\n未改写。");
        assert_eq!(canonical.model, "unknown");
        assert!(canonical.usage.is_none());
        assert!(canonical.tool_calls.is_empty());
        let serialized = serde_json::to_value(canonical).unwrap();
        assert_eq!(serialized["sources"], sources());
        assert!(serialized.get("annotations").is_none());
    }
}

#[test]
fn response_sources_old_canonical_and_empty_wire_remain_compatible() {
    let old = json!({"model":"unknown", "text":"old", "usage":null,
        "tool_calls":[], "upstream_status":200, "finish_reason":"stop"});
    let canonical: CanonicalRelayResponse = serde_json::from_value(old.clone()).unwrap();
    assert_eq!(serde_json::to_value(canonical).unwrap(), old);
    for mut body in [chat(), response()] {
        body.as_object_mut().unwrap().remove("gateway_sources");
        let canonical = if body.get("choices").is_some() {
            unpack_openai_response(&body)
        } else {
            unpack_responses_response(&body)
        }
        .unwrap();
        assert!(serde_json::to_value(canonical)
            .unwrap()
            .get("sources")
            .is_none());
    }
}

#[test]
fn response_sources_invalid_or_excessive_extension_is_not_silently_dropped() {
    let invalid = [
        Value::Null,
        json!({}),
        json!([{}]),
        json!([{"name":"x", "url":"file:///secret", "snippet":""}]),
        json!([{"name":"x", "url":"https://user:pass@example.org", "snippet":""}]),
        json!([{"name":"x", "url":"javascript:alert(1)", "snippet":""}]),
        json!([{"name":4, "url":"https://example.org", "snippet":""}]),
        json!([{"name":"x".repeat(1025), "url":"https://example.org", "snippet":""}]),
        json!([{"name":"x", "url":"https://example.org", "snippet":"x".repeat(8193)}]),
        json!([{"name":"中".repeat(342), "url":"https://example.org", "snippet":""}]),
        json!([{"name":"x", "url":"https://example.org", "snippet":"中".repeat(2731)}]),
        json!([{"name":"x", "url":format!("https://example.org/{}", "x".repeat(2048)), "snippet":""}]),
        json!(vec![sources()[0].clone(); 65]),
    ];
    for extension in invalid {
        let mut body = chat();
        body["gateway_sources"] = extension.clone();
        assert!(
            unpack_openai_response(&body).is_err(),
            "accepted invalid extension"
        );
        let mut body = response();
        body["gateway_sources"] = extension;
        assert!(
            unpack_responses_response(&body).is_err(),
            "accepted invalid extension"
        );
    }
}

#[test]
fn response_sources_explicit_empty_is_omitted_and_extra_fields_do_not_project() {
    for mut body in [chat(), response()] {
        body["gateway_sources"] = json!([]);
        let result = if body.get("choices").is_some() {
            unpack_openai_response(&body)
        } else {
            unpack_responses_response(&body)
        }
        .unwrap();
        let canonical = serde_json::to_value(result).unwrap();
        assert!(canonical.get("sources").is_none());
    }
    let mut body = chat();
    body["gateway_sources"][0]["read_write_token"] = json!("must-not-project");
    let canonical = serde_json::to_value(unpack_openai_response(&body).unwrap()).unwrap();
    assert_eq!(canonical["sources"], sources());
    assert!(!canonical.to_string().contains("must-not-project"));
}
