use super::*;

fn credential(provider: &str, id: &str) -> GatewayProviderCredentialView {
    GatewayProviderCredentialView {
        id: id.into(),
        provider_account_id: provider.into(),
        label: id.into(),
        status: "active".into(),
        payload: serde_json::json!({}),
        storage_mode: "inline".into(),
        source_kind: "manual".into(),
        source_path: None,
        source_hash: None,
        sync_mode: "manual".into(),
        sync_state: "ready".into(),
        sync_error: None,
        cooldown_until: None,
        last_error: None,
        failure_count: 0,
        last_health_check_at: None,
        created_at: String::new(),
        updated_at: String::new(),
        archived_at: None,
    }
}

#[test]
fn catalog_admission_account_only_passes_none_identity() {
    let credentials = CatalogCredentials::default();
    assert!(
        credentials.allows("p", "upstream", |_| panic!("no pool"), &|p, c, m| {
            assert_eq!((p, c, m), ("p", None, "upstream"));
            true
        })
    );
    assert!(!credentials.allows("p", "upstream", |_| true, &|_, _, _| false));
}

#[test]
fn catalog_admission_disabled_or_archived_pool_never_falls_back_to_account() {
    let credentials = CatalogCredentials {
        pools: HashMap::from([("p".into(), vec![])]),
    };
    assert!(!credentials.allows("p", "m", |_| true, &|_, _, _| panic!("must not fallback")));
}

#[test]
fn catalog_admission_uses_concrete_credential_and_actual_upstream() {
    let credentials = CatalogCredentials {
        pools: HashMap::from([(
            "p".into(),
            vec![credential("p", "plus"), credential("p", "pro")],
        )]),
    };
    assert!(credentials.allows("p", "actual", |_| true, &|p, c, m| {
        assert_eq!((p, m), ("p", "actual"));
        c == Some("pro")
    }));
    assert!(!credentials.allows("p", "actual", |_| true, &|_, _, _| false));
}

#[test]
fn catalog_admission_healthy_but_model_blocked_credential_is_not_a_witness() {
    let credentials = CatalogCredentials {
        pools: HashMap::from([(
            "p".into(),
            vec![credential("p", "plus"), credential("p", "pro")],
        )]),
    };
    assert!(
        !credentials.allows("p", "actual", |c| c.id == "plus", &|_, c, _| c
            == Some("pro"))
    );
}

#[test]
fn catalog_admission_another_providers_healthy_credential_cannot_rescue_row() {
    let credentials = CatalogCredentials {
        pools: HashMap::from([
            ("denied".into(), vec![credential("denied", "plus")]),
            ("other".into(), vec![credential("other", "pro")]),
        ]),
    };
    assert!(!credentials.allows("denied", "same-model", |_| true, &|p, _, _| p == "other"));
}
