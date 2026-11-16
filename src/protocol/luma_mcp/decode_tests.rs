use super::{decode::Decoder, McpError};
use bytes::Bytes;
use serde_json::json;

#[test]
fn luma_mcp_sse_finishes_on_correlated_result_without_socket_eof() {
    let mut decoder = Decoder::new(7);
    let wire = format!(
        "data: {}\n\ndata: {}\n\n",
        json!({"jsonrpc":"2.0","method":"notifications/progress","params":{"progress":1}}),
        json!({"jsonrpc":"2.0","id":7,"result":{"tools":[]}})
    );
    let result = decoder.push(Bytes::from(wire)).unwrap().unwrap();
    assert_eq!(result, json!({"tools":[]}));
}

#[test]
fn luma_mcp_rejects_wrong_ids_duplicate_keys_errors_and_reverse_requests() {
    for raw in [
        r#"{"jsonrpc":"2.0","id":2,"result":{}}"#,
        r#"{"jsonrpc":"2.0","id":1,"id":1,"result":{}}"#,
        r#"{"jsonrpc":"2.0","id":1,"result":{},"error":{"message":"private"}}"#,
        r#"{"jsonrpc":"2.0","id":1,"method":"sampling/createMessage","params":{}}"#,
        r#"{"jsonrpc":"2.0","id":1,"result":null}"#,
    ] {
        let error = super::decode::json(raw.as_bytes(), 1).unwrap_err();
        assert!(!error.to_string().contains("private"));
    }
}

#[test]
fn luma_mcp_sse_partial_or_unknown_notifications_cannot_succeed() {
    let mut decoder = Decoder::new(1);
    assert!(decoder
        .push(Bytes::from_static(b"data: {\"jsonrpc\":\"2.0\",\"id\":1"))
        .unwrap()
        .is_none());
    assert_eq!(decoder.finish(), Err(McpError::InvalidResponse));
    let mut decoder = Decoder::new(1);
    assert!(decoder
        .push(Bytes::from_static(
            b"data: {\"jsonrpc\":\"2.0\",\"method\":\"unapproved/action\"}\n\n"
        ))
        .is_err());
}
