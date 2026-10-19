use super::fixture::{candidate, context, TestState, Upstream, DEADLINE, SSE_REPLY};
use axum::http::StatusCode;
use futures::StreamExt;
use neuro_gateway::pipeline::{stage_send, PipelineOutput};
use neuro_gateway::routing::latency::LatencyKey;
use tokio::time::timeout;

#[tokio::test]
async fn only_successfully_completed_nonempty_streams_teach_ttft() {
    for completed in [true, false] {
        let upstream = Upstream::start(StatusCode::OK, true, SSE_REPLY).await;
        let fixture = TestState::new();
        let selected = candidate(&upstream, "stream-sample");
        let mut ctx = context(true, vec![selected.clone()]);
        let key = LatencyKey::for_request(&selected, &ctx.canonical_req).unwrap();
        let mut buffered_req = ctx.canonical_req.clone();
        buffered_req.stream = false;
        let buffered_key = LatencyKey::for_request(&selected, &buffered_req).unwrap();
        let PipelineOutput::Sse(mut stream) =
            timeout(DEADLINE, stage_send::run(&mut ctx, &fixture.state))
                .await
                .unwrap()
                .unwrap()
        else {
            panic!("expected stream")
        };
        assert!(fixture.state.routing_latency.estimate_ms(&key).is_none());
        if completed {
            timeout(DEADLINE, async {
                while let Some(chunk) = stream.next().await {
                    chunk.unwrap();
                }
            })
            .await
            .unwrap();
        }
        drop(stream);
        let estimate = fixture.state.routing_latency.estimate_ms(&key);
        assert_eq!(estimate.is_some(), completed);
        assert!(fixture
            .state
            .routing_latency
            .estimate_ms(&buffered_key)
            .is_none());
        upstream.finish().await;
        fixture.finish().await;
    }
}

#[tokio::test]
async fn interrupted_streams_do_not_teach_a_success() {
    let upstream = Upstream::start_broken_stream(
        "data: {\"choices\":[{\"delta\":{\"content\":\"partial\"}}]}\n\n",
    )
    .await;
    let fixture = TestState::new();
    let selected = candidate(&upstream, "broken-sample");
    let mut ctx = context(true, vec![selected.clone()]);
    let key = LatencyKey::for_request(&selected, &ctx.canonical_req).unwrap();
    let PipelineOutput::Sse(mut stream) =
        timeout(DEADLINE, stage_send::run(&mut ctx, &fixture.state))
            .await
            .unwrap()
            .unwrap()
    else {
        panic!("expected stream")
    };
    let first = timeout(DEADLINE, stream.next())
        .await
        .unwrap()
        .unwrap()
        .unwrap();
    assert!(!first.is_empty());
    upstream.release_stream();
    let mut failed = false;
    timeout(DEADLINE, async {
        while let Some(chunk) = stream.next().await {
            failed |= chunk.is_err();
        }
    })
    .await
    .unwrap();
    drop(stream);
    assert!(failed);
    assert!(fixture.state.routing_latency.estimate_ms(&key).is_none());
    upstream.finish().await;
    fixture.finish().await;
}

#[tokio::test]
async fn binary_success_uses_the_shared_dispatch_sample_exit() {
    use neuro_gateway::protocol::canonical::EndpointKind;
    let upstream = Upstream::start(StatusCode::OK, false, "fake-binary-audio").await;
    let fixture = TestState::new();
    let mut selected = candidate(&upstream, "binary-sample");
    selected.payload.audio_speech_path = Some("/v1/chat/completions".into());
    selected
        .supported_protocol_families
        .push("openai_audio_speech".into());
    let mut ctx = context(false, vec![selected.clone()]);
    ctx.canonical_req.endpoint_kind = EndpointKind::AudioSpeech;
    ctx.canonical_req.raw_body = serde_json::json!({"input":"fixture speech", "voice":"alloy"});
    let key = LatencyKey::for_request(&selected, &ctx.canonical_req).unwrap();
    let PipelineOutput::Binary(output) =
        timeout(DEADLINE, stage_send::run(&mut ctx, &fixture.state))
            .await
            .unwrap()
            .unwrap()
    else {
        panic!("expected binary")
    };
    assert_eq!(output.body.as_ref(), b"fake-binary-audio");
    assert!(fixture.state.routing_latency.estimate_ms(&key).is_some());
    assert_eq!(upstream.requests().len(), 1);
    upstream.finish().await;
    fixture.finish().await;
}

#[tokio::test]
async fn empty_stream_does_not_invent_a_ttft_sample() {
    let upstream = Upstream::start(StatusCode::OK, true, "").await;
    let fixture = TestState::new();
    let selected = candidate(&upstream, "empty-sample");
    let mut ctx = context(true, vec![selected.clone()]);
    let key = LatencyKey::for_request(&selected, &ctx.canonical_req).unwrap();
    let output = timeout(DEADLINE, stage_send::run(&mut ctx, &fixture.state))
        .await
        .unwrap();
    if let Ok(PipelineOutput::Sse(mut stream)) = output {
        timeout(DEADLINE, async { while stream.next().await.is_some() {} })
            .await
            .unwrap();
        drop(stream);
    }
    assert!(fixture.state.routing_latency.estimate_ms(&key).is_none());
    upstream.finish().await;
    fixture.finish().await;
}
