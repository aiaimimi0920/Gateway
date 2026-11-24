use std::collections::HashSet;

use axum::http::HeaderMap;
use neuro_gateway::access_control::{
    authorize_internal_request, ensure_allowed_resource_ids, AccessBoundary, InternalAccessSurface,
};
use neuro_gateway::rate_limit::{build_rate_limit_scope_keys, RateLimitDimensions, RateLimitScope};
use neuro_gateway::splitter::SplitterWorkerStatus;

fn header_map(name: &'static str, value: &'static str) -> HeaderMap {
    let mut headers = HeaderMap::new();
    headers.insert(name, value.parse().expect("valid header value"));
    headers
}

#[test]
fn internal_surfaces_fail_closed_without_configured_tokens() {
    for surface in [
        InternalAccessSurface::Management,
        InternalAccessSurface::BrowserExecutor,
    ] {
        let error = authorize_internal_request(surface, None, false, None, &HeaderMap::new())
            .expect_err("missing internal credentials must fail closed");

        assert_eq!(error.http_status, Some(503));
        assert!(matches!(
            error.code.as_deref(),
            Some("gateway_management_token_not_configured")
                | Some("browser_executor_token_not_configured")
        ));
    }
}

#[test]
fn internal_surfaces_require_the_exact_configured_token() {
    let wrong = authorize_internal_request(
        InternalAccessSurface::Management,
        Some("management-secret"),
        false,
        None,
        &header_map("x-management-token", "wrong"),
    )
    .expect_err("wrong management token must be rejected");
    assert_eq!(wrong.http_status, Some(401));

    authorize_internal_request(
        InternalAccessSurface::Management,
        Some("management-secret"),
        false,
        Some("management-secret"),
        &HeaderMap::new(),
    )
    .expect("bearer management token should be accepted");

    authorize_internal_request(
        InternalAccessSurface::BrowserExecutor,
        Some("executor-secret"),
        false,
        None,
        &header_map("x-internal-api-key", "executor-secret"),
    )
    .expect("browser executor internal key should be accepted");
}

#[test]
fn access_boundaries_reject_cross_tenant_and_cross_project_bindings() {
    let expected = AccessBoundary::new("tenant-a", "project-a").unwrap();
    let same = AccessBoundary::new("tenant-a", "project-a").unwrap();
    let other_tenant = AccessBoundary::new("tenant-b", "project-a").unwrap();
    let other_project = AccessBoundary::new("tenant-a", "project-b").unwrap();

    expected
        .ensure_same(&same, "access key")
        .expect("same tenant/project boundary must be accepted");
    assert_eq!(
        expected
            .ensure_same(&other_tenant, "access key")
            .expect_err("cross-tenant binding must be rejected")
            .code
            .as_deref(),
        Some("access_boundary_mismatch")
    );
    assert_eq!(
        expected
            .ensure_same(&other_project, "access key")
            .expect_err("cross-project binding must be rejected")
            .code
            .as_deref(),
        Some("access_boundary_mismatch")
    );
}

#[test]
fn scoped_resource_ids_reject_ids_outside_the_resolved_boundary() {
    let requested = vec!["bundle-a".to_string(), "bundle-b".to_string()];
    let allowed = vec!["bundle-a".to_string()];

    let error = ensure_allowed_resource_ids(&requested, &allowed, "access bundle")
        .expect_err("cross-project resource IDs must be rejected");

    assert_eq!(error.code.as_deref(), Some("access_boundary_mismatch"));
    assert!(error.message.contains("bundle-b"));
}

#[test]
fn rate_limit_scope_keys_cover_project_key_model_endpoint_and_provider() {
    let keys = build_rate_limit_scope_keys(&RateLimitDimensions {
        tenant_id: "tenant-a",
        project_id: "project-a",
        access_key_id: Some("access-key-a"),
        model: Some("GPT-4O"),
        endpoint: "Chat_Completions",
        endpoint_policy_alias: Some("POST /v1/chat/completions"),
        provider_account_id: Some("provider-a"),
    })
    .unwrap();

    let scopes = keys.iter().map(|entry| entry.scope).collect::<HashSet<_>>();
    assert_eq!(
        scopes,
        HashSet::from([
            RateLimitScope::Project,
            RateLimitScope::AccessKey,
            RateLimitScope::Model,
            RateLimitScope::Endpoint,
            RateLimitScope::ProviderAttempt,
        ])
    );
    assert_eq!(
        keys.iter()
            .map(|entry| entry.key.as_str())
            .collect::<HashSet<_>>()
            .len(),
        5
    );
}

#[test]
fn rate_limit_scope_keys_are_tenant_project_and_component_collision_safe() {
    let first = build_rate_limit_scope_keys(&RateLimitDimensions {
        tenant_id: "a:b",
        project_id: "c",
        access_key_id: Some("key-a"),
        model: Some("model-a"),
        endpoint: "chat",
        endpoint_policy_alias: None,
        provider_account_id: Some("provider-a"),
    })
    .unwrap();
    let second = build_rate_limit_scope_keys(&RateLimitDimensions {
        tenant_id: "a",
        project_id: "b:c",
        access_key_id: Some("key-a"),
        model: Some("model-a"),
        endpoint: "chat",
        endpoint_policy_alias: None,
        provider_account_id: Some("provider-a"),
    })
    .unwrap();

    for scope in [
        RateLimitScope::Project,
        RateLimitScope::AccessKey,
        RateLimitScope::Model,
        RateLimitScope::Endpoint,
        RateLimitScope::ProviderAttempt,
    ] {
        assert_ne!(scope_key(&first, scope), scope_key(&second, scope));
    }
}

#[test]
fn splitter_routes_only_workers_in_the_active_state() {
    assert!(!SplitterWorkerStatus::Starting.can_receive_traffic());
    assert!(!SplitterWorkerStatus::Ready.can_receive_traffic());
    assert!(SplitterWorkerStatus::Active.can_receive_traffic());
    assert!(!SplitterWorkerStatus::Draining.can_receive_traffic());
    assert!(!SplitterWorkerStatus::Exited.can_receive_traffic());
}

#[test]
fn splitter_replacement_requires_ready_cutover_before_old_worker_drain() {
    let replacement = SplitterWorkerStatus::Starting
        .transition_to(SplitterWorkerStatus::Ready)
        .unwrap()
        .transition_to(SplitterWorkerStatus::Active)
        .unwrap();
    let previous = SplitterWorkerStatus::Active
        .transition_to(SplitterWorkerStatus::Draining)
        .unwrap()
        .transition_to(SplitterWorkerStatus::Exited)
        .unwrap();

    assert_eq!(replacement, SplitterWorkerStatus::Active);
    assert_eq!(previous, SplitterWorkerStatus::Exited);
    assert!(
        SplitterWorkerStatus::Starting
            .transition_to(SplitterWorkerStatus::Active)
            .is_err(),
        "a replacement must not become active before readiness succeeds"
    );
}

fn scope_key(keys: &[neuro_gateway::rate_limit::RateLimitScopeKey], scope: RateLimitScope) -> &str {
    keys.iter()
        .find(|entry| entry.scope == scope)
        .map(|entry| entry.key.as_str())
        .expect("scope key must exist")
}
