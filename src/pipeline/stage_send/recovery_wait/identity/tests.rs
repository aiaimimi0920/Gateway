use super::*;
use crate::protocol::canonical::{CanonicalRelayRequest, EndpointKind, ProtocolFamily};
use crate::routing::config::RouteConfigStore;
use serde_json::{json, Value};
use std::sync::Arc;
mod wait_policy;

fn provider(id: &str, account: Option<&str>) -> Value {
    let mut value = json!({"id":id,"base_url":"https://example.invalid/v1",
        "supported_models":["public-model"],"model_map":{"public-model":"upstream-model"}});
    if let Some(account) = account {
        value["credentials"] = json!([{"id":account,"api_key":"synthetic-only"}]);
    } else {
        value["api_key"] = json!("synthetic-only");
    }
    value
}

fn routes(providers: Vec<Value>) -> Arc<RouteConfigSnapshot> {
    RouteConfigStore::from_document(serde_json::from_value(json!({"providers":providers})).unwrap())
        .unwrap()
        .snapshot()
}

fn aliases() -> ProviderAliases {
    ProviderAliases::new(vec![("old".into(), "new-api".into())]).unwrap()
}

fn setup(explicit: bool) -> (PipelineContext, RouteCandidate) {
    let snapshot = routes(vec![provider("old", explicit.then_some("account-one"))]);
    let ctx = PipelineContext::new(
        CanonicalRelayRequest {
            protocol_family: ProtocolFamily::OpenAi,
            endpoint_kind: EndpointKind::ChatCompletions,
            requested_model: Some("public-model".into()),
            stream: false,
            messages: vec![],
            tools: vec![],
            tool_choice: None,
            reasoning: None,
            metadata: None,
            raw_body: json!({}),
            previous_response_id: None,
            explicit_session_key: None,
            extra: Default::default(),
        },
        None,
    );
    let mut ctx = ctx;
    ctx.cash_route_snapshot = Some(snapshot.clone());
    let candidate = refresh(
        &ctx,
        &snapshot,
        &snapshot.account_group_constraint(None).unwrap(),
        false,
        &ProviderAliases::default(),
        &snapshot
            .resolve_candidates_admitted(
                Some("public-model"),
                &snapshot.account_group_constraint(None).unwrap(),
                &|_, _, _| true,
                false,
            )
            .candidates
            .remove(0),
    )
    .unwrap();
    (ctx, candidate)
}

fn attempt(
    ctx: &PipelineContext,
    snapshot: &RouteConfigSnapshot,
    aliases: &ProviderAliases,
    candidate: &RouteCandidate,
) -> Result<RouteCandidate, GatewayError> {
    refresh(
        ctx,
        snapshot,
        &snapshot.account_group_constraint(None).unwrap(),
        false,
        aliases,
        candidate,
    )
}

#[test]
fn sleeping_request_refreshes_only_same_explicit_account_and_can_refresh_again() {
    let (ctx, old) = setup(true);
    let next = routes(vec![
        provider("new-api", Some("account-one")),
        provider("other", Some("other-account")),
    ]);
    let fresh = attempt(&ctx, &next, &aliases(), &old).unwrap();
    assert_eq!(fresh.provider_account_id, "new-api");
    assert_eq!(fresh.payload.credential_id, old.payload.credential_id);
    assert_eq!(fresh.upstream_model, old.upstream_model);
    assert_eq!(fresh.adapter, old.adapter);
    assert_eq!(
        attempt(&ctx, &next, &aliases(), &fresh)
            .unwrap()
            .provider_account_id,
        "new-api"
    );
    assert_eq!(
        ctx.cash_route_snapshot
            .as_ref()
            .unwrap()
            .document()
            .providers[0]
            .id,
        "old"
    );
    assert!(ctx.projected_access_candidates.is_empty());
    assert_eq!(ctx.route_attempt_count(), 0);
}

#[test]
fn absent_alias_coexisting_names_and_implicit_accounts_fail_closed() {
    let (ctx, old) = setup(true);
    let next = routes(vec![provider("new-api", Some("account-one"))]);
    assert!(attempt(&ctx, &next, &ProviderAliases::default(), &old).is_err());
    let both = routes(vec![
        provider("new-api", Some("account-one")),
        provider("old", Some("other-account")),
    ]);
    assert!(attempt(&ctx, &both, &aliases(), &old).is_err());
    let (ctx, old) = setup(false);
    let next = routes(vec![provider("new-api", None)]);
    assert!(attempt(&ctx, &next, &aliases(), &old).is_err());
}

#[test]
fn account_model_protocol_and_group_changes_cannot_become_a_rename() {
    let (ctx, old) = setup(true);
    let mut changed_account = provider("new-api", Some("different-account"));
    let mut disabled = provider("new-api", Some("account-one"));
    disabled["credentials"][0]["enabled"] = json!(false);
    let mut changed_model = provider("new-api", Some("account-one"));
    changed_model["model_map"]["public-model"] = json!("different-model");
    for changed in [&mut changed_account, &mut disabled, &mut changed_model] {
        assert!(attempt(&ctx, &routes(vec![changed.clone()]), &aliases(), &old).is_err());
    }
    let next = routes(vec![provider("new-api", Some("account-one"))]);
    let mut wrong_protocol = old.clone();
    wrong_protocol.protocol_profile = "not-the-original-profile".into();
    assert!(attempt(&ctx, &next, &aliases(), &wrong_protocol).is_err());
    let mut wrong_adapter = old.clone();
    wrong_adapter.adapter = "other-adapter".into();
    assert!(attempt(&ctx, &next, &aliases(), &wrong_adapter).is_err());
    let excluded = next.access_key_group_constraint(&["missing-group".into()]);
    assert!(refresh(&ctx, &next, &excluded, true, &aliases(), &old).is_err());
}

#[test]
fn staged_mapping_preserves_old_route_and_never_rewrites_projected_accounts() {
    let (ctx, old) = setup(true);
    let current = ctx.cash_route_snapshot.as_ref().unwrap();
    assert_eq!(
        attempt(&ctx, current, &aliases(), &old)
            .unwrap()
            .provider_account_id,
        "old"
    );
    let mut external = old.clone();
    external.provider_credential_id = Some("database-account".into());
    assert!(attempt(&ctx, current, &aliases(), &external).is_err());
    assert_eq!(
        attempt(&ctx, current, &ProviderAliases::default(), &external)
            .unwrap()
            .provider_credential_id,
        external.provider_credential_id
    );
}
