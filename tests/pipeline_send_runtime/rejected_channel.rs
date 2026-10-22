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
async fn diagnostic_html_403_advances_to_next_provider() {
    let rejected = Upstream::start(StatusCode::FORBIDDEN, false,
        "<!DOCTYPE html><html lang=\"zh-CN\"><head><title>系统终端 - 异常状态诊断</title></head></html>").await;
    let next = Upstream::start(StatusCode::OK, true, SSE_REPLY).await;
    let fixture = TestState::new();
    let mut ctx = context(
        true,
        vec![candidate(&rejected, "rejected"), candidate(&next, "next")],
    );
    authorize_candidates(&mut ctx);
    let output = timeout(DEADLINE, stage_send::run(&mut ctx, &fixture.state))
        .await
        .unwrap()
        .unwrap();
    let PipelineOutput::Sse(stream) = output else {
        panic!("expected stream")
    };
    assert!(timeout(DEADLINE, stream.collect::<Vec<_>>())
        .await
        .unwrap()
        .iter()
        .all(Result::is_ok));
    assert_eq!((rejected.requests().len(), next.requests().len()), (1, 1));
    rejected.finish().await;
    next.finish().await;
    fixture.finish().await;
}

#[tokio::test]
async fn output_then_error_in_same_chunk_is_forwarded_without_replay() {
    let failed = Upstream::start(StatusCode::OK, true,
        "data: {\"type\":\"response.output_text.delta\",\"delta\":\"partial\"}\n\nevent: response.failed\ndata: {\"type\":\"response.failed\",\"response\":{\"error\":{\"code\":\"model_not_found\",\"message\":\"upstream model unavailable\"}}}\n\n").await;
    let fallback = Upstream::start(StatusCode::OK, true, SSE_REPLY).await;
    let fixture = TestState::new();
    let mut first = candidate(&failed, "failed");
    first.payload.responses_path = Some("/v1/responses".into());
    first.payload.chat_completions_path = None;
    let mut ctx = context(true, vec![first, candidate(&fallback, "fallback")]);
    ctx.canonical_req.endpoint_kind = EndpointKind::Responses;
    authorize_candidates(&mut ctx);
    let PipelineOutput::Sse(stream) = stage_send::run(&mut ctx, &fixture.state).await.unwrap()
    else {
        panic!("expected SSE")
    };
    let chunks = timeout(DEADLINE, stream.collect::<Vec<_>>()).await.unwrap();
    assert!(chunks.iter().any(Result::is_err));
    let bytes: Vec<u8> = chunks
        .iter()
        .filter_map(|v| v.as_ref().ok())
        .flat_map(|v| v.iter().copied())
        .collect();
    assert!(String::from_utf8(bytes)
        .unwrap()
        .contains("response.failed"));
    assert!(fallback.requests().is_empty());
    failed.finish().await;
    fallback.finish().await;
    fixture.finish().await;
}

