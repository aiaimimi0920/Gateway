//! Old requests read the mapped provider policy without receiving a larger wait budget.
use super::*;
use crate::pipeline::stage_send::recovery_wait::policy::for_candidate;
use crate::request_wait::RequestWaitMode;

fn configured(id: &str, mode: &str, timeout: Option<u32>) -> Value {
    let mut value = provider(id, Some("account-one"));
    value["request_wait"] = json!({"mode":mode,"total_timeout_seconds":timeout});
    value
}

#[test]
fn old_candidate_finds_new_policy_and_keeps_the_stricter_original_wait() {
    for (old_mode, new_mode, expected) in [
        ("bounded", "client_connected", RequestWaitMode::Bounded),
        ("client_connected", "bounded", RequestWaitMode::Bounded),
        (
            "client_connected",
            "client_connected",
            RequestWaitMode::ClientConnected,
        ),
    ] {
        let (mut ctx, candidate) = setup(true);
        ctx.cash_route_snapshot = Some(routes(vec![configured("old", old_mode, Some(20))]));
        let next = routes(vec![configured("new-api", new_mode, Some(50))]);
        let policy = for_candidate(&ctx, &next, &aliases(), &candidate).unwrap();
        assert_eq!(policy.mode, expected);
        assert_eq!(policy.total_timeout_seconds, Some(20));
        let next = routes(vec![configured("new-api", new_mode, Some(5))]);
        assert_eq!(
            for_candidate(&ctx, &next, &aliases(), &candidate)
                .unwrap()
                .total_timeout_seconds,
            Some(5)
        );
    }
}

#[test]
fn original_or_current_disabled_policy_cannot_be_revived_by_rename() {
    for (old_mode, new_mode) in [("disabled", "client_connected"), ("bounded", "disabled")] {
        let (mut ctx, candidate) = setup(true);
        ctx.cash_route_snapshot = Some(routes(vec![configured("old", old_mode, None)]));
        let next = routes(vec![configured("new-api", new_mode, None)]);
        assert_eq!(
            for_candidate(&ctx, &next, &aliases(), &candidate)
                .unwrap()
                .mode,
            RequestWaitMode::Disabled
        );
    }
}

#[test]
fn policy_lookup_handles_staged_mapping_and_missing_mapping_without_guessing() {
    let (mut ctx, candidate) = setup(true);
    let old = routes(vec![configured("old", "bounded", Some(30))]);
    ctx.cash_route_snapshot = Some(old.clone());
    let policy = for_candidate(&ctx, &old, &aliases(), &candidate).unwrap();
    assert_eq!(policy.mode, RequestWaitMode::Bounded);
    assert_eq!(policy.total_timeout_seconds, Some(30));
    let next = routes(vec![configured("new-api", "client_connected", None)]);
    assert_eq!(
        for_candidate(&ctx, &next, &ProviderAliases::default(), &candidate)
            .unwrap()
            .mode,
        RequestWaitMode::Disabled
    );
}

#[test]
fn mapped_policy_rejects_ambiguous_default_or_projected_account_ownership() {
    let (ctx, candidate) = setup(true);
    let both = routes(vec![
        configured("new-api", "bounded", None),
        provider("old", Some("different-account")),
    ]);
    assert!(for_candidate(&ctx, &both, &aliases(), &candidate).is_err());
    let next = routes(vec![configured("new-api", "bounded", None)]);
    let mut projected = candidate.clone();
    projected.provider_credential_id = Some("database-account".into());
    assert!(for_candidate(&ctx, &next, &aliases(), &projected).is_err());
    let (ctx, implicit) = setup(false);
    let next = routes(vec![provider("new-api", None)]);
    assert!(for_candidate(&ctx, &next, &aliases(), &implicit).is_err());
}
