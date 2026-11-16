//! Native endpoint encoding must finish before the single pipeline success owner.
#[path = "pipeline_send_runtime/config.rs"]
mod config;
#[path = "pipeline_send_runtime/fixture.rs"]
mod fixture;
mod support;
#[path = "bedrock_native_pipeline_contract/termination.rs"]
mod termination;
#[path = "bedrock_native_outbound_contract/fixture.rs"]
mod wire;

use axum::http::StatusCode;
use futures::StreamExt;
use neuro_gateway::{
    pipeline::{stage_send, PipelineOutput},
    protocol::canonical::ProtocolFamily,
};
use tokio::time::timeout;

#[tokio::test]
async fn native_pipeline_emits_aws_wire_and_success_only_after_outer_eof() {
    let upstream = fixture::Upstream::start(StatusCode::OK, true, fixture::SSE_REPLY).await;
    let fixture = fixture::TestState::new();
    let selected = fixture::candidate(&upstream, "native-bedrock");
    let controller = fixture
        .state
        .concurrency_registry
        .get_or_create(&selected.provider_account_id);
    let mut ctx = fixture::context(true, vec![selected]);
    ctx.canonical_req.protocol_family = ProtocolFamily::BedrockConverse;
    let PipelineOutput::Sse(mut stream) =
        timeout(fixture::DEADLINE, stage_send::run(&mut ctx, &fixture.state))
            .await
            .unwrap()
            .unwrap()
    else {
        panic!("stream")
    };
    let mut chunks = Vec::new();
    timeout(fixture::DEADLINE, async {
        while let Some(chunk) = stream.next().await {
            chunks.push(chunk.unwrap());
            assert_eq!(controller.snapshot().consecutive_successes, 0);
        }
    })
    .await
    .unwrap();
    let success = controller.snapshot().consecutive_successes;
    drop(stream);
    assert_eq!(controller.snapshot().active_count, 0);
    assert_eq!(upstream.requests().len(), 1);
    upstream.finish().await;
    fixture.finish().await;
    assert_eq!(success, 1);
    let events: Vec<_> = chunks.iter().map(wire::decode).collect();
    assert_eq!(events.first().unwrap().0, "messageStart");
    assert_eq!(events.last().unwrap().0, "metadata");
    assert_eq!(events.last().unwrap().1["usage"]["totalTokens"], 10);
}

#[tokio::test]
async fn native_pipeline_missing_finish_cannot_be_counted_as_inner_success() {
    let partial = "data: {\"choices\":[{\"delta\":{\"content\":\"partial\"}}]}\n\n";
    let upstream = fixture::Upstream::start(StatusCode::OK, true, partial).await;
    let fixture = fixture::TestState::new();
    let selected = fixture::candidate(&upstream, "native-bedrock-invalid");
    let controller = fixture
        .state
        .concurrency_registry
        .get_or_create(&selected.provider_account_id);
    let mut ctx = fixture::context(true, vec![selected]);
    ctx.canonical_req.protocol_family = ProtocolFamily::BedrockConverse;
    let PipelineOutput::Sse(stream) =
        timeout(fixture::DEADLINE, stage_send::run(&mut ctx, &fixture.state))
            .await
            .unwrap()
            .unwrap()
    else {
        panic!("stream")
    };
    let frames: Vec<_> = timeout(fixture::DEADLINE, stream.collect()).await.unwrap();
    let success = controller.snapshot().consecutive_successes;
    let active = controller.snapshot().active_count;
    upstream.finish().await;
    fixture.finish().await;
    assert!(
        frames.last().unwrap().is_err(),
        "outer protocol completion must fail"
    );
    assert_eq!(success, 0, "inner HTTP EOF cannot commit success first");
    assert_eq!(active, 0);
}

