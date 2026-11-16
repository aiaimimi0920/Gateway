use super::{fixture::*, inspection::inspect_at, McpError, VERSION};
use serde_json::{json, Value};
use std::time::Duration;

fn init() -> Reply {
    let mut reply = Reply::json(json!({"jsonrpc":"2.0","id":1,"result":{
        "protocolVersion":VERSION,"capabilities":{"tools":{}},"instructions":"do not execute server prose"}}));
    reply.session = Some("private-session");
    reply
}
fn tool(name: &str) -> Value {
    json!({"name":name,"inputSchema":{"type":"object","properties":{}}})
}
fn page(id: u64, tools: Vec<Value>) -> Reply {
    Reply::json(json!({"jsonrpc":"2.0","id":id,"result":{"tools":tools}}))
}
async fn inspect(peer: &Peer) -> Result<Vec<super::ToolDefinition>, McpError> {
    inspect_at(
        &http(),
        &peer.endpoint,
        "synthetic-access",
        None,
        Duration::from_secs(3),
    )
    .await
}

#[tokio::test]
async fn luma_mcp_inspection_initializes_and_paginates_json_and_sse_without_eof() {
    let first = Reply::json(
        json!({"jsonrpc":"2.0","id":2,"result":{"tools":[tool("luma_model_list")],"nextCursor":"cursor-1"}}),
    );
    let mut second = page(3, vec![tool("luma_generate_image")]);
    second.mime = "text/event-stream";
    second.body = format!("data: {}\n\n", second.body);
    second.hold = true;
    let peer = Peer::new(vec![init(), Reply::initialized(), first, second]).await;
    let catalog = tokio::time::timeout(Duration::from_secs(2), inspect(&peer))
        .await
        .unwrap()
        .unwrap();
    assert_eq!(catalog.len(), 2);
    assert_eq!(catalog[1].name, "luma_generate_image");
    let serialized = serde_json::to_string(&catalog).unwrap();
    assert!(!serialized.contains("synthetic-access") && !serialized.contains("private-session"));
    {
        let calls = peer.calls.lock().unwrap();
        assert_eq!(calls.len(), 4);
        assert_eq!(
            calls
                .iter()
                .map(|(_, v)| v["method"].as_str().unwrap())
                .collect::<Vec<_>>(),
            [
                "initialize",
                "notifications/initialized",
                "tools/list",
                "tools/list"
            ]
        );
        assert!(calls[0].0.get("mcp-session-id").is_none());
        assert_eq!(calls[0].1["params"]["capabilities"], json!({}));
        for (index, (headers, _)) in calls.iter().enumerate() {
            assert_eq!(headers["authorization"], "Bearer synthetic-access");
            assert!(!headers.contains_key("cookie") && !headers.contains_key("x-api-key"));
            if index > 0 {
                assert_eq!(headers["mcp-session-id"], "private-session");
                assert_eq!(headers["mcp-protocol-version"], VERSION);
            }
        }
        assert_eq!(calls[3].1["params"]["cursor"], "cursor-1");
    }
    peer.wait_drops(4).await;
    peer.stop().await;
}

#[tokio::test]
async fn luma_mcp_inspection_rejects_unsupported_version_and_capability() {
    for result in [
        json!({"protocolVersion":"unknown","capabilities":{"tools":{}}}),
        json!({"protocolVersion":VERSION,"capabilities":{}}),
    ] {
        let peer = Peer::new(vec![Reply::json(
            json!({"jsonrpc":"2.0","id":1,"result":result}),
        )])
        .await;
        assert_eq!(inspect(&peer).await.unwrap_err(), McpError::Unsupported);
        assert_eq!(peer.count(), 1);
        peer.stop().await;
    }
}

#[tokio::test]
async fn luma_mcp_inspection_rejects_session_switch_and_bad_notification_ack() {
    let mut switched = page(2, vec![]);
    switched.session = Some("different-private-session");
    let peer = Peer::new(vec![init(), Reply::initialized(), switched]).await;
    assert_eq!(inspect(&peer).await.unwrap_err(), McpError::InvalidResponse);
    peer.stop().await;
    for ack in [
        Reply::json(json!({})),
        Reply {
            body: "private".into(),
            ..Reply::initialized()
        },
    ] {
        let peer = Peer::new(vec![init(), ack]).await;
        assert_eq!(inspect(&peer).await.unwrap_err(), McpError::InvalidResponse);
        assert_eq!(peer.count(), 2);
        peer.stop().await;
    }
}

#[tokio::test]
async fn luma_mcp_inspection_rejects_partial_catalog_duplicate_names_and_cursor_loops() {
    for second in [
        page(3, vec![tool("same")]),
        Reply::json(json!({"jsonrpc":"2.0","id":3,
        "result":{"tools":[],"nextCursor":"repeat"}})),
        Reply::json(json!({"jsonrpc":"2.0","id":3,"error":{"message":"private-error"}})),
    ] {
        let first = Reply::json(
            json!({"jsonrpc":"2.0","id":2,"result":{"tools":[tool("same")],"nextCursor":"repeat"}}),
        );
        let peer = Peer::new(vec![init(), Reply::initialized(), first, second]).await;
        let error = inspect(&peer).await.unwrap_err();
        assert!(!error.to_string().contains("private-error"));
        assert_eq!(peer.count(), 4);
        peer.stop().await;
    }
}

