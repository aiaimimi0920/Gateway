use crate::{
    protocol::{anthropic, openai, responses},
    upstream::stream::{snapshot_tapped_usage, tap_sse_usage_with_error},
};
use futures::StreamExt;
use serde_json::json;

#[test]
fn incomplete_buffered_usage_remains_unknown_in_all_supported_dialects() {
    for usage in [
        json!({}),
        json!({"prompt_tokens":1}),
        json!({"input_tokens":1}),
        json!({"prompt_tokens":-1,"completion_tokens":1}),
        json!(null),
    ] {
        let openai = json!({"choices":[{"message":{"content":"ok"}}],"usage":usage});
        assert!(openai::unpack_openai_response(&openai)
            .unwrap()
            .usage
            .is_none());
        let anthropic = json!({"content":[{"type":"text","text":"ok"}],"usage":usage});
        assert!(anthropic::unpack_anthropic_response(&anthropic)
            .unwrap()
            .usage
            .is_none());
        let responses = json!({"output":[],"usage":usage});
        assert!(responses::unpack_responses_response(&responses)
            .unwrap()
            .usage
            .is_none());
    }
    let explicit_zero = json!({"choices":[{"message":{"content":""}}],
        "usage":{"prompt_tokens":0,"completion_tokens":0}});
    assert_eq!(
        openai::unpack_openai_response(&explicit_zero)
            .unwrap()
            .usage
            .unwrap()
            .total_tokens,
        0
    );
}

#[tokio::test]
async fn provisional_stream_usage_is_not_a_final_zero_output_bill() {
    let start = bytes::Bytes::from_static(b"event: message_start\ndata: {\"message\":{\"usage\":{\"input_tokens\":100,\"output_tokens\":0}}}\n\n");
    let end = bytes::Bytes::from_static(
        b"event: message_delta\ndata: {\"usage\":{\"output_tokens\":50}}\n\n",
    );
    let (stream, capture) = tap_sse_usage_with_error(futures::stream::iter([
        Ok::<_, std::io::Error>(start),
        Ok(end),
    ]));
    let mut stream = Box::pin(stream);
    assert!(stream.next().await.is_some());
    assert!(snapshot_tapped_usage(&capture).is_none());
    assert!(stream.next().await.is_some());
    let usage = snapshot_tapped_usage(&capture).unwrap();
    assert_eq!((usage.prompt_tokens, usage.completion_tokens), (100, 50));
}

#[test]
fn cash_tariff_never_falls_back_to_display_registry_or_incomplete_model_prices() {
    let empty = crate::db::operator::cash_model_rate(&json!({}), "gpt-5.4");
    assert!(!empty.configured);
    let configured = crate::db::operator::cash_model_rate(
        &json!({
            "staticInputMicrosPer1kTokens":100,"staticOutputMicrosPer1kTokens":200,
            "modelPricing":{"gpt-5.4":{"staticInputMicrosPer1kTokens":300}}
        }),
        "gpt-5.4",
    );
    assert_eq!(configured.prompt_micros_per_1k_tokens, Some(300));
    assert_eq!(configured.completion_micros_per_1k_tokens, None);
}

#[test]
fn corrupt_persisted_cash_values_fail_closed() {
    let valid = json!({"balance":{"currency":"USD","totalMicros":100,"spentMicros":0,
        "reservedMicros":0,"pendingRequests":0},"recorded_requests":0});
    assert!(crate::cash_billing::ledger::decode_account(&valid.to_string()).is_ok());
    for (field, value) in [
        ("currency", json!("CNY")),
        ("spentMicros", json!(-1)),
        ("totalMicros", json!(crate::cash_billing::MAX_MICROS + 1)),
        ("reservedMicros", json!(1)),
        ("pendingRequests", json!(1025)),
    ] {
        let mut invalid = valid.clone();
        invalid["balance"][field] = value;
        assert!(
            crate::cash_billing::ledger::decode_account(&invalid.to_string()).is_err(),
            "{field}"
        );
    }
}
