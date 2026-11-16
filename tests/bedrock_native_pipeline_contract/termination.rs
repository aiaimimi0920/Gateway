//! Preserve caller/deadline neutrality while propagating a real upstream transport failure.
use super::{fixture, wire};
use futures::StreamExt;
use neuro_gateway::{
    pipeline::{request_budget::RequestBudget, stage_send, PipelineOutput},
    protocol::canonical::ProtocolFamily,
};
use std::time::Duration;
use tokio::time::timeout;

#[tokio::test]
async fn native_deadline_and_transport_failure_never_emit_terminal_success_or_replay() {
    for deadline in [true, false] {
        let upstream = if deadline {
            fixture::Upstream::start_held_stream(fixture::SSE_REPLY).await
        } else {
            fixture::Upstream::start_broken_stream(fixture::SSE_REPLY).await
        };
        let fixture = fixture::TestState::new();
        let selected = fixture::candidate(&upstream, "native-bedrock-termination");
        let controller = fixture
            .state
            .concurrency_registry
            .get_or_create(&selected.provider_account_id);
        let initial_limit = controller.snapshot().current_limit;
        let mut ctx = fixture::context(true, vec![selected]);
        ctx.canonical_req.protocol_family = ProtocolFamily::BedrockConverse;
        if deadline {
            ctx.request_budget = RequestBudget::new(6, Duration::from_secs(1));
        }
        let PipelineOutput::Sse(mut stream) =
            timeout(fixture::DEADLINE, stage_send::run(&mut ctx, &fixture.state))
                .await
                .unwrap()
                .unwrap()
        else {
            panic!("stream")
        };
        for kind in ["messageStart", "contentBlockDelta"] {
            assert_eq!(
                wire::decode(
                    &timeout(fixture::DEADLINE, stream.next())
                        .await
                        .unwrap()
                        .unwrap()
                        .unwrap()
                )
                .0,
                kind
            );
        }
        if !deadline {
            upstream.release_stream();
        }
        let error = timeout(fixture::DEADLINE, stream.next())
            .await
            .unwrap()
            .unwrap()
            .unwrap_err();
        if deadline {
            assert!(error.to_string().contains("budget_exhausted:deadline"));
            assert_eq!(controller.snapshot().current_limit, initial_limit);
        } else {
            assert!(controller.snapshot().current_limit < initial_limit);
        }
        assert!(stream.next().await.is_none());
        drop(stream);
        assert_eq!(controller.snapshot().consecutive_successes, 0);
        assert_eq!(controller.snapshot().active_count, 0);
        assert_eq!(ctx.route_attempt_count(), 1);
        assert_eq!(upstream.requests().len(), 1);
        upstream.finish().await;
        fixture.finish().await;
    }
}

#[tokio::test]
async fn native_drop_after_metadata_without_polling_outer_eof_is_neutral_not_success() {
    let upstream =
        fixture::Upstream::start(axum::http::StatusCode::OK, true, fixture::SSE_REPLY).await;
    let fixture = fixture::TestState::new();
    let selected = fixture::candidate(&upstream, "native-bedrock-metadata-drop");
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
    loop {
        let chunk = timeout(fixture::DEADLINE, stream.next())
            .await
            .unwrap()
            .unwrap()
            .unwrap();
        if wire::decode(&chunk).0 == "metadata" {
            break;
        }
    }
    assert_eq!(controller.snapshot().consecutive_successes, 0);
    drop(stream);
    assert_eq!(controller.snapshot().consecutive_successes, 0);
    assert_eq!(controller.snapshot().current_limit, initial_limit);
    assert_eq!(controller.snapshot().active_count, 0);
    upstream.finish().await;
    fixture.finish().await;
}
