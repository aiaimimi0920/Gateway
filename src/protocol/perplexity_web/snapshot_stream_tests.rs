//! Frame-by-frame replacement observations retain the original strict completion rule.
use super::*;
use bytes::Bytes;
use serde_json::json;
fn frame(answer: &str) -> String {
    format!(
        "data: {}\n\n",
        json!({"blocks":[{"intended_usage":"ask_text","markdown_block":{"answer":answer}}]})
    )
}
#[test]
fn perplexity_web_snapshot_updates_are_ordered_replacements_and_clear_not_text_deltas() {
    let mut decoder = Decoder::new();
    decoder
        .push_chunk(Bytes::from(format!(
            "{}{}data: {{}}\n\n{}event: end_of_stream\n\n",
            frame("旧答案"),
            frame("改"),
            frame("最终")
        )))
        .unwrap();
    for expected in [Some("旧答案"), Some("改"), None, Some("最终")] {
        let Some(Update::Snapshot(answer)) = decoder.next_event().unwrap() else {
            panic!("Expected one frame")
        };
        assert_eq!(answer.as_ref().map(|v| v.text.as_str()), expected);
    }
    let Some(Update::Complete(answer)) = decoder.next_event().unwrap() else {
        panic!("Expected strict completion")
    };
    assert_eq!(answer.text, "最终");
}
#[test]
fn perplexity_web_snapshot_later_bad_frame_does_not_preconsume_earlier_observation() {
    let mut decoder = Decoder::new();
    decoder
        .push_chunk(Bytes::from(format!("{}data: {{bad}}\n\n", frame("临时"))))
        .unwrap();
    assert!(matches!(
        decoder.next_event().unwrap(),
        Some(Update::Snapshot(Some(_)))
    ));
    assert_eq!(
        decoder.next_event().unwrap_err().code.as_deref(),
        Some("perplexity_web_invalid_json")
    );
}
#[test]
fn perplexity_web_snapshot_carrier_has_exact_shape_order_and_no_usage_or_native_metadata() {
    let answer = Answer {
        text: "当前".into(),
        sources: vec![],
    };
    let good = json!({public_snapshot::KEY:public_snapshot::value(1,Some(&answer)),"choices":[]});
    assert!(public_snapshot::Relay::default()
        .observe(&good)
        .unwrap()
        .is_some());
    for bad in [
        json!({public_snapshot::KEY:{"version":1,"sequence":2,"answer":null},"choices":[]}),
        json!({public_snapshot::KEY:{"version":1,"sequence":1,"answer":null,"read_write_token":"private"},"choices":[]}),
        json!({public_snapshot::KEY:{"version":1,"sequence":1,"answer":null},"choices":[{"delta":{"content":"not-provisional"}}]}),
        json!({public_snapshot::KEY:{"version":1,"sequence":1,"answer":null},"choices":[],"usage":null}),
    ] {
        assert!(public_snapshot::Relay::default().observe(&bad).is_err());
    }
}
