//! Malformed AWS frame/header/state inputs must fail closed without echoing private fields.
use super::fixtures::{event, frame, raw_frame, repair_crc, response, sequence};
use crc::{Crc, CRC_32_ISO_HDLC};
use neuro_gateway::upstream::bedrock_converse_official_api_common::accumulate_stream;
use serde_json::json;

async fn rejected(frames: Vec<Vec<u8>>) {
    let error = accumulate_stream(response(frames), "fixture")
        .await
        .unwrap_err();
    assert!(!error.message.contains("private-fixture-secret"));
    assert_eq!(
        error.provider_name.as_deref(),
        Some("bedrock_converse_compatible")
    );
}

#[tokio::test]
async fn bedrock_upstream_rejects_bad_discriminators_json_and_typed_headers() {
    for headers in [
        vec![],
        vec![(":event-type", "messageStart")],
        vec![
            (":message-type", "event"),
            (":event-type", "messageStart"),
            (":event-type", "private-fixture-secret"),
        ],
        vec![
            (":message-type", "event"),
            (":event-type", "messageStart"),
            (":content-type", "text/plain"),
        ],
        vec![
            (":message-type", "event"),
            (":event-type", "messageStart"),
            (":exception-type", "private-fixture-secret"),
        ],
    ] {
        rejected(vec![frame(&headers, &json!({"role":"assistant"}))]).await;
    }
    let mut typed = vec![13];
    typed.extend_from_slice(b":message-type");
    typed.push(0);
    rejected(vec![raw_frame(&typed, b"{}")]).await;
    rejected(vec![raw_frame(&[255, 1], b"{}")]).await;
    let mut invalid_json = event("messageStart", json!({"role":"assistant"}));
    let header_length = u32::from_be_bytes(invalid_json[4..8].try_into().unwrap()) as usize;
    invalid_json[12 + header_length] = 0xff;
    repair_crc(&mut invalid_json);
    rejected(vec![invalid_json]).await;
}

#[tokio::test]
async fn bedrock_upstream_all_header_value_types_are_bounded_and_optional_content_type_is_valid() {
    let mut first = frame(
        &[(":message-type", "event"), (":event-type", "messageStart")],
        &json!({"role":"assistant"}),
    );
    let end = 12 + u32::from_be_bytes(first[4..8].try_into().unwrap()) as usize;
    let mut headers = first[12..end].to_vec();
    for kind in 0u8..=9 {
        headers.extend_from_slice(&[1, b'a' + kind, kind]);
        match kind {
            0 | 1 => {}
            2 => headers.push(1),
            3 => headers.extend([0, 1]),
            4 => headers.extend([0; 4]),
            5 | 8 => headers.extend([0; 8]),
            6 | 7 => headers.extend([0, 1, b'x']),
            9 => headers.extend([0; 16]),
            _ => unreachable!(),
        }
    }
    first = raw_frame(&headers, b"{\"role\":\"assistant\"}");
    let mut frames = sequence();
    frames[0] = first;
    assert_eq!(
        accumulate_stream(response(frames), "fixture")
            .await
            .unwrap()
            .text,
        "你好"
    );
    headers.push(1);
    headers.push(b'z');
    headers.push(255);
    rejected(vec![raw_frame(&headers, b"{}")]).await;
}

#[tokio::test]
async fn bedrock_upstream_untrusted_lengths_fail_at_prelude_without_allocating_claimed_body() {
    for (total, headers) in [
        (0u32, 0u32),
        (15, 0),
        (u32::MAX, 0),
        (16, 1),
        (16 + 131073, 131073),
        (16 + 24 * 1024 * 1024 + 1, 0),
    ] {
        let mut prelude = Vec::new();
        prelude.extend_from_slice(&total.to_be_bytes());
        prelude.extend_from_slice(&headers.to_be_bytes());
        prelude.extend_from_slice(
            &Crc::<u32>::new(&CRC_32_ISO_HDLC)
                .checksum(&prelude)
                .to_be_bytes(),
        );
        rejected(vec![prelude]).await;
    }
}

#[tokio::test]
async fn bedrock_upstream_conflicting_block_identity_or_event_order_is_rejected() {
    let base = sequence();
    for (position, replacement) in [
        (0, event("messageStart", json!({"role":"user"}))),
        (
            1,
            event(
                "contentBlockDelta",
                json!({"contentBlockIndex":-1,"delta":{"text":"x"}}),
            ),
        ),
        (
            // Reasoning is now supported, but may not reuse an already closed text block.
            4,
            event(
                "contentBlockDelta",
                json!({"contentBlockIndex":0,"delta":{"reasoningContent":{"text":"private-fixture-secret"}}}),
            ),
        ),
        (
            1,
            event(
                "contentBlockDelta",
                json!({"contentBlockIndex":0,"delta":{"text":"x","toolUse":{"input":"{}"}}}),
            ),
        ),
        (
            2,
            event("contentBlockStop", json!({"contentBlockIndex":10})),
        ),
        (
            3,
            event(
                "contentBlockStart",
                json!({"contentBlockIndex":0,"start":{"toolUse":{"toolUseId":"x","name":"f"}}}),
            ),
        ),
        (
            4,
            event(
                "contentBlockDelta",
                json!({"contentBlockIndex":0,"delta":{"toolUse":{"input":"{}"}}}),
            ),
        ),
        (6, event("messageStop", json!({"stopReason":"end_turn"}))),
        (
            7,
            event(
                "metadata",
                json!({"usage":{"inputTokens":1,"outputTokens":1,"totalTokens":2}}),
            ),
        ),
        (
            7,
            event(
                "messageStop",
                json!({"stopReason":"unknownPrivateStopReason"}),
            ),
        ),
        (
            8,
            event(
                "metadata",
                json!({"usage":{"inputTokens":-1,"outputTokens":1,"totalTokens":2}}),
            ),
        ),
    ] {
        let mut frames = base.clone();
        frames[position] = replacement;
        rejected(frames).await;
    }
    let mut frames = base.clone();
    frames.insert(1, base[0].clone());
    rejected(frames).await;
    let mut frames = base.clone();
    frames.push(base[8].clone());
    rejected(frames).await;
    let mut frames = base;
    frames[5] = event(
        "contentBlockDelta",
        json!({"contentBlockIndex":1,"delta":{"toolUse":{"input":"{"}}}),
    );
    rejected(frames).await;
}

#[tokio::test]
async fn bedrock_upstream_stop_reason_mapping_does_not_hide_capacity_or_filtering() {
    for (reason, expected) in [
        ("end_turn", "stop"),
        ("stop_sequence", "stop"),
        ("max_tokens", "length"),
        ("model_context_window_exceeded", "length"),
        ("guardrail_intervened", "content_filter"),
        ("content_filtered", "content_filter"),
    ] {
        let mut frames = sequence();
        frames[7] = event("messageStop", json!({"stopReason":reason}));
        assert_eq!(
            accumulate_stream(response(frames), "fixture")
                .await
                .unwrap()
                .finish_reason
                .as_deref(),
            Some(expected)
        );
    }
}