#[tokio::test]
async fn luma_mcp_inspection_rejects_redirect_html_and_http_errors_without_retry() {
    let destination = Peer::new(vec![]).await;
    for status in [302, 307, 401, 429, 503] {
        let mut reply = init();
        reply.status = status;
        reply.location = Some(destination.endpoint.clone());
        let peer = Peer::new(vec![reply]).await;
        assert_eq!(
            inspect(&peer).await.unwrap_err(),
            if status < 400 {
                McpError::Redirect
            } else {
                McpError::Rejected
            }
        );
        assert_eq!(peer.count(), 1);
        assert_eq!(destination.count(), 0);
        peer.stop().await;
    }
    let mut html = init();
    html.mime = "text/html";
    let peer = Peer::new(vec![html]).await;
    assert_eq!(inspect(&peer).await.unwrap_err(), McpError::InvalidResponse);
    peer.stop().await;
    destination.stop().await;
}

#[tokio::test]
async fn luma_mcp_inspection_deadline_cancellation_and_expiry_are_not_success() {
    let mut stalled = init();
    stalled.mime = "text/event-stream";
    stalled.body = String::new();
    stalled.hold = true;
    let peer = Peer::new(vec![stalled]).await;
    let error = inspect_at(
        &http(),
        &peer.endpoint,
        "token",
        None,
        Duration::from_millis(100),
    )
    .await
    .unwrap_err();
    assert!(matches!(error, McpError::Timeout | McpError::Transport));
    peer.wait_drops(1).await;
    peer.stop().await;
    let mut stalled = init();
    stalled.mime = "text/event-stream";
    stalled.body = String::new();
    stalled.hold = true;
    let peer = Peer::new(vec![stalled]).await;
    let endpoint = peer.endpoint.clone();
    let task = tokio::spawn(async move {
        inspect_at(&http(), &endpoint, "token", None, Duration::from_secs(30)).await
    });
    peer.wait_calls(1).await;
    tokio::time::sleep(Duration::from_millis(30)).await;
    task.abort();
    assert!(task.await.unwrap_err().is_cancelled());
    peer.wait_drops(1).await;
    peer.stop().await;
    let peer = Peer::new(vec![]).await;
    assert_eq!(
        inspect_at(
            &http(),
            &peer.endpoint,
            "token",
            Some(time::OffsetDateTime::now_utc() - time::Duration::seconds(1)),
            Duration::from_secs(1)
        )
        .await
        .unwrap_err(),
        McpError::Expired
    );
    assert_eq!(peer.count(), 0);
    peer.stop().await;
}

#[tokio::test]
async fn luma_mcp_inspection_enforces_body_tool_and_page_budgets() {
    let mut oversized = init();
    oversized.body = "x".repeat(2 * 1024 * 1024 + 1);
    let peer = Peer::new(vec![oversized]).await;
    assert_eq!(inspect(&peer).await.unwrap_err(), McpError::Limit);
    peer.stop().await;
    let peer = Peer::new(vec![
        init(),
        Reply::initialized(),
        page(2, (0..257).map(|i| tool(&format!("tool_{i}"))).collect()),
    ])
    .await;
    assert_eq!(inspect(&peer).await.unwrap_err(), McpError::Limit);
    peer.stop().await;
    let mut replies = vec![init(), Reply::initialized()];
    for id in 2..=9 {
        replies.push(Reply::json(json!({"jsonrpc":"2.0","id":id,"result":{"tools":[],"nextCursor":format!("cursor_{id}")}})));
    }
    let peer = Peer::new(replies).await;
    assert_eq!(inspect(&peer).await.unwrap_err(), McpError::Limit);
    assert_eq!(peer.count(), 10);
    peer.stop().await;
}

#[tokio::test]
async fn luma_mcp_stateless_inspection_does_not_inherit_session() {
    let mut initialize = init();
    initialize.session = None;
    let peer = Peer::new(vec![initialize, Reply::initialized(), page(2, vec![])]).await;
    assert!(inspect(&peer).await.unwrap().is_empty());
    assert!(peer
        .calls
        .lock()
        .unwrap()
        .iter()
        .all(|(headers, _)| { !headers.contains_key("mcp-session-id") }));
    peer.stop().await;
}

#[tokio::test]
async fn luma_mcp_inspection_bounds_cumulative_response_bytes() {
    let mut replies = vec![init(), Reply::initialized()];
    for id in 2..=4 {
        let mut item = tool(&format!("tool_{id}"));
        item["inputSchema"]["description"] = Value::String("x".repeat(1500 * 1024));
        replies.push(Reply::json(json!({"jsonrpc":"2.0","id":id,
            "result":{"tools":[item],"nextCursor":format!("cursor_{id}")}})));
    }
    let peer = Peer::new(replies).await;
    assert_eq!(inspect(&peer).await.unwrap_err(), McpError::Limit);
    assert_eq!(peer.count(), 5);
    peer.stop().await;
}
