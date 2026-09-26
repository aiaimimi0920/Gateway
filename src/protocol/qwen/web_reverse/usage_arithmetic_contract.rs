use super::accumulation::accumulate_qwen_web_body;
use crate::protocol::canonical::TokenUsage;
use serde_json::{json, Value};

fn response_usage(usage: Option<Value>) -> Option<TokenUsage> {
    let mut body = json!({
        "model": "usage-fixture",
        "choices": [{"message": {"content": "usage fixture"}, "finish_reason": "stop"}],
    });
    if let Some(usage) = usage {
        body["usage"] = usage;
    }
    let response = accumulate_qwen_web_body(&body.to_string(), "requested-model")
        .expect("parse Qwen usage response");
    assert_eq!(response.text, "usage fixture");
    assert_eq!(response.model, "usage-fixture");
    assert_eq!(response.finish_reason.as_deref(), Some("stop"));
    response.usage
}

fn assert_counters(usage: Value, expected: [u64; 3]) {
    let parsed = response_usage(Some(usage)).expect("usage counters");
    assert_eq!(
        [
            parsed.prompt_tokens,
            parsed.completion_tokens,
            parsed.total_tokens
        ],
        expected
    );
}

#[test]
fn exact_nonoverflow_sums_are_preserved() {
    for (prompt, completion, total) in [
        (0, 0, 0),
        (3, 7, 10),
        (u64::MAX - 1, 1, u64::MAX),
        (u64::MAX, 0, u64::MAX),
    ] {
        assert_counters(
            json!({"prompt_tokens": prompt, "completion_tokens": completion}),
            [prompt, completion, total],
        );
    }
}

#[test]
fn reported_total_and_optional_fields_are_preserved() {
    assert_counters(
        json!({"prompt_tokens": 3, "completion_tokens": 7, "total_tokens": 9}),
        [3, 7, 9],
    );
    assert_counters(json!({"prompt_tokens": 12}), [12, 0, 12]);
    assert!(response_usage(None).is_none());
    assert!(response_usage(Some(json!({"completion_tokens": 7}))).is_none());
}

#[test]
fn overflowing_fallback_saturates() {
    for total in [None, Some(Value::Null), Some(json!("invalid"))] {
        let mut usage = json!({"prompt_tokens": u64::MAX, "completion_tokens": 1});
        if let Some(total) = total {
            usage["total_tokens"] = total;
        }
        assert_counters(usage, [u64::MAX, 1, u64::MAX]);
    }
    assert_counters(
        json!({"prompt_tokens": u64::MAX, "completion_tokens": u64::MAX}),
        [u64::MAX; 3],
    );
}

#[test]
fn overflowing_components_keep_reported_total() {
    assert_counters(
        json!({"prompt_tokens": u64::MAX, "completion_tokens": u64::MAX, "total_tokens": 17}),
        [u64::MAX, u64::MAX, 17],
    );
}
