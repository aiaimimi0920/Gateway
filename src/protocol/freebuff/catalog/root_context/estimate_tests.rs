//! Independent cheap-estimator vectors; no tokenizer/BPE dependency or third-party code execution.
use super::{estimate, BusinessError};
use serde_json::{json, Value};

#[test]
fn freebuff_root_context_text_estimates_utf16_and_utf8_without_bpe() {
    for (input, expected) in [
        ("", 0),
        ("abc", 1),
        ("abcd", 2),
        ("中", 3),
        ("😀", 3),
        ("a中😀", 6),
    ] {
        assert_eq!(estimate::text(input).unwrap(), expected, "{input}");
    }
    assert_eq!(estimate::text(&" ".repeat(3_000_000)).unwrap(), 1_000_000);
}

#[test]
fn freebuff_root_context_json_estimate_counts_structure_and_weighted_keys() {
    // 2 braces + key length 1 + 4 + quoted string length 3 = 10; ceil(10/3).
    assert_eq!(estimate::json(&json!({"a":"b"})).unwrap(), 4);
    assert_eq!(estimate::json(&json!([null, true, false, 123])).unwrap(), 8);
    assert_eq!(estimate::json(&json!({"中":"😀"})).unwrap(), 8);
}

#[test]
fn freebuff_root_context_json_wide_and_deep_limits_are_errors_not_zero() {
    let mut deep = json!(null);
    for _ in 0..101 {
        deep = json!([deep]);
    }
    assert_eq!(estimate::json(&deep), Err(BusinessError::RequestTooLarge));
    let wide = Value::Array(vec![Value::Null; 100_000]);
    assert_eq!(estimate::json(&wide), Err(BusinessError::RequestTooLarge));
    assert!(estimate::json(&Value::Array(vec![Value::Null; 99_999])).is_ok());
}

#[test]
fn freebuff_root_context_media_is_flat_not_base64_character_count() {
    for kind in [
        "image_url",
        "file",
        "file_url",
        "image",
        "input_audio",
        "video_url",
    ] {
        let value = json!({"role":"user","content":[{"type":kind,"data":"x".repeat(1_000_000)}]});
        assert_eq!(estimate::message(&value).unwrap(), 1608);
    }
}

#[test]
fn freebuff_root_context_native_tools_reasoning_and_unknown_parts_are_counted() {
    let base = json!({"role":"assistant","content":"abc","reasoning":"中文",
        "tool_calls":[{"id":"ignored","type":"function","function":{"name":"read_files","arguments":"{}"}}]});
    assert_eq!(estimate::message(&base).unwrap(), 8 + 1 + 5 + 4 + 1);
    let mut extended = base.clone();
    extended["tool_calls"][0]["provider_extension"] = json!({"kept":"large".repeat(100)});
    extended["reasoning_details"] = json!([{"data":"opaque"}]);
    assert!(estimate::message(&extended).unwrap() > estimate::message(&base).unwrap());
    let unknown = json!({"role":"user","content":[{"type":"future","data":"payload"}]});
    assert!(estimate::message(&unknown).unwrap() > 8);
    assert_eq!(
        estimate::message(&json!(null)),
        Err(BusinessError::InvalidInput)
    );
}

#[test]
fn freebuff_root_context_request_includes_system_and_wire_tool_schemas() {
    let mut value = json!({"messages":[{"role":"system","content":"abc"},{"role":"user","content":"abc"}],"tools":[]});
    assert_eq!(estimate::request(&value).unwrap(), 11);
    value["tools"] = json!([{"type":"function","function":{"name":"synthetic","parameters":{"description":"abc".repeat(100)}}}]);
    assert!(estimate::request(&value).unwrap() > 111);
}
