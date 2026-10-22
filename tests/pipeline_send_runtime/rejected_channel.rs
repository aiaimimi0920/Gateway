//! A CLI tool declaration must not prevent fallback after model admission rejection.
use super::fixture::{
    authorize_candidates, candidate, context, TestState, Upstream, DEADLINE, JSON_REPLY, SSE_REPLY,
};
use axum::http::StatusCode;
use futures::StreamExt;
use neuro_gateway::pipeline::{stage_send, PipelineOutput};
use neuro_gateway::protocol::canonical::CanonicalTool;
use neuro_gateway::protocol::canonical::EndpointKind;
use tokio::time::timeout;

#[tokio::test]
async fn rejected_tool_request_advances_once_in_streaming_and_buffered_modes() {
    for streaming in [true, false] {
        let rejected = Upstream::start(StatusCode::SERVICE_UNAVAILABLE, false,
            r#"{"error":{"code":"model_not_found","message":"No available channel for model gpt-6.1-sol under group default (distributor)"}}"#).await;
        let next = Upstream::start(
            StatusCode::OK,
            streaming,
            if streaming { SSE_REPLY } else { JSON_REPLY },
        )
        .await;
        let fixture = TestState::new();
        let mut ctx = context(
            streaming,
            vec![candidate(&rejected, "rejected"), candidate(&next, "next")],
        );
        ctx.canonical_req.tools.push(CanonicalTool {
            tool_type: "function".into(),
            name: Some("shell".into()),
            description: None,
            input_schema: None,
            raw: Default::default(),
        });
        authorize_candidates(&mut ctx);
        let result = timeout(DEADLINE, stage_send::run(&mut ctx, &fixture.state))
            .await
            .unwrap();
        let counts = (rejected.requests().len(), next.requests().len());
        let attempts = ctx.route_attempt_count();
        let success = matches!(
            &result,
            Ok(PipelineOutput::Json(_)) | Ok(PipelineOutput::Sse(_))
        );
        drop(result);
        rejected.finish().await;
        next.finish().await;
        fixture.finish().await;
        assert!(success, "explicit rejection must allow the next provider");
        assert_eq!(counts, (1, 1));
        assert_eq!(attempts, 2);
    }
}

#[tokio::test]
async fn responses_cli_tools_rejection_reaches_completed_stream() {
    let rejected = Upstream::start(StatusCode::SERVICE_UNAVAILABLE, false,
        r#"{"error":{"code":"model_not_found","message":"No available channel for model gpt-6.1-sol under group default (distributor)"}}"#).await;
    let complete = "event: response.completed\ndata: {\"type\":\"response.completed\",\"response\":{\"status\":\"completed\",\"output\":[]}}\n\n";
    let next = Upstream::start(StatusCode::OK, true, complete).await;
    let depleted = Upstream::start(
        StatusCode::FORBIDDEN,
        false,
        r#"{"error":{"code":"bad_response_status_code","message":"Insufficient account balance"}}"#,
    )
    .await;
    let fixture = TestState::new();
    let mut candidates = vec![
        candidate(&rejected, "rejected"),
        candidate(&depleted, "depleted"),
        candidate(&next, "next"),
    ];
    for candidate in &mut candidates {
        candidate.payload.responses_path = Some("/v1/responses".into());
        candidate.payload.chat_completions_path = None;
    }
    let mut ctx = context(true, candidates);
    ctx.canonical_req.endpoint_kind = EndpointKind::Responses;
    ctx.canonical_req.tool_choice = Some(serde_json::json!("auto"));
    ctx.canonical_req.tools.push(CanonicalTool {
        tool_type: "function".into(),
        name: Some("shell".into()),
        description: None,
        input_schema: None,
        raw: Default::default(),
    });
    authorize_candidates(&mut ctx);
    let result = timeout(DEADLINE, stage_send::run(&mut ctx, &fixture.state))
        .await
        .unwrap()
        .unwrap();
    let PipelineOutput::Sse(stream) = result else {
        panic!("expected SSE")
    };
    let chunks = timeout(DEADLINE, stream.collect::<Vec<_>>()).await.unwrap();
    assert!(chunks.iter().all(Result::is_ok));
    let bytes: Vec<u8> = chunks
        .into_iter()
        .flat_map(|chunk| chunk.unwrap().to_vec())
        .collect();
    assert!(String::from_utf8(bytes)
        .unwrap()
        .contains("response.completed"));
    assert_eq!((rejected.requests().len(), next.requests().len()), (1, 1));
    assert_eq!(next.requests()[0].path, "/v1/responses");
    assert_eq!(depleted.requests().len(), 1);
    assert_eq!(ctx.route_attempt_count(), 3);
    rejected.finish().await;
    depleted.finish().await;
    next.finish().await;
    fixture.finish().await;
}
