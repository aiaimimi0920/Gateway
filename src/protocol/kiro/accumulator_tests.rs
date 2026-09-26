//! Real HTTP response characterization for the nonstream accumulator boundary.
use std::collections::HashMap;
use std::time::Duration;

use serde_json::json;
use tokio::io::{AsyncReadExt, AsyncWriteExt};

use super::accumulator_content::AccumulationLimits;
use super::event_stream_tests::{event_frame, provider_error_frame, request};
use super::{accumulate_kiro_stream, estimate_tokens, pack_kiro, prompt_tokens_from_context};
use crate::error::GatewayError;
use crate::protocol::canonical::{CanonicalRelayRequest, CanonicalRelayResponse, CanonicalTool};

struct ServerTask(tokio::task::JoinHandle<()>);

impl Drop for ServerTask {
    fn drop(&mut self) {
        self.0.abort();
    }
}

async fn accumulate(
    body: Vec<u8>,
    req: &CanonicalRelayRequest,
) -> Result<CanonicalRelayResponse, GatewayError> {
    run(body, req, None).await
}

pub(super) async fn accumulate_limited(
    body: Vec<u8>,
    limits: AccumulationLimits,
) -> Result<CanonicalRelayResponse, GatewayError> {
    run(body, &request(), Some(limits)).await
}

async fn run(
    body: Vec<u8>,
    req: &CanonicalRelayRequest,
    limits: Option<AccumulationLimits>,
) -> Result<CanonicalRelayResponse, GatewayError> {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let mut server = ServerTask(tokio::spawn(async move {
        tokio::time::timeout(Duration::from_secs(5), async move {
            let (mut socket, _) = listener.accept().await.unwrap();
            let mut headers = Vec::new();
            let mut buffer = [0u8; 1024];
            while !headers.windows(4).any(|window| window == b"\r\n\r\n") {
                let count = socket.read(&mut buffer).await.unwrap();
                assert!(count > 0 && headers.len() + count <= 8192);
                headers.extend_from_slice(&buffer[..count]);
            }
            let headers = format!("HTTP/1.1 200 OK\r\nContent-Type: application/vnd.amazon.eventstream\r\nContent-Length: {}\r\nConnection: close\r\n\r\n", body.len());
            socket.write_all(headers.as_bytes()).await.unwrap();
            socket.write_all(&body).await.unwrap();
            socket.shutdown().await.unwrap();
        }).await.unwrap();
    }));
    let response = tokio::time::timeout(
        Duration::from_secs(5),
        rquest::Client::new()
            .get(format!("http://{address}"))
            .send(),
    )
    .await
    .unwrap()
    .unwrap();
    let result = tokio::time::timeout(Duration::from_secs(5), async {
        match limits {
            Some(limits) => {
                super::accumulator::accumulate_with_limits(
                    response,
                    "claude-sonnet-4.6",
                    req,
                    limits,
                )
                .await
            }
            None => accumulate_kiro_stream(response, "claude-sonnet-4.6", req).await,
        }
    })
    .await
    .unwrap();
    (&mut server.0).await.unwrap();
    result
}

fn tool(id: &str, name: &str, input: &str) -> Vec<u8> {
    event_frame(
        "toolUseEvent",
        json!({"toolUseId":id,"name":name,"input":input,"stop":false}),
    )
}

#[tokio::test]
async fn aggregate_preserves_text_tool_order_restored_names_and_usage() {
    let mut req = request();
    let name = format!("tool_{}", "long".repeat(24));
    req.tools.push(CanonicalTool {
        tool_type: "function".into(),
        name: Some(name.clone()),
        description: None,
        input_schema: Some(json!({"type":"object"})),
        raw: HashMap::new(),
    });
    let packed = pack_kiro(&req, "claude-sonnet-4.6").unwrap();
    let short = packed["conversationState"]["currentMessage"]["userInputMessage"]
        ["userInputMessageContext"]["tools"][0]["toolSpecification"]["name"]
        .as_str()
        .unwrap();
    let body = vec![
        event_frame("assistantResponseEvent", json!({"content":"hello "})),
        tool("b", short, " {\"x\":"),
        tool("a", "empty", ""),
        tool("b", "ignored", "1} "),
        event_frame("assistantResponseEvent", json!({"content":"world"})),
        event_frame("contextUsageEvent", json!({"contextUsagePercentage":100.0})),
        event_frame("contextUsageEvent", json!({"contextUsagePercentage":5.0})),
    ]
    .concat();
    let response = accumulate(body, &req).await.unwrap();
    assert_eq!(response.text, "hello world");
    assert_eq!(response.model, "claude-sonnet-4.6");
    assert_eq!(response.upstream_status, Some(200));
    assert_eq!(response.finish_reason.as_deref(), Some("tool_calls"));
    assert_eq!(response.tool_calls.len(), 2);
    assert_eq!(response.tool_calls[0].id.as_deref(), Some("b"));
    assert_eq!(response.tool_calls[0].name.as_deref(), Some(name.as_str()));
    assert_eq!(
        response.tool_calls[0].arguments.as_deref(),
        Some("{\"x\":1}")
    );
    assert_eq!(response.tool_calls[1].id.as_deref(), Some("a"));
    assert_eq!(response.tool_calls[1].arguments.as_deref(), Some("{}"));
    let usage = response.usage.unwrap();
    assert_eq!(
        usage.prompt_tokens,
        prompt_tokens_from_context("claude-sonnet-4.6", 100.0)
    );
    assert_eq!(
        usage.completion_tokens,
        ["hello ", " {\"x\":", "", "1} ", "world"]
            .iter()
            .map(|value| estimate_tokens(value))
            .sum::<u64>()
    );
    assert_eq!(
        usage.total_tokens,
        usage.prompt_tokens + usage.completion_tokens
    );
}

#[tokio::test]
async fn empty_and_context_overflow_keep_nonstream_finish_contract() {
    let empty = accumulate(Vec::new(), &request()).await.unwrap();
    assert_eq!(empty.text, "");
    assert!(empty.tool_calls.is_empty());
    assert_eq!(empty.finish_reason.as_deref(), Some("stop"));
    let overflow = accumulate(
        event_frame("contextUsageEvent", json!({"contextUsagePercentage":100.0})),
        &request(),
    )
    .await
    .unwrap();
    assert_eq!(overflow.finish_reason.as_deref(), Some("length"));
}

#[tokio::test]
async fn provider_error_and_truncated_eof_reject_partial_success() {
    let valid = event_frame("assistantResponseEvent", json!({"content":"partial"}));
    let provider = accumulate([valid.clone(), provider_error_frame()].concat(), &request())
        .await
        .unwrap_err();
    assert!(provider.message.contains("provider failed"));
    let mut truncated = valid.clone();
    truncated.pop();
    let error = accumulate([valid, truncated].concat(), &request())
        .await
        .unwrap_err();
    assert!(error.message.contains("Truncated Kiro event-stream frame"));
}
