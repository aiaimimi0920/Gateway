use super::*;
use bytes::Bytes;
use serde_json::{json, Value};

fn frame(raw: Value) -> Bytes {
    Bytes::from(format!("data: {raw}\n\n"))
}
fn snapshot(answer: &str) -> Value {
    json!({"blocks":[{"intended_usage":"ask_text","markdown_block":{"answer":answer}}]})
}

#[test]
fn perplexity_web_terminal_payload_cannot_hide_error_or_replace_final_answer() {
    for payload in [
        json!({"error":"private-generation-failure"}),
        snapshot("final"),
        snapshot(""),
        json!("unknown-terminal"),
    ] {
        let mut decoder = Decoder::new();
        decoder.push(frame(snapshot("partial"))).unwrap();
        let error = decoder
            .push(Bytes::from(format!(
                "event: end_of_stream\ndata: {payload}\n\n"
            )))
            .unwrap_err();
        assert_eq!(
            error.code.as_deref(),
            Some("perplexity_web_unsupported_terminal_payload")
        );
        assert!(!format!("{error:?}").contains("private-generation-failure"));
    }
    let mut decoder = Decoder::new();
    decoder.push(frame(snapshot("complete"))).unwrap();
    assert_eq!(
        decoder
            .push(Bytes::from_static(b"event: end_of_stream\ndata: {}\n\n"))
            .unwrap()
            .unwrap()
            .text,
        "complete"
    );
}

#[test]
fn perplexity_web_complete_snapshots_do_not_inherit_prior_answer_sources() {
    let old = json!({"blocks":[
        {"intended_usage":"web_results","web_result_block":{"web_results":[{"url":"https://example.test/old"}]}},
        {"intended_usage":"ask_text","markdown_block":{"answer":"old"}}
    ]});
    for new in [
        snapshot("new"),
        json!({"blocks":[
            {"intended_usage":"web_results","web_result_block":{"web_results":[]}},
            {"intended_usage":"ask_text","markdown_block":{"answer":"new"}}
        ]}),
    ] {
        let mut decoder = Decoder::new();
        decoder.push(frame(old.clone())).unwrap();
        decoder.push(frame(new)).unwrap();
        let answer = decoder
            .push(Bytes::from_static(b"event: end_of_stream\n\n"))
            .unwrap()
            .unwrap();
        assert_eq!(answer.text, "new");
        assert!(answer.sources.is_empty());
    }
}

#[test]
fn perplexity_web_error_events_never_echo_payload_or_become_success() {
    let mut decoder = Decoder::new();
    decoder.push(frame(snapshot("partial"))).unwrap();
    let error = decoder
        .push(Bytes::from_static(
            b"event: error\ndata: private-provider-payload\n\n",
        ))
        .unwrap_err();
    assert_eq!(error.code.as_deref(), Some("perplexity_web_error_event"));
    assert!(!format!("{error:?}").contains("private-provider-payload"));
}
