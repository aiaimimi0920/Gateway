use super::*;

fn provider(id: &str) -> GatewayProviderRouteRow {
    GatewayProviderRouteRow {
        id: id.into(),
        label: id.into(),
        adapter: "openai_compatible".into(),
        protocol_family: "openai".into(),
        protocol_profile: "openai".into(),
        cooldown_until: None,
        failure_count: 0,
        execution_mode: "direct_http".into(),
        endpoint_execution_modes: None,
        payload_inline: None,
        payload_object_key: None,
    }
}

fn alias(provider: &str, model: &str, upstream: &str) -> GatewayModelAliasRow {
    GatewayModelAliasRow {
        id: model.into(),
        project_id: None,
        scope_type: "global".into(),
        alias: model.into(),
        provider_account_id: provider.into(),
        upstream_model: Some(upstream.into()),
        priority: 0,
        weight: 1,
        enabled: true,
        created_at: OffsetDateTime::UNIX_EPOCH,
        updated_at: OffsetDateTime::UNIX_EPOCH,
    }
}

fn capability(provider: &str, model: &str, upstream: Option<&str>) -> ProviderCapabilityModelRow {
    ProviderCapabilityModelRow {
        provider_account_id: provider.into(),
        model_code: model.into(),
        upstream_model: upstream.map(str::to_string),
    }
}

#[test]
fn catalog_admission_project_filters_alias_capability_and_default_models() {
    let models = collect_catalog_ids(
        None,
        &[alias("p", "alias", "actual")],
        &[provider("p")],
        &[capability("p", "mapped", Some("actual"))],
        &HashMap::from([("p".into(), Some("actual".into()))]),
        &|p, _, actual| {
            assert_eq!((p, actual), ("p", "actual"));
            false
        },
    );
    assert!(models.is_empty());
}

#[test]
fn catalog_admission_project_unauthorized_or_inactive_alias_never_leaks() {
    let policy = GatewayRoutePolicyConfig {
        allowed_provider_account_ids: Some(vec!["allowed".into()]),
        ..Default::default()
    };
    let models = collect_catalog_ids(
        Some(&policy),
        &[
            alias("denied", "denied-alias", "native"),
            alias("inactive", "inactive-alias", "native"),
        ],
        &[provider("allowed"), provider("denied")],
        &[capability("allowed", "native", None)],
        &HashMap::new(),
        &|_, _, _| true,
    );
    assert_eq!(models, vec!["native"]);
}

#[test]
fn catalog_admission_project_denied_alias_does_not_shadow_allowed_native() {
    let models = collect_catalog_ids(
        None,
        &[alias("p", "alias", "native")],
        &[provider("p")],
        &[capability("p", "native", None)],
        &HashMap::new(),
        &|_, model, _| model != "alias",
    );
    assert_eq!(models, vec!["native"]);
}

#[test]
fn catalog_admission_project_shadowing_is_provider_scoped() {
    let models = collect_catalog_ids(
        None,
        &[alias("p1", "alias", "native")],
        &[provider("p1"), provider("p2")],
        &[
            capability("p1", "native", None),
            capability("p2", "native", None),
        ],
        &HashMap::new(),
        &|_, _, _| true,
    );
    assert_eq!(models, vec!["alias", "native"]);
}

#[test]
fn catalog_admission_project_same_named_healthy_other_provider_cannot_rescue_mapping() {
    let models = collect_catalog_ids(
        None,
        &[],
        &[provider("p1"), provider("p2")],
        &[
            capability("p1", "visible", Some("native")),
            capability("p2", "native", None),
        ],
        &HashMap::new(),
        &|provider, _, _| provider == "p2",
    );
    assert_eq!(models, vec!["native"]);
}

#[test]
fn catalog_admission_project_model_policy_precedes_health_and_shadowing() {
    let policy = GatewayRoutePolicyConfig {
        blocked_model_ids: Some(vec!["alias".into()]),
        ..Default::default()
    };
    let models = collect_catalog_ids(
        Some(&policy),
        &[alias("p", "alias", "native")],
        &[provider("p")],
        &[capability("p", "native", None)],
        &HashMap::new(),
        &|_, _, _| true,
    );
    assert_eq!(models, vec!["native"]);
}

#[test]
fn catalog_admission_project_repeated_directory_reads_do_not_select_a_route() {
    let providers = [provider("p")];
    let aliases = [alias("p", "alias", "native")];
    let capabilities = [capability("p", "native", None)];
    let read = || {
        collect_catalog_ids(
            None,
            &aliases,
            &providers,
            &capabilities,
            &HashMap::new(),
            &|_, _, _| true,
        )
    };
    assert_eq!(read(), vec!["alias"]);
    assert_eq!(read(), read());
}
