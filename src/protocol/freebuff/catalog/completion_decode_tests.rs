//! Fixed-source native completion and reported SDK credits; no supplier charge inference.
use super::completion_decode::{decode_json, Accounting, StreamDecoder};
use super::*;
use bytes::Bytes;
use serde_json::{json, Value};
mod stream;

fn answer() -> Value {
    json!({"id":"answer", "model":"reported-model", "choices":[{"index":0,
        "message":{"role":"assistant", "content":"answer", "reasoning_content":"thought",
        "reasoning_details":[{"opaque":"keep"}]}, "finish_reason":"stop"}],
        "usage":{"prompt_tokens":3,"completion_tokens":4,"total_tokens":7,"cost":0}})
}

#[test]
fn freebuff_catalog_json_preserves_native_choices_reasoning_tools_and_zero_accounting() {
    let mut body = answer();
    body["choices"]
        .as_array_mut()
        .unwrap()
        .push(json!({"index":1,
        "message":{"content":null,"tool_calls":[{"id":"tool-1","type":"function",
        "function":{"name":"lookup","arguments":"{\"x\":1}"},"extra_content":{"opaque":true}}]},
        "finish_reason":"tool_calls"}));
    let mut accounting = Accounting::default();
    assert_eq!(decode_json(body.clone(), &mut accounting).unwrap(), body);
    assert_eq!(accounting.cost, Some(0.0));
    assert_eq!(accounting.upstream_estimate, None);
    assert_eq!(accounting.sdk_credits().unwrap(), 0);
}

#[test]
fn freebuff_catalog_json_rejects_error_missing_terminal_and_invalid_native_content() {
    let valid = answer();
    let mut variants = vec![
        json!({"error":{"message":"synthetic-secret"},"choices":[]}),
        json!(null),
    ];
    for (pointer, value) in [
        ("/choices", json!([])),
        ("/choices/0/finish_reason", json!(null)),
        ("/choices/0/finish_reason", json!("unknown")),
        ("/choices/0/message/content", json!([])),
        ("/choices/0/message/reasoning_content", json!(4)),
        ("/usage/prompt_tokens", json!(-1)),
        ("/usage/cost", json!("0")),
        ("/usage/cost", json!(-1)),
    ] {
        let mut body = valid.clone();
        *body.pointer_mut(pointer).unwrap() = value;
        variants.push(body);
    }
    let mut tools = valid.clone();
    tools["choices"][0]["message"]["tool_calls"] = json!([
        {"function":{"name":"lookup","arguments":"{bad"}}]);
    variants.push(tools);
    for body in variants {
        assert!(decode_json(body, &mut Accounting::default()).is_err());
    }
}

#[test]
fn freebuff_catalog_accounting_replaces_snapshots_and_distinguishes_unknown_from_zero() {
    let mut accounting = Accounting::default();
    assert_eq!(
        accounting.sdk_credits().err(),
        Some(BusinessError::AccountingUnknown)
    );
    for cost in [0.5, 0.25, 0.25] {
        accounting.observe(&json!({"cost":cost})).unwrap();
    }
    accounting
        .observe(&json!({"cost_details":{"upstream_inference_cost":0.125}}))
        .unwrap();
    assert_eq!(accounting.cost, Some(0.25));
    assert_eq!(accounting.sdk_credits().unwrap(), 40);
    accounting
        .observe(&json!({"cost":0,"cost_details":{"upstream_inference_cost":0}}))
        .unwrap();
    assert_eq!(accounting.sdk_credits().unwrap(), 0);
    for usage in [
        json!({"cost":-1}),
        json!({"cost":true}),
        json!({"cost_details":[]}),
        json!({"cost_details":{"upstream_inference_cost":"1"}}),
        json!(4),
    ] {
        assert!(accounting.observe(&usage).is_err());
        assert_eq!(accounting.sdk_credits().unwrap(), 0);
    }
    accounting.observe(&json!({"cost":1e308})).unwrap();
    assert!(accounting.sdk_credits().is_err());
}

#[test]
fn freebuff_catalog_accounting_matches_fixed_sdk_nonnegative_rounding() {
    for (cost, credits) in [
        (0.0, 0),
        (0.001, 0),
        (0.005, 1),
        (0.01, 1),
        (0.125, 13),
        (0.375, 40),
        (1.0, 106),
    ] {
        let mut accounting = Accounting::default();
        accounting.observe(&json!({"cost":cost})).unwrap();
        assert_eq!(accounting.sdk_credits().unwrap(), credits);
    }
}
