//! Policy divergence and nested lossy fields are rejected without reaching a purchase owner.
use super::*;
#[test]
fn freebuff_public_json_rejects_raw_restoration_over_policy_edits() {
    for mut req in [
        crate::protocol::responses::normalize_responses(json!({"model":"open","input":"hello"}))
            .unwrap(),
        crate::protocol::anthropic::normalize_messages(
            json!({"model":"open","messages":[{"role":"user","content":"hello"}],"max_tokens":16}),
        )
        .unwrap(),
    ] {
        assert!(request(&req).is_ok());
        req.messages[0].content[0] = crate::protocol::canonical::ContentPart::Text {
            text: "policy replacement".into(),
        };
        assert!(request(&req).is_err());
    }
}
#[test]
fn freebuff_public_json_serialized_byte_budget_measures_escaping() {
    let value = json!({"x":"\n\"中"});
    let size = serde_json::to_vec(&value).unwrap().len();
    assert!(within_limit(&value, size));
    assert!(!within_limit(&value, size - 1));
}
#[test]
fn freebuff_public_json_limits_message_expansion_before_allocating_the_complete_mapping() {
    let raw = json!({"model":"open","max_tokens":1,"messages":vec![
        json!({"role":"user","content":[{"type":"text","text":"a"},{"type":"text","text":"b"}]});
        4096
    ]});
    assert!(
        messages::convert(&raw).is_err(),
        "converter itself must stop at its cumulative bound"
    );
}
