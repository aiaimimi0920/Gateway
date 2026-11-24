//! Session routing must retain the concrete pool account, not just the provider.
use super::*;

fn context(header: bool, label: &str) -> PipelineContext {
    let mut ctx = PipelineContext::new(make_request(), None);
    ctx.session = Some(crate::auth::session::AuthenticatedSession {
        project_id: "project".into(),
        tenant_id: "tenant".into(),
        api_key_id: Some("key-a".into()),
        user_id: None,
        credential_ref: None,
        scopes: vec![],
        user_credential_id: None,
        access_key_id: None,
        access_key_kind: None,
    });
    if header {
        ctx.client_session_hints
            .insert("session-id".into(), label.into());
    } else {
        ctx.canonical_req.explicit_session_key = Some(label.into());
    }
    ctx
}

async fn fixture(size: usize) -> (Arc<AppState>, std::path::PathBuf) {
    let credentials: Vec<_> = (0..size)
        .map(|i| {
            serde_json::json!({
                "id":format!("c{i}"), "api_key":"fixture", "supported_models":["upstream"]
            })
        })
        .collect();
    let store = RouteConfigStore::from_document(
        serde_json::from_value(serde_json::json!({
            "providers":[{"id":"site", "preset":"openai", "base_url":"http://127.0.0.1:1",
                "supported_models":["upstream"], "credentials":credentials}],
            "aliases":{"gpt-4o":"upstream"}
        }))
        .unwrap(),
    )
    .unwrap();
    let root = std::env::temp_dir().join(format!(
        "gateway-local-session-accounts-{}",
        uuid::Uuid::new_v4()
    ));
    let local = crate::local_runtime::LocalRuntime::open(&root)
        .await
        .unwrap();
    let mut state = make_state_with_route_store(store);
    Arc::get_mut(&mut state).unwrap().local_runtime = Some(local);
    (state, root)
}

async fn close(state: Arc<AppState>, root: std::path::PathBuf) {
    state.local_runtime.as_ref().unwrap().close().await;
    drop(state);
    crate::local_runtime::test_support::remove_test_root(&root).await;
}

async fn selected(state: &Arc<AppState>, ctx: &mut PipelineContext) -> RouteCandidate {
    Box::pin(run(ctx, state)).await.unwrap();
    ctx.candidates[0].clone()
}

fn send_started(state: &Arc<AppState>, ctx: &PipelineContext, candidate: &RouteCandidate) {
    super::super::super::stage_rate_limit::provider_attempt_gate(
        ctx,
        state,
        &candidate.provider_account_id,
    )
    .unwrap()
    .with_audit_candidate(candidate, "upstream")
    .begin_attempt()
    .unwrap();
}

async fn repeated_account(header: bool, size: usize) {
    let (state, root) = fixture(size).await;
    let mut sequence = Vec::new();
    for _ in 0..size * 2 {
        let mut ctx = context(header, "same-conversation");
        let candidate = selected(&state, &mut ctx).await;
        sequence.push(candidate.payload.credential_id.clone());
        send_started(&state, &ctx, &candidate);
    }
    close(state, root).await;
    assert!(
        sequence.iter().all(|id| id == &sequence[0]),
        "healthy session account rotated: {sequence:?}"
    );
}

#[tokio::test]
async fn explicit_session_keeps_same_account_with_eight_account_pool() {
    repeated_account(false, 8).await;
}

#[tokio::test]
async fn codex_session_header_keeps_same_account_with_three_account_pool() {
    repeated_account(true, 3).await;
}

#[test]
fn session_headers_are_scoped_to_authenticated_identity_not_request_id_or_user_label() {
    let mut a = context(true, "conversation-a");
    a.canonical_req.explicit_session_key = Some("same-user".into());
    let scope = super::super::smart::session_scope(&a);
    let mut b = context(true, "conversation-b");
    b.canonical_req.explicit_session_key = Some("same-user".into());
    assert_ne!(scope, super::super::smart::session_scope(&b));
    b.client_session_hints = a.client_session_hints.clone();
    assert_eq!(scope, super::super::smart::session_scope(&b));
    for field in ["tenant", "project", "key"] {
        let mut other = context(true, "conversation-a");
        let identity = other.session.as_mut().unwrap();
        match field {
            "tenant" => identity.tenant_id = "other".into(),
            "project" => identity.project_id = "other".into(),
            _ => identity.api_key_id = Some("other".into()),
        }
        assert_ne!(scope, super::super::smart::session_scope(&other));
    }
    a.session = None;
    assert!(super::super::smart::session_scope(&a).is_none());
    let mut absent = context(true, "conversation-a");
    absent.client_session_hints.clear();
    absent
        .client_session_hints
        .insert("x-client-request-id".into(), "request-only".into());
    assert!(super::super::smart::session_scope(&absent).is_none());
    let mut alias = context(true, "conversation-a");
    alias.client_session_hints.clear();
    alias
        .client_session_hints
        .insert("session_id".into(), "conversation-a".into());
    assert_eq!(scope, super::super::smart::session_scope(&alias));
}

#[tokio::test]
async fn cooling_session_account_is_excluded_and_next_admitted_account_is_retained() {
    let (state, root) = fixture(3).await;
    let mut first = context(true, "cooling-conversation");
    let previous = selected(&state, &mut first).await;
    send_started(&state, &first, &previous);
    state
        .local_runtime
        .as_ref()
        .unwrap()
        .model_failure(db::RecordCredentialModelFailureInput {
            provider_account_id: previous.provider_account_id.clone(),
            provider_credential_id: previous.provider_credential_id.clone(),
            provider_credential_ref: previous.payload.credential_id.clone(),
            protocol_profile: Some(previous.protocol_profile.clone()),
            model: "upstream".into(),
            upstream_status: Some(429),
            upstream_retry_after_ms: Some(172_800_000),
            error_message: Some("rate limit".into()),
            classification: crate::provider_failure::classify_provider_failure(
                Some(429),
                None,
                Some("rate limit"),
            ),
        })
        .await
        .unwrap();
    let mut next = context(true, "cooling-conversation");
    let replacement = selected(&state, &mut next).await;
    send_started(&state, &next, &replacement);
    let mut after = context(true, "cooling-conversation");
    let repeated = selected(&state, &mut after).await;
    close(state, root).await;
    assert_ne!(
        replacement.payload.credential_id,
        previous.payload.credential_id
    );
    assert_eq!(
        replacement.payload.credential_id,
        repeated.payload.credential_id
    );
}
