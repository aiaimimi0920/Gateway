//! Real loopback sends keep a pool account across both buffered and terminal streams.
use super::fixture::{
    authorize_candidates, context, TestState, Upstream, DEADLINE, JSON_REPLY, SSE_REPLY,
};
use axum::http::{HeaderMap, StatusCode};
use futures::StreamExt;
use neuro_gateway::pipeline::{stage_route, stage_send, PipelineOutput};
use serde_json::{json, Value};
use tokio::time::timeout;

async fn send(fixture: &TestState, stream: bool, session: &str) -> String {
    send_in_group(fixture, stream, session, None).await
}

async fn send_in_group(
    fixture: &TestState,
    stream: bool,
    session: &str,
    group: Option<&str>,
) -> String {
    let mut ctx = context(stream, vec![]);
    ctx.account_group_id = group.map(str::to_owned);
    ctx.session = Some(neuro_gateway::auth::session::AuthenticatedSession {
        project_id: "fixture-project".into(),
        tenant_id: "fixture-tenant".into(),
        api_key_id: Some("fixture-key".into()),
        user_id: None,
        credential_ref: None,
        scopes: vec![],
        user_credential_id: None,
        access_key_id: None,
        access_key_kind: None,
    });
    let mut headers = HeaderMap::new();
    headers.insert("session-id", session.parse().unwrap());
    neuro_gateway::http::request_headers::apply_public_request_headers(
        &mut ctx,
        &headers,
        &fixture.state.config,
        &fixture.state.console_auth,
        &[],
    )
    .unwrap();
    Box::pin(stage_route::run(&mut ctx, &fixture.state))
        .await
        .unwrap();
    authorize_candidates(&mut ctx);
    let output = timeout(DEADLINE, stage_send::run(&mut ctx, &fixture.state))
        .await
        .unwrap()
        .unwrap();
    match output {
        PipelineOutput::Json(_) => assert!(!stream),
        PipelineOutput::Sse(output) => {
            assert!(stream);
            assert!(timeout(DEADLINE, output.collect::<Vec<_>>())
                .await
                .unwrap()
                .iter()
                .all(Result::is_ok));
        }
        _ => panic!("unexpected binary response"),
    }
    ctx.audited_real_credential_ref().unwrap()
}

fn routes(first: &Upstream, second: &Upstream) -> Value {
    json!({
        "providers":[{"id":"site", "preset":"openai", "base_url":first.base_url,
            "supported_models":["upstream-model"], "credentials":[
                {"id":"c0", "api_key":"send-test-token", "base_url":first.base_url},
                {"id":"c1", "api_key":"send-test-token", "base_url":second.base_url},
                {"id":"c2", "api_key":"send-test-token", "base_url":second.base_url}
            ]}],
        "aliases":{"caller-alias":"upstream-model"}
    })
}

async fn account_reuse(stream: bool) {
    let reply = if stream { SSE_REPLY } else { JSON_REPLY };
    let first = Upstream::start(StatusCode::OK, stream, reply).await;
    let second = Upstream::start(StatusCode::OK, stream, reply).await;
    let fixture = TestState::with_local_limits().await;
    let mut document = routes(&first, &second);
    fixture
        .state
        .route_config
        .replace_document(serde_json::from_value(document.clone()).unwrap())
        .unwrap();
    for _ in 0..3 {
        assert_eq!(send(&fixture, stream, "conversation-a").await, "c0");
    }
    assert_eq!(first.requests().len(), 3);
    assert!(second.requests().is_empty());
    // A new session still receives the next round-robin account; no user-global pin.
    assert_eq!(send(&fixture, stream, "conversation-b").await, "c1");
    document["providers"][0]["credentials"][0]["enabled"] = json!(false);
    fixture
        .state
        .route_config
        .replace_document(serde_json::from_value(document).unwrap())
        .unwrap();
    let fallback = send(&fixture, stream, "conversation-a").await;
    assert_ne!(fallback, "c0", "stickiness must not undo manual disable");
    assert_eq!(send(&fixture, stream, "conversation-a").await, fallback);
    assert_eq!(first.requests().len(), 3);
    assert_eq!(second.requests().len(), 3);
    first.finish().await;
    second.finish().await;
    fixture.finish().await;
}

#[tokio::test]
async fn buffered_session_retains_account_and_rebinds_after_manual_disable() {
    account_reuse(false).await;
}

#[tokio::test]
async fn streamed_session_retains_account_and_rebinds_after_manual_disable() {
    account_reuse(true).await;
}

#[tokio::test]
async fn remembered_account_cannot_escape_the_requested_account_group() {
    let first = Upstream::start(StatusCode::OK, false, JSON_REPLY).await;
    let second = Upstream::start(StatusCode::OK, false, JSON_REPLY).await;
    let fixture = TestState::with_local_limits().await;
    let mut document = routes(&first, &second);
    document["account_groups"] = json!([{
        "id":"restricted", "name":"Restricted", "provider_credential_ids":["c1"]
    }]);
    fixture
        .state
        .route_config
        .replace_document(serde_json::from_value(document).unwrap())
        .unwrap();
    assert_eq!(send(&fixture, false, "same-conversation").await, "c0");
    for _ in 0..2 {
        assert_eq!(
            send_in_group(&fixture, false, "same-conversation", Some("restricted")).await,
            "c1"
        );
    }
    assert_eq!(first.requests().len(), 1);
    assert_eq!(second.requests().len(), 2);
    first.finish().await;
    second.finish().await;
    fixture.finish().await;
}
