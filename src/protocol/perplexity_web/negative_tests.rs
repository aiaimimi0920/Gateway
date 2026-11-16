use super::{tests::request, *};
use crate::protocol::canonical::{ContentPart, EndpointKind, MessageRole};
use bytes::Bytes;
use serde_json::{json, Value};

fn snapshot(answer: &str) -> Value {
    json!({"blocks":[{"intended_usage":"ask_text","markdown_block":{"answer":answer}}]})
}
fn event(value: Value) -> String {
    format!("data: {value}\n\n")
}
fn decode(bytes: &[u8]) -> Result<Answer, GatewayError> {
    let mut decoder = Decoder::new();
    let answer = decoder.push(Bytes::copy_from_slice(bytes))?;
    decoder.finish()?;
    answer.ok_or_else(|| GatewayError::server_error("missing answer"))
}

#[test]
fn perplexity_web_rejects_unsupported_surface_instead_of_flattening_it() {
    let base = request();
    let mut cases = vec![];
    let mut r = base.clone();
    r.messages.push(r.messages[0].clone());
    cases.push(r);
    let mut r = base.clone();
    r.messages[0].role = MessageRole::System;
    cases.push(r);
    let mut r = base.clone();
    r.messages[0].name = Some("named".into());
    cases.push(r);
    let mut r = base.clone();
    r.messages[0].content.push(ContentPart::Text {
        text: "second".into(),
    });
    cases.push(r);
    let mut r = base.clone();
    r.messages[0].content = vec![ContentPart::ImageUrl {
        image_url: "https://example.test/image".into(),
        detail: None,
    }];
    cases.push(r);
    let mut r = base.clone();
    r.requested_model = Some("sonar".into());
    cases.push(r);
    let mut r = base.clone();
    r.extra.insert("mode".into(), json!("pro"));
    cases.push(r);
    let mut r = base.clone();
    r.previous_response_id = Some("other-account-context".into());
    cases.push(r);
    let mut r = base.clone();
    r.explicit_session_key = Some("session".into());
    cases.push(r);
    let mut r = base.clone();
    r.tool_choice = Some(json!("auto"));
    cases.push(r);
    let mut r = base.clone();
    r.reasoning = Some(json!({"effort":"high"}));
    cases.push(r);
    let mut r = base.clone();
    r.endpoint_kind = EndpointKind::Search;
    cases.push(r);
    let mut r = base.clone();
    r.raw_body = json!({"temperature":0.2});
    cases.push(r);
    for r in cases {
        assert_eq!(
            pack_single_turn(&r).unwrap_err().code.as_deref(),
            Some("perplexity_web_unsupported_request")
        );
    }
    for value in [" ".into(), "x".repeat(64 * 1024 + 1)] {
        let mut r = base.clone();
        r.messages[0].content = vec![ContentPart::Text { text: value }];
        assert!(pack_single_turn(&r).is_err());
    }
}

#[test]
fn perplexity_web_chunk_boundaries_preserve_utf8_crlf_bom_and_multiline_json() {
    let wire = format!("\u{feff}: comment\r\nevent: message\r\ndata: {{\r\ndata: \"blocks\":[{{\"intended_usage\":\"ask_text\",\"markdown_block\":{{\"answer\":\"你好🌍\"}}}}]\r\ndata: }}\r\n\r\nevent: end_of_stream\r\n\r\n");
    for split in 0..=wire.len() {
        let mut decoder = Decoder::new();
        let first = decoder
            .push(Bytes::copy_from_slice(&wire.as_bytes()[..split]))
            .unwrap();
        let result = if first.is_some() {
            first
        } else {
            decoder
                .push(Bytes::copy_from_slice(&wire.as_bytes()[split..]))
                .unwrap()
        };
        assert_eq!(result.unwrap().text, "你好🌍", "split {split}");
    }
}