#[tokio::test]
async fn mentioning_model_not_found_does_not_authorize_tool_replay() {
    let rejected = Upstream::start(StatusCode::OK, true,
        "event: error\ndata: {\"error\":{\"code\":\"internal_error\",\"message\":\"backend failed during model_not_found recovery\"}}\n\n").await;
    let next = Upstream::start(StatusCode::OK, true, SSE_REPLY).await;
    let fixture = TestState::new();
    let mut ctx = context(
        true,
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
    assert!(result.is_err());
    assert!(next.requests().is_empty());
    rejected.finish().await;
    next.finish().await;
    fixture.finish().await;
}

#[tokio::test]
async fn stream_model_refusal_before_output_uses_next_provider() {
    let rejected = Upstream::start(StatusCode::OK, true,
        "data: {\"choices\":[{\"delta\":{\"role\":\"assistant\",\"content\":\"\"}}]}\n\nevent: error\ndata: {\"error\":{\"type\":\"model_not_found\",\"message\":\"Model 'invalid' not found\"}}\n\n").await;
    let next = Upstream::start(StatusCode::OK, true, SSE_REPLY).await;
    let fixture = TestState::new();
    let mut ctx = context(
        true,
        vec![candidate(&rejected, "bad-model"), candidate(&next, "next")],
    );
    ctx.canonical_req.tools.push(CanonicalTool {
        tool_type: "function".into(),
        name: Some("shell".into()),
        description: None,
        input_schema: None,
        raw: Default::default(),
    });
    authorize_candidates(&mut ctx);
    let output = timeout(DEADLINE, stage_send::run(&mut ctx, &fixture.state))
        .await
        .unwrap()
        .unwrap();
    let PipelineOutput::Sse(stream) = output else {
        panic!("expected stream")
    };
    let chunks = timeout(DEADLINE, stream.collect::<Vec<_>>()).await.unwrap();
    assert!(chunks.iter().all(Result::is_ok));
    assert_eq!((rejected.requests().len(), next.requests().len()), (1, 1));
    assert_eq!(ctx.route_attempt_count(), 2);
    rejected.finish().await;
    next.finish().await;
    fixture.finish().await;
}

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
async fn upstream_authentication_rejection_switches_without_retrying_bad_token() {
    let forbidden = neuro_gateway::error::classify_upstream_error(
        403,
        r#"{"error":{"message":"Forbidden by policy"}}"#,
        Some("fixture"),
    );
    assert!(matches!(
        forbidden.fallback_hint,
        neuro_gateway::error::FallbackHint::Abort { .. }
    ));
    let local = neuro_gateway::error::GatewayError::unauthorized("Invalid local key");
    assert!(matches!(
        local.fallback_hint,
        neuro_gateway::error::FallbackHint::Abort { .. }
    ));
    for streaming in [true, false] {
        let rejected = Upstream::start(
            StatusCode::UNAUTHORIZED,
            false,
            r#"{"error":{"message":"Invalid token (request id: fixture)"}}"#,
        )
        .await;
        let next = Upstream::start(
            StatusCode::OK,
            streaming,
            if streaming { SSE_REPLY } else { JSON_REPLY },
        )
        .await;
        let fixture = TestState::new();
        let mut ctx = context(
            streaming,
            vec![candidate(&rejected, "bad-token"), candidate(&next, "next")],
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
        assert!(result.is_ok(), "upstream 401 must advance");
        assert_eq!((rejected.requests().len(), next.requests().len()), (1, 1));
        assert_eq!(ctx.route_attempt_count(), 2);
        drop(result);
        rejected.finish().await;
        next.finish().await;
        fixture.finish().await;
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
    let preconsume = Upstream::start(
        StatusCode::FORBIDDEN,
        false,
        r#"{"error":{"message":"Not enough quota to pre-consume for this request: ＄13.406424 remaining, ＄18.228000 required (request id: fixture)"}}"#,
    )
    .await;
    let fixture = TestState::new();
    let mut candidates = vec![
        candidate(&rejected, "rejected"),
        candidate(&rejected, "rejected-second"),
        candidate(&depleted, "depleted"),
        candidate(&preconsume, "preconsume"),
        candidate(&next, "next"),
    ];
    let overloaded = Upstream::start(
        StatusCode::SERVICE_UNAVAILABLE,
        false,
        r#"{"error":{"code":"system_cpu_overloaded","message":"system cpu overloaded (current: 97.6%, threshold: 90%)"}}"#,
    ).await;
    candidates.insert(4, candidate(&overloaded, "overloaded"));
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
    assert_eq!((rejected.requests().len(), next.requests().len()), (2, 1));
    assert_eq!(next.requests()[0].path, "/v1/responses");
    assert_eq!(depleted.requests().len(), 1);
    assert_eq!(preconsume.requests().len(), 1);
    assert_eq!(overloaded.requests().len(), 1);
    assert_eq!(ctx.route_attempt_count(), 6);
    rejected.finish().await;
    depleted.finish().await;
    preconsume.finish().await;
    overloaded.finish().await;
    next.finish().await;
    fixture.finish().await;
}
