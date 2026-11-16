//! Synthetic reconstruction of the authorized observation, never a raw account capture.
use super::{Decoder, Update};
use bytes::Bytes;
use serde_json::{json, Value};

pub(super) fn complete() -> Value {
    json!({"status":"COMPLETED","final":true,"text_completed":true,
        "final_sse_message":true,"read_write_token":"synthetic-private-token",
        "blocks":[{"intended_usage":"workflow_root","workflow_block":{
            "version":"1.0","status":"WORKFLOW_COMPLETED","headline":"not an answer",
            "steps":[{"status":"WORKFLOW_COMPLETED","title":"diagnostic","items":[]},
                {"status":"WORKFLOW_COMPLETED","items":[{
                    "type":"WORKFLOW_ITEM_TEXT","variant":"answer","payload":{
                        "text_payload":{"text":"蓝色纸船","chunks":["蓝色纸船"],
                            "variant":"answer","is_streaming":false}}}]}],"snapshots":[]}},
            {"intended_usage":"answer_tabs","answer_tabs_block":{}},
            {"intended_usage":"pending_followups","pending_followups_block":{"followups":[]}}]})
}

pub(super) fn event(
    decoder: &mut Decoder,
    value: Value,
) -> Result<Option<Update>, crate::error::GatewayError> {
    decoder.push_chunk(Bytes::from(format!("data: {value}\n\n")))?;
    decoder.next_event()
}

#[test]
fn perplexity_workflow_materialized_answer_completes_without_eof() {
    let mut decoder = Decoder::new();
    let result = event(&mut decoder, complete()).unwrap().unwrap();
    let Update::Complete(answer) = result else {
        panic!("not complete")
    };
    assert_eq!(answer.text, "蓝色纸船");
    assert!(answer.sources.is_empty());
    assert!(!serde_json::to_string(&answer).unwrap().contains("private"));
    decoder.finish().unwrap();
}

#[test]
fn perplexity_workflow_pending_patch_never_completes() {
    let pending = json!({"status":"PENDING","final":true,"text_completed":true,
        "blocks":[{"intended_usage":"workflow_root","diff_block":{
            "field":"workflow_block","patches":[{"op":"replace","path":"/status",
                "value":"WORKFLOW_COMPLETED"}]}}]});
    let mut decoder = Decoder::new();
    assert_eq!(
        event(&mut decoder, pending.clone()).unwrap(),
        Some(Update::Snapshot(None))
    );
    assert!(decoder.finish().is_err());
    assert!(decoder
        .push(Bytes::from_static(b"event: end_of_stream\ndata: {}\n\n"))
        .is_err());
    let mut decoder = Decoder::new();
    event(&mut decoder, pending).unwrap();
    assert!(matches!(
        event(&mut decoder, complete()).unwrap(),
        Some(Update::Complete(_))
    ));
}

#[test]
fn perplexity_workflow_rejects_ambiguous_or_incomplete_answers() {
    let base = complete();
    let mut invalid = Vec::new();
    for field in ["final", "text_completed", "final_sse_message"] {
        let mut value = base.clone();
        value[field] = json!(false);
        invalid.push(value);
    }
    for (pointer, replacement) in [
        ("/blocks/0/workflow_block/version", json!("2.0")),
        (
            "/blocks/0/workflow_block/status",
            json!("WORKFLOW_EXECUTING_STEPS"),
        ),
        ("/blocks/0/workflow_block/steps/1/status", json!("PENDING")),
        (
            "/blocks/0/workflow_block/steps/1/items/0/variant",
            json!("thinking"),
        ),
        (
            "/blocks/0/workflow_block/steps/1/items/0/payload/text_payload/chunks",
            json!(["wrong"]),
        ),
        (
            "/blocks/0/workflow_block/steps/1/items/0/payload/text_payload/is_streaming",
            json!(true),
        ),
        (
            "/blocks/0/workflow_block/steps/1/items/0/payload/text_payload/text",
            json!(""),
        ),
    ] {
        let mut value = base.clone();
        *value.pointer_mut(pointer).unwrap() = replacement;
        invalid.push(value);
    }
    let mut duplicate = base.clone();
    let item = duplicate["blocks"][0]["workflow_block"]["steps"][1]["items"][0].clone();
    duplicate["blocks"][0]["workflow_block"]["steps"][1]["items"]
        .as_array_mut()
        .unwrap()
        .push(item);
    invalid.push(duplicate);
    let mut mixed = base.clone();
    mixed["blocks"]
        .as_array_mut()
        .unwrap()
        .push(json!({"intended_usage":"ask_text",
        "markdown_block":{"answer":"different"}}));
    invalid.push(mixed);
    let mut legacy = base;
    legacy["text"] = json!("legacy");
    invalid.push(legacy);
    for value in invalid {
        assert!(
            event(&mut Decoder::new(), value.clone()).is_err(),
            "{value}"
        );
    }
}
