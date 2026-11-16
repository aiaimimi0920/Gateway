//! Profile identity and budgets must hold even before a workflow has produced text.
use super::{
    workflow_tests::{complete, event},
    Decoder, Update,
};
use serde_json::{json, Value};

fn pending() -> Value {
    json!({"status":"PENDING","blocks":[{"intended_usage":"workflow_root","diff_block":{
        "field":"workflow_block","patches":[{"op":"replace","path":"","value":{}}]}}]})
}

#[test]
fn perplexity_workflow_rejects_cross_profile_switches() {
    let auto = json!({"blocks":[{"intended_usage":"ask_text","markdown_block":{"answer":"auto"}}]});
    let markdown = json!({"status":"PENDING","blocks":[{"intended_usage":"ask_text_0_markdown",
        "diff_block":{"field":"markdown_block","patches":[{"op":"replace","path":"",
            "value":{"chunks":[]}}]}}]});
    for other in [auto, markdown] {
        let mut decoder = Decoder::new();
        event(&mut decoder, other.clone()).unwrap();
        assert!(event(&mut decoder, complete()).is_err());
        let mut decoder = Decoder::new();
        event(&mut decoder, pending()).unwrap();
        assert!(event(&mut decoder, other).is_err());
    }
}

#[test]
fn perplexity_workflow_rejects_unobserved_patches_and_hidden_tracks() {
    for patch in [
        json!({"op":"add","path":"/steps/2","value":{}}),
        json!({"op":"remove","path":"/status"}),
        json!({"op":"replace","path":"/status","value":true}),
        json!({"op":"replace","path":"","value":"string"}),
    ] {
        let mut value = pending();
        value["blocks"][0]["diff_block"]["patches"] = json!([patch]);
        assert!(event(&mut Decoder::new(), value).is_err());
    }
    for field in [
        "workflow_block",
        "diff_block",
        "markdown_block",
        "web_result_block",
    ] {
        let mut value = complete();
        value["blocks"][1][field] = json!({});
        assert!(event(&mut Decoder::new(), value).is_err());
    }
    let mut value = complete();
    value["error_message"] = json!("synthetic-secret-error");
    let error = event(&mut Decoder::new(), value).unwrap_err();
    assert!(!format!("{error:?}").contains("synthetic-secret-error"));
}

#[test]
fn perplexity_workflow_enforces_tree_and_text_budgets() {
    let base = complete();
    for (pointer, replacement) in [
        ("/blocks", json!(vec![json!({}); 129])),
        (
            "/blocks/0/workflow_block/steps",
            json!(vec![json!({}); 129]),
        ),
        ("/blocks/0/workflow_block/snapshots", json!([{}])),
        (
            "/blocks/0/workflow_block/steps/1/items/0/payload/text_payload/chunks",
            json!(vec![""; 4097]),
        ),
        (
            "/blocks/0/workflow_block/steps/1/items/0/payload/text_payload/text",
            json!("x".repeat(256 * 1024 + 1)),
        ),
        (
            "/blocks/0/workflow_block/steps/1/items/0/type",
            json!("WORKFLOW_ITEM_IMAGE"),
        ),
        (
            "/blocks/0/workflow_block/steps/1/items/0/payload/text_payload/variant",
            json!("thinking"),
        ),
    ] {
        let mut value = base.clone();
        *value.pointer_mut(pointer).unwrap() = replacement;
        assert!(event(&mut Decoder::new(), value).is_err());
    }
    let mut value = pending();
    value["blocks"][0]["diff_block"]["patches"] = json!(vec![json!({}); 4097]);
    assert!(event(&mut Decoder::new(), value).is_err());
}

#[test]
fn perplexity_workflow_accepts_ui_prelude_and_split_utf8_chunks() {
    let mut decoder = Decoder::new();
    assert_eq!(
        event(
            &mut decoder,
            json!({"status":"PENDING","blocks":[{
        "intended_usage":"answer_tabs","answer_tabs_block":{}}]})
        )
        .unwrap(),
        Some(Update::Snapshot(None))
    );
    event(&mut decoder, pending()).unwrap();
    let mut value = complete();
    value["blocks"][0]["workflow_block"]["steps"][1]["items"][0]["payload"]["text_payload"]
        ["chunks"] = json!(["蓝色", "纸船"]);
    assert!(matches!(
        event(&mut decoder, value).unwrap(),
        Some(Update::Complete(_))
    ));
}
