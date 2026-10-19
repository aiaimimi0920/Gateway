//! Real loopback stage_send wiring for reference-inspired stream changes.
#[path = "pipeline_send_runtime/config.rs"]
mod config;
#[path = "pipeline_send_runtime/fixture.rs"]
mod fixture;
mod support;

use axum::http::StatusCode;
use futures::StreamExt;
use neuro_gateway::pipeline::{stage_send, PipelineOutput};
use neuro_gateway::protocol::canonical::EndpointKind;
use std::sync::atomic::Ordering;
use tokio::time::timeout;

use fixture::{candidate, context, TestState, Upstream, DEADLINE};

const PARTIAL: &str =
    "data: {\"choices\":[{\"delta\":{\"content\":\"partial-reply\"},\"finish_reason\":null}]}\n\n";
const COMPLETE_WITH_BOM: &str = concat!(
    "\u{feff}data: {\"choices\":[{\"delta\":{\"content\":\"bom-reply\"},\"finish_reason\":null}]}\n\n",
    "data: {\"choices\":[{\"delta\":{},\"finish_reason\":\"stop\"}],\"usage\":{\"prompt_tokens\":2,\"completion_tokens\":3,\"total_tokens\":5}}\n\n"
);

async fn run_case(wire: &'static str, complete: bool) {
    let upstream = Upstream::start(StatusCode::OK, true, wire).await;
    let fixture = TestState::new();
    let mut selected = candidate(&upstream, "reference-primary");
    selected.payload.chat_completions_path = Some("/v1/chat/completions".into());
    let controller = fixture
        .state
        .concurrency_registry
        .get_or_create(&selected.provider_account_id);
    let backup = candidate(&upstream, "reference-backup");
    let mut ctx = context(true, vec![selected, backup]);
    ctx.canonical_req.endpoint_kind = EndpointKind::Completions;
    let output = timeout(DEADLINE, stage_send::run(&mut ctx, &fixture.state))
        .await
        .unwrap()
        .unwrap();
    let PipelineOutput::Sse(stream) = output else {
        panic!("expected SSE");
    };
    let chunks = timeout(DEADLINE, stream.collect::<Vec<_>>()).await.unwrap();
    let text = String::from_utf8(
        chunks
            .iter()
            .filter_map(|chunk| chunk.as_ref().ok())
            .flat_map(|chunk| chunk.iter().copied())
            .collect(),
    )
    .unwrap();
    if complete {
        assert!(chunks.iter().all(Result::is_ok));
        assert!(text.contains("bom-reply"));
        assert!(text.contains("\"total_tokens\":5"));
        assert_eq!(text.matches("[DONE]").count(), 1);
        assert_eq!(controller.snapshot().consecutive_successes, 1);
    } else {
        assert_eq!(chunks.iter().filter(|chunk| chunk.is_err()).count(), 1);
        assert!(chunks
            .last()
            .unwrap()
            .as_ref()
            .unwrap_err()
            .to_string()
            .contains("openai_chat_stream_incomplete"));
        assert!(text.contains("partial-reply"));
        assert!(!text.contains("[DONE]"));
        assert!(!text.contains("\"finish_reason\":\"stop\""));
        assert_eq!(controller.snapshot().consecutive_successes, 0);
    }
    assert_eq!(controller.snapshot().active_count, 0);
    assert_eq!(ctx.route_attempt_count.load(Ordering::Relaxed), 1);
    assert_eq!(
        upstream.requests().len(),
        1,
        "stream ownership must not re-enter fallback"
    );
    assert_eq!(upstream.requests()[0].path, "/v1/chat/completions");
    assert!(upstream.requests()[0].fixture_auth);
    upstream.finish().await;
    fixture.finish().await;
}

#[tokio::test]
async fn clean_http_eof_is_not_a_successful_legacy_completion_or_a_replay() {
    run_case(PARTIAL, false).await;
}

#[tokio::test]
async fn bom_prefixed_chat_stream_keeps_text_usage_and_success_tracking() {
    run_case(COMPLETE_WITH_BOM, true).await;
}