#[test]
fn perplexity_web_terminal_requires_last_event_answer_not_historical_partial() {
    for tail in [
        "".into(),
        "event: end_of_stream\n\n".into(),
        event(json!({"final_sse_message":true})),
        format!(
            "{}event: end_of_stream\n\n",
            event(json!({"final_sse_message":true}))
        ),
        format!("{}event: end_of_stream\n\n", event(snapshot(" "))),
    ] {
        let wire = if tail == "event: end_of_stream\n\n" {
            tail
        } else {
            format!("{}{tail}", event(snapshot("partial")))
        };
        assert!(decode(wire.as_bytes()).is_err(), "must reject {wire}");
    }
}

#[test]
fn perplexity_web_does_not_treat_unknown_markdown_patch_or_plain_text_as_answer() {
    for value in [
        json!({"answer":"not-wire-answer"}),
        json!({"blocks":[{"intended_usage":"card","markdown_block":{"answer":"not-answer"}}]}),
        json!({"blocks":[{"intended_usage":"ask_text_0_markdown","markdown_block":{"answer":"different-profile"}}]}),
        json!({"blocks":[{"intended_usage":"ask_text","diff_block":{"patches":[{"value":"unsafe-append"}]}}]}),
        json!({"text":"plain text not nested JSON"}),
        json!({"text":[{"step_type":"FINAL","content":{"answer":"plain"}}]}),
        json!({"blocks":[{"intended_usage":"ask_text","markdown_block":{"answer":"one"}},{"intended_usage":"ask_text","markdown_block":{"answer":"two"}}]}),
    ] {
        let wire = format!("{}event: end_of_stream\n\n", event(value));
        assert!(decode(wire.as_bytes()).is_err());
    }
}

#[test]
fn perplexity_web_sources_are_separate_bounded_and_do_not_echo_private_fields() {
    let raw = json!({"blocks":[
        {"intended_usage":"web_results","web_result_block":{"web_results":[{"name":"来源","url":"https://example.test/a","snippet":"摘要","private_field":"secret"}]}},
        {"intended_usage":"ask_text","markdown_block":{"answer":"结果 [1]"}}
    ],"read_write_token":"session-secret","backend_uuid":"context-secret"});
    let answer = decode(format!("{}event: end_of_stream\n\n", event(raw)).as_bytes()).unwrap();
    assert_eq!(answer.text, "结果 [1]");
    assert_eq!(
        answer.sources,
        vec![Source {
            name: "来源".into(),
            url: "https://example.test/a".into(),
            snippet: "摘要".into()
        }]
    );
    assert!(!format!("{answer:?}").contains("secret"));
    for url in [
        "javascript:alert(1)",
        "https://user:password@example.test/",
        "",
    ] {
        let value = json!({"blocks":[{"intended_usage":"web_results","web_result_block":{"web_results":[{"url":url}]}}]});
        assert!(decode(event(value).as_bytes()).is_err());
    }
}

#[test]
fn perplexity_web_limits_bytes_frames_answers_blocks_sources_and_events() {
    for wire in [
        vec![b'x'; BODY_LIMIT + 1],
        vec![b'x'; 512 * 1024 + 1],
        b"data: \xff\n\n".to_vec(),
        b"data: {bad}\n\n".to_vec(),
        b"data: {}\n".to_vec(),
    ] {
        assert!(decode(&wire).is_err());
    }
    let too_many_sources = json!({"blocks":[{"intended_usage":"web_results","web_result_block":{"web_results":vec![json!({"url":"https://example.test/"});65]}}]});
    for value in [
        snapshot(&"x".repeat(256 * 1024 + 1)),
        json!({"blocks":vec![json!({});129]}),
        too_many_sources,
    ] {
        assert!(decode(event(value).as_bytes()).is_err());
    }
    let mut decoder = Decoder::new();
    for _ in 0..4096 {
        assert!(decoder
            .push(Bytes::from_static(b"data: {}\n\n"))
            .unwrap()
            .is_none());
    }
    assert_eq!(
        decoder
            .push(Bytes::from_static(b"data: {}\n\n"))
            .unwrap_err()
            .code
            .as_deref(),
        Some("perplexity_web_event_limit")
    );
}
