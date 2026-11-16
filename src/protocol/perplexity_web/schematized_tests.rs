//! 合成 wire 验证明确 markdown profile，不连接消费端或借用账号。
use super::{Decoder, Update};
use bytes::Bytes;
use serde_json::{json, Value};

fn event(decoder: &mut Decoder, value: Value) -> Result<Update, crate::error::GatewayError> {
    decoder.push_chunk(Bytes::from(format!("data: {value}\n\n")))?;
    Ok(decoder.next_event()?.unwrap())
}

fn patch(patches: Value) -> Value {
    json!({"status":"PENDING","blocks":[{"intended_usage":"ask_text_0_markdown",
        "diff_block":{"field":"markdown_block","patches":patches}}]})
}

fn seed() -> Value {
    patch(json!([{"op":"replace","path":"","value":{"chunks":["Hello "]}}]))
}

fn complete(text: &str) -> Value {
    json!({"status":"COMPLETED","blocks":[{"intended_usage":"ask_text_0_markdown",
        "markdown_block":{"chunks":[text],"answer":text}}]})
}

#[test]
fn perplexity_schematized_accepts_answer_only_materialized_completion() {
    for with_partial in [false, true] {
        let mut decoder = Decoder::new();
        if with_partial {
            event(&mut decoder, seed()).unwrap();
        }
        let mut final_event = complete("最终答案");
        final_event["blocks"][0]["markdown_block"]
            .as_object_mut()
            .unwrap()
            .remove("chunks");
        let Update::Complete(answer) = event(&mut decoder, final_event).unwrap() else {
            panic!()
        };
        assert_eq!(answer.text, "最终答案");
        assert!(decoder.finish().is_ok());
    }
}

#[test]
fn perplexity_schematized_answer_only_cannot_hide_invalid_chunks_or_missing_terminal() {
    for value in [
        json!(null),
        json!(42),
        json!(""),
        json!(" "),
        json!("x".repeat(256 * 1024 + 1)),
    ] {
        let mut final_event = complete("unused");
        final_event["blocks"][0]["markdown_block"] = json!({"answer":value});
        assert!(event(&mut Decoder::new(), final_event).is_err());
    }
    for chunks in [json!(null), json!(42), json!([]), json!(["different"])] {
        let mut final_event = complete("valid");
        final_event["blocks"][0]["markdown_block"]["chunks"] = chunks;
        assert!(event(&mut Decoder::new(), final_event).is_err());
    }
    let mut pending = complete("not final");
    pending["status"] = json!("PENDING");
    pending["blocks"][0]["markdown_block"]
        .as_object_mut()
        .unwrap()
        .remove("chunks");
    assert!(event(&mut Decoder::new(), pending).is_err());
}

#[test]
fn perplexity_schematized_replaces_snapshots_and_requires_materialized_completion() {
    let mut decoder = Decoder::new();
    let Update::Snapshot(Some(answer)) = event(&mut decoder, seed()).unwrap() else {
        panic!()
    };
    assert_eq!(answer.text, "Hello ");
    let mut next = patch(json!([{"op":"add","path":"/chunks/1","value":"world"}]));
    next["final"] = json!(true);
    let Update::Snapshot(Some(answer)) = event(&mut decoder, next).unwrap() else {
        panic!()
    };
    assert_eq!(answer.text, "Hello world");
    assert!(decoder.finish().is_err());
    let Update::Complete(answer) = event(&mut decoder, complete("Hi")).unwrap() else {
        panic!()
    };
    assert_eq!(answer.text, "Hi");
    assert!(decoder.finish().is_ok());
}

#[test]
fn perplexity_schematized_rejects_uninitialized_sparse_and_unknown_patches() {
    let invalid = [
        json!({"op":"remove","path":"/chunks/0"}),
        json!({"op":"add","path":"/chunks/2","value":"gap"}),
        json!({"op":"add","path":"/chunks/0","value":"overwrite"}),
        json!({"op":"add","path":"/chunks/01","value":"alias"}),
        json!({"op":"add","path":"/chunks/-","value":"unknown"}),
        json!({"op":"replace","path":"","value":{"chunks":[42]}}),
        json!({"op":"add","path":"/chunks/1","value":null}),
    ];
    for operation in invalid {
        let mut decoder = Decoder::new();
        event(&mut decoder, seed()).unwrap();
        assert!(event(&mut decoder, patch(json!([operation]))).is_err());
    }
    assert!(event(
        &mut Decoder::new(),
        patch(json!([{"op":"add","path":"/chunks/0","value":"no seed"}]))
    )
    .is_err());
}

#[test]
fn perplexity_schematized_rejects_eof_only_terminal_and_mixed_profiles() {
    let mut decoder = Decoder::new();
    event(&mut decoder, seed()).unwrap();
    decoder
        .push_chunk(Bytes::from_static(b"event: end_of_stream\n\n"))
        .unwrap();
    assert!(decoder.next_event().is_err());
    let mut decoder = Decoder::new();
    event(&mut decoder, seed()).unwrap();
    assert!(event(
        &mut decoder,
        json!({"blocks":[{"intended_usage":"ask_text","markdown_block":{"answer":"other"}}]})
    )
    .is_err());
    let mut value = complete("ok");
    value["error_code"] = json!("private-error");
    assert!(event(&mut Decoder::new(), value).is_err());
}

#[test]
fn perplexity_schematized_keeps_only_final_sources_and_enforces_answer_budget() {
    let mut decoder = Decoder::new();
    let mut first = seed();
    first["blocks"]
        .as_array_mut()
        .unwrap()
        .push(json!({"intended_usage":"web_results",
        "web_result_block":{"web_results":[{"url":"https://example.com/old"}]}}));
    event(&mut decoder, first).unwrap();
    let Update::Complete(answer) = event(&mut decoder, complete("done")).unwrap() else {
        panic!()
    };
    assert!(answer.sources.is_empty());
    assert!(event(&mut Decoder::new(), complete(&"x".repeat(256 * 1024 + 1))).is_err());
    let mut mismatch = complete("a");
    mismatch["blocks"][0]["markdown_block"]["answer"] = json!("b");
    assert!(event(&mut Decoder::new(), mismatch).is_err());
}

#[test]
fn perplexity_schematized_rejects_workflow_and_auto_to_patch_switch() {
    for block in [
        json!({"intended_usage":"workflow_root"}),
        json!({"intended_usage":"unknown","workflow_block":{}}),
        json!({"intended_usage":"unknown","diff_block":{"field":"workflow_block","patches":[]}}),
    ] {
        assert!(event(
            &mut Decoder::new(),
            json!({"status":"PENDING","blocks":[block]})
        )
        .is_err());
    }
    for extra in [
        json!({"intended_usage":"workflow_root","workflow_block":{}}),
        json!({"intended_usage":"unknown","workflow_block":{}}),
        json!({"intended_usage":"ask_text","markdown_block":{"answer":"other"}}),
    ] {
        let mut value = complete("ok");
        value["blocks"].as_array_mut().unwrap().push(extra);
        assert!(event(&mut Decoder::new(), value).is_err());
    }
    let mut value = complete("ok");
    value["blocks"][0]["workflow_block"] = json!({});
    assert!(event(&mut Decoder::new(), value).is_err());
    let mut decoder = Decoder::new();
    event(
        &mut decoder,
        json!({"blocks":[{"intended_usage":"ask_text","markdown_block":{"answer":"auto"}}]}),
    )
    .unwrap();
    event(&mut decoder, json!({})).unwrap();
    assert!(event(&mut decoder, seed()).is_err());
}