async fn rejects_after_handoff(reply: &'static str) {
    let upstream = fixture::Upstream::start(StatusCode::OK, true, reply).await;
    let fixture = fixture::TestState::new();
    let selected = fixture::candidate(&upstream, "native-bedrock-tail-invalid");
    let controller = fixture
        .state
        .concurrency_registry
        .get_or_create(&selected.provider_account_id);
    let mut ctx = fixture::context(true, vec![selected]);
    ctx.canonical_req.protocol_family = ProtocolFamily::BedrockConverse;
    let PipelineOutput::Sse(mut stream) =
        timeout(fixture::DEADLINE, stage_send::run(&mut ctx, &fixture.state))
            .await
            .unwrap()
            .unwrap()
    else {
        panic!("stream")
    };
    let mut failed = false;
    while let Some(chunk) = timeout(fixture::DEADLINE, stream.next()).await.unwrap() {
        match chunk {
            Ok(bytes) => assert!(!matches!(
                wire::decode(&bytes).0.as_str(),
                "messageStop" | "metadata"
            )),
            Err(_) => {
                failed = true;
                break;
            }
        }
        assert_eq!(controller.snapshot().consecutive_successes, 0);
    }
    assert!(failed);
    assert!(stream.next().await.is_none());
    drop(stream);
    assert_eq!(controller.snapshot().consecutive_successes, 0);
    assert_eq!(controller.snapshot().active_count, 0);
    assert_eq!(upstream.requests().len(), 1);
    upstream.finish().await;
    fixture.finish().await;
}

#[tokio::test]
async fn native_pipeline_missing_usage_bad_tool_and_truncated_tail_are_not_success() {
    for reply in [
        "data: {\"choices\":[{\"delta\":{},\"finish_reason\":\"stop\"}]}\n\ndata: [DONE]\n\n",
        concat!("data: {\"choices\":[{\"delta\":{\"tool_calls\":[{\"id\":\"call-a\",\"function\":{\"name\":\"lookup\",\"arguments\":\"{\"}}]}}]}\n\n",
            "data: {\"choices\":[{\"delta\":{},\"finish_reason\":\"tool_calls\"}],\"usage\":{\"prompt_tokens\":1,\"completion_tokens\":2,\"total_tokens\":3}}\n\ndata: [DONE]\n\n"),
        concat!("data: {\"choices\":[{\"delta\":{},\"finish_reason\":\"stop\"}],\"usage\":{\"prompt_tokens\":1,\"completion_tokens\":2,\"total_tokens\":3}}\n\n",
            "data: [DONE]\n\ndata: {\"choices\":")
    ] { rejects_after_handoff(reply).await; }
}

#[tokio::test]
async fn native_pipeline_drop_after_content_is_neutral_and_releases_the_live_source() {
    let upstream = fixture::Upstream::start_held_stream(fixture::SSE_REPLY).await;
    let fixture = fixture::TestState::new();
    let selected = fixture::candidate(&upstream, "native-bedrock-drop");
    let controller = fixture
        .state
        .concurrency_registry
        .get_or_create(&selected.provider_account_id);
    let initial_limit = controller.snapshot().current_limit;
    let mut ctx = fixture::context(true, vec![selected]);
    ctx.canonical_req.protocol_family = ProtocolFamily::BedrockConverse;
    let PipelineOutput::Sse(mut stream) =
        timeout(fixture::DEADLINE, stage_send::run(&mut ctx, &fixture.state))
            .await
            .unwrap()
            .unwrap()
    else {
        panic!("stream")
    };
    for kind in ["messageStart", "contentBlockDelta"] {
        let chunk = timeout(fixture::DEADLINE, stream.next())
            .await
            .unwrap()
            .unwrap()
            .unwrap();
        assert_eq!(wire::decode(&chunk).0, kind);
    }
    assert!(timeout(std::time::Duration::from_millis(25), stream.next())
        .await
        .is_err());
    drop(stream);
    assert_eq!(controller.snapshot().consecutive_successes, 0);
    assert_eq!(controller.snapshot().current_limit, initial_limit);
    assert_eq!(controller.snapshot().active_count, 0);
    upstream.finish().await;
    fixture.finish().await;
}
