//! Pure-Rust unit tests for Cursor ProtoJSON, Protobuf, Connect stream, and MCP codecs.
#[path = "transport_tests.rs"]
mod transport_tests;

use super::{
    connect_stream::TextDecoderState,
    json_wire::{unique_json, wire_fields},
    protobuf::{
        connect_frame, decode_struct_map, encode_value, field_bytes, field_str, field_varint,
        fields,
    },
    qualification::encode_workos_cookie,
    tool_stream::ToolDecoderState,
    ToolSpec,
};
use serde_json::json;

#[test]
fn strict_json_and_protojson_reject_duplicates_non_canonical_ints_and_null_scalars() {
    assert!(unique_json(br#"{"a":1,"b":12.5}"#).is_ok());
    for bad in [
        &br#"{"a":1,"a":2}"#[..],
        &br#"{"a":1,"\u0061":2}"#[..],
        &br#"{"a":1.0}"#[..],
        &br#"{"a":1e3}"#[..],
        &br#"{"a":9007199254740993}"#[..],
    ] {
        assert!(unique_json(bad).is_err());
    }
    let valid = unique_json(br#"{"has_credit_grants":false,"totalCents":"0"}"#)
        .unwrap()
        .as_object()
        .unwrap()
        .clone();
    let mapped = wire_fields(&valid, &["hasCreditGrants", "totalCents"]).unwrap();
    assert_eq!(mapped.len(), 2);

    let dup = unique_json(br#"{"totalCents":0,"total_cents":0}"#)
        .unwrap()
        .as_object()
        .unwrap()
        .clone();
    assert_eq!(
        wire_fields(&dup, &["totalCents"]).unwrap_err().code,
        "unknown_or_duplicate_protojson_field"
    );
    let nul = unique_json(br#"{"totalCents":null}"#)
        .unwrap()
        .as_object()
        .unwrap()
        .clone();
    assert_eq!(
        wire_fields(&nul, &["totalCents"]).unwrap_err().code,
        "null_protojson_field"
    );
}

#[test]
fn protobuf_struct_value_round_trip_formats_safe_integers_without_decimal_suffix() {
    let original = json!({
        "count": 3,
        "zero": 0,
        "ratio": 1.25,
        "flag": true,
        "empty": null,
        "items": ["中文", {"nested": -42}]
    });
    let mut action = Vec::new();
    for (k, v) in original.as_object().unwrap() {
        let mut pair = field_str(1, k).unwrap();
        pair.extend_from_slice(&field_bytes(2, &encode_value(v, 0).unwrap()).unwrap());
        action.extend_from_slice(&field_bytes(2, &pair).unwrap());
    }
    let decoded = decode_struct_map(&action, 2, 0).unwrap();
    let rendered = serde_json::to_string(&decoded).unwrap();
    assert!(rendered.contains(r#""count":3"#), "{rendered}");
    assert!(!rendered.contains("3.0"), "{rendered}");
    assert_eq!(serde_json::Value::Object(decoded), original);
}

#[test]
fn workos_cookie_matches_encode_uri_component_rules() {
    assert_eq!(
        encode_workos_cookie("user_01.ab-c", "a.b_c-d"),
        "WorkosCursorSessionToken=user_01.ab-c%3A%3Aa.b_c-d"
    );
}

#[test]
fn text_decoder_handles_context_kv_blobs_thinking_notices_and_end_stream() {
    let mut decoder = TextDecoderState::new_text().unwrap();

    // ExecServerMessage tag 2 -> request_context_args tag 10
    let mut ctx_req = field_varint(1, 7).unwrap();
    ctx_req.extend_from_slice(&field_str(15, "exec-1").unwrap());
    ctx_req.extend_from_slice(&field_bytes(10, &field_varint(7, 1).unwrap()).unwrap());
    let ctx_frame = connect_frame(&field_bytes(2, &ctx_req).unwrap(), 0);

    // KvServerMessage tag 4 -> set_blob (tag 3) then get_blob (tag 2)
    let blob_key = [0xABu8; 32];
    let mut set_inner = field_bytes(1, &blob_key).unwrap();
    set_inner.extend_from_slice(&field_bytes(2, b"blob-payload").unwrap());
    let mut kv_set = field_varint(1, 9).unwrap();
    kv_set.extend_from_slice(&field_bytes(3, &set_inner).unwrap());
    let kv_set_frame = connect_frame(&field_bytes(4, &kv_set).unwrap(), 0);

    let get_inner = field_bytes(1, &blob_key).unwrap();
    let mut kv_get = field_varint(1, 10).unwrap();
    kv_get.extend_from_slice(&field_bytes(2, &get_inner).unwrap());
    let kv_get_frame = connect_frame(&field_bytes(4, &kv_get).unwrap(), 0);

    // Thinking delta (tag 4), server notice (tag 1 w/ tag 2 = 1), real text delta (tag 1), TurnEnded (tag 14)
    let thinking = connect_frame(
        &field_bytes(
            1,
            &field_bytes(4, &field_str(1, "思考内容").unwrap()).unwrap(),
        )
        .unwrap(),
        0,
    );
    let mut notice_inner = field_str(1, "ignored notice").unwrap();
    notice_inner.extend_from_slice(&field_varint(2, 1).unwrap());
    let notice = connect_frame(
        &field_bytes(1, &field_bytes(1, &notice_inner).unwrap()).unwrap(),
        0,
    );
    let text = connect_frame(
        &field_bytes(
            1,
            &field_bytes(1, &field_str(1, "真实回答").unwrap()).unwrap(),
        )
        .unwrap(),
        0,
    );
    let turn_end = connect_frame(
        &field_bytes(1, &field_bytes(14, &field_varint(1, 0).unwrap()).unwrap()).unwrap(),
        0,
    );
    let end_stream = connect_frame(b"{}", 2);

    let mut all = Vec::new();
    for part in [
        ctx_frame,
        kv_set_frame,
        kv_get_frame,
        thinking,
        notice,
        text,
        turn_end,
        end_stream,
    ] {
        all.extend_from_slice(&part);
    }
    // Feed in two parts so replies are drained before EndStream arrives!
    let split = all.len() - 7;
    decoder.push(&all[..split]).unwrap();
    let replies = decoder.take_replies().unwrap();
    assert_eq!(replies.len(), 3);
    decoder.push(&all[split..]).unwrap();
    assert!(decoder.take_replies().unwrap().is_empty());

    let out = decoder.finish("default".into()).unwrap();
    assert_eq!(out.text, "真实回答");
    assert_eq!(out.thinking.as_deref(), Some("思考内容"));
}

#[test]
fn tool_decoder_handles_state_permission_mcp_args_and_continuation_frame() {
    let specs = vec![ToolSpec {
        name: "echo".into(),
        description: "Echo input".into(),
        parameters: json!({"type":"object","properties":{"n":{"type":"integer"}}}),
    }];
    let mut decoder = ToolDecoderState::new(&specs, "default".into()).unwrap();

    // 1) Action 36: MCP state request
    let mut state_exec = field_varint(1, 1).unwrap();
    state_exec.extend_from_slice(&field_bytes(36, &field_str(1, "gateway").unwrap()).unwrap());
    decoder
        .push(&connect_frame(&field_bytes(2, &state_exec).unwrap(), 0))
        .unwrap();
    assert_eq!(decoder.take_replies().unwrap().len(), 1);

    // 2) Action 42: MCP permission check
    let mut perm_args = field_str(1, "gateway").unwrap();
    perm_args.extend_from_slice(&field_str(2, "echo").unwrap());
    let mut perm_exec = field_varint(1, 2).unwrap();
    perm_exec.extend_from_slice(&field_bytes(42, &perm_args).unwrap());
    decoder
        .push(&connect_frame(&field_bytes(2, &perm_exec).unwrap(), 0))
        .unwrap();
    assert_eq!(decoder.take_replies().unwrap().len(), 1);

    // 3) Prelude text + Action 11 mcp_args + next-phase text in the SAME chunk
    let prelude = connect_frame(
        &field_bytes(
            1,
            &field_bytes(1, &field_str(1, "before tool").unwrap()).unwrap(),
        )
        .unwrap(),
        0,
    );
    let mut arg_pair = field_str(1, "n").unwrap();
    arg_pair.extend_from_slice(&field_bytes(2, &encode_value(&json!(7), 0).unwrap()).unwrap());
    let mut mcp_args = field_str(1, "mcp_gateway_echo").unwrap();
    mcp_args.extend_from_slice(&field_bytes(2, &arg_pair).unwrap());
    mcp_args.extend_from_slice(&field_str(3, "call_1").unwrap());
    mcp_args.extend_from_slice(&field_str(4, "gateway").unwrap());
    mcp_args.extend_from_slice(&field_str(5, "echo").unwrap());
    let mut call_exec = field_varint(1, 3).unwrap();
    call_exec.extend_from_slice(&field_str(15, "exec-uuid").unwrap());
    call_exec.extend_from_slice(&field_bytes(11, &mcp_args).unwrap());
    let call_frame = connect_frame(&field_bytes(2, &call_exec).unwrap(), 0);
    let after = connect_frame(
        &field_bytes(
            1,
            &field_bytes(1, &field_str(1, "after tool").unwrap()).unwrap(),
        )
        .unwrap(),
        0,
    );

    let mut combined = prelude;
    combined.extend_from_slice(&call_frame);
    combined.extend_from_slice(&after);
    decoder.push(&combined).unwrap();

    let calls = decoder.take_calls();
    assert_eq!(calls.len(), 1);
    assert_eq!(calls[0].call_id, "call_1");
    assert_eq!(calls[0].text, "before tool");
    assert_eq!(calls[0].arguments, r#"{"n":7}"#);

    // Complete tool & finish next phase
    let cont_frame = decoder.complete("call_1", "tool_ok").unwrap();
    assert_eq!(cont_frame[0], 0);
    assert!(fields(&cont_frame[5..]).is_ok());

    let mut tail = connect_frame(&field_bytes(1, &field_bytes(14, &[]).unwrap()).unwrap(), 0);
    tail.extend_from_slice(&connect_frame(b"{}", 2));
    decoder.push(&tail).unwrap();
    let final_out = decoder.finish().unwrap();
    assert_eq!(final_out.text, "after tool");
}
