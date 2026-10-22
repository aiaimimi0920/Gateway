//! Actual sends pin the authenticated conversation even if the caller cancels.
use super::fixture::{
    authorize_candidates, context as base_context, TestState, Upstream, DEADLINE, SSE_REPLY,
};
use axum::http::StatusCode;
use futures::StreamExt;
use neuro_gateway::pipeline::{stage_route, stage_send, PipelineOutput};
use serde_json::json;
use tokio::time::timeout;

fn context(
    stream: bool,
    candidates: Vec<neuro_gateway::routing::candidate::RouteCandidate>,
) -> neuro_gateway::pipeline::PipelineContext {
    let mut ctx = base_context(stream, candidates);
    ctx.session = Some(neuro_gateway::auth::session::AuthenticatedSession {
        project_id: "fixture-project".into(),
        tenant_id: "fixture-tenant".into(),
        user_id: None,
        credential_ref: None,
        scopes: vec![],
        api_key_id: Some("fixture-key".into()),
        user_credential_id: None,
        access_key_id: None,
        access_key_kind: None,
    });
    ctx
}

#[tokio::test]
async fn actual_send_pins_same_session_over_healthy_preferred_provider() {
    let preferred = Upstream::start(StatusCode::OK, true, SSE_REPLY).await;
    let successful = Upstream::start(StatusCode::OK, true, SSE_REPLY).await;
    let fixture = TestState::with_local_limits().await;
    fixture.state.route_config.replace_document(serde_json::from_value(json!({
        "providers":[
            {"id":"preferred","preset":"openai","base_url":preferred.base_url,"api_key":"send-test-token","weight":100,"supported_models":["upstream-model"]},
            {"id":"successful","preset":"openai","base_url":successful.base_url,"api_key":"send-test-token","weight":1,"supported_models":["upstream-model"]}
        ],
        "model_routes":[{"pattern":"upstream-model","provider_ids":["preferred","successful"]}],
        "aliases":{"caller-alias":"upstream-model"}
    })).unwrap()).unwrap();
    let candidates = fixture
        .state
        .route_config
        .resolve_candidates(Some("upstream-model"));
    let winner = candidates
        .iter()
        .find(|c| c.provider_account_id == "successful")
        .unwrap()
        .clone();
    let mut cancelled = context(true, vec![winner.clone()]);
    cancelled.canonical_req.requested_model = Some("caller-alias".into());
    cancelled.canonical_req.explicit_session_key = Some("two-streams".into());
    authorize_candidates(&mut cancelled);
    let output = stage_send::run(&mut cancelled, &fixture.state)
        .await
        .unwrap();
    drop(output); // Actual sends establish stickiness even when the caller cancels.
    timeout(DEADLINE, async {
        while std::sync::Arc::strong_count(&fixture.state) != 1 {
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap();
    let mut after_cancel = context(true, Vec::new());
    after_cancel.canonical_req.requested_model = Some("caller-alias".into());
    after_cancel.canonical_req.explicit_session_key = Some("two-streams".into());
    stage_route::run(&mut after_cancel, &fixture.state)
        .await
        .unwrap();
    assert_eq!(after_cancel.candidates[0].provider_account_id, "successful");
    // Start at the eventual successful candidate without cooling the preferred provider.
    // Otherwise health exclusion alone could make this test pass without affinity.
    let mut first = context(true, vec![winner]);
    first.canonical_req.requested_model = Some("caller-alias".into());
    first.canonical_req.explicit_session_key = Some("two-streams".into());
    authorize_candidates(&mut first);
    let PipelineOutput::Sse(stream) = stage_send::run(&mut first, &fixture.state).await.unwrap()
    else {
        panic!("expected stream")
    };
    let chunks = timeout(DEADLINE, stream.collect::<Vec<_>>()).await.unwrap();
    assert!(chunks.iter().all(Result::is_ok));
    // Terminal persistence is owned by the tracked finalizer, not first-byte delivery.
    let mut second = timeout(DEADLINE, async {
        loop {
            let mut next = context(true, Vec::new());
            next.canonical_req.requested_model = Some("caller-alias".into());
            next.canonical_req.explicit_session_key = Some("two-streams".into());
            stage_route::run(&mut next, &fixture.state).await.unwrap();
            if next.candidates[0].provider_account_id == "successful" {
                break next;
            }
            tokio::task::yield_now().await;
        }
    })
    .await
    .expect("stream success must persist affinity");
    authorize_candidates(&mut second);
    let PipelineOutput::Sse(stream) = stage_send::run(&mut second, &fixture.state).await.unwrap()
    else {
        panic!("expected stream")
    };
    assert!(timeout(DEADLINE, stream.collect::<Vec<_>>())
        .await
        .unwrap()
        .iter()
        .all(Result::is_ok));
    assert!(preferred.requests().is_empty());
    assert_eq!(successful.requests().len(), 3); // One cancelled, then two completed requests.
    preferred.finish().await;
    successful.finish().await;
    fixture.finish().await;
}
