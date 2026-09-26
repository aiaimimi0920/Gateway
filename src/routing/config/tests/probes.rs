use super::*;

#[test]
fn scheduled_probe_targets_include_provider_default_and_pooled_credentials() {
    let yaml = r#"
providers:
  - id: default-provider
    base_url: "https://default.example/v1"
    api_key: "default-key"
    scheduled_probe_enabled: true
    scheduled_probe_interval_minutes: 30
  - id: pooled-provider
    base_url: "https://pooled.example/v1"
    api_key: "provider-key"
    credentials:
      - id: scheduled-account
        api_key: "scheduled-key"
        scheduled_probe_enabled: true
        scheduled_probe_interval_minutes: 15
      - id: unscheduled-account
        api_key: "unscheduled-key"
model_routes: []
aliases: {}
"#;
    let store = make_store_from_yaml(yaml);
    let snapshot = store.snapshot();
    let targets = snapshot.scheduled_credential_probe_targets();

    assert_eq!(targets.len(), 2);
    assert!(targets.iter().any(|candidate| {
        candidate.target.credential_id == "default-provider::default"
            && candidate.target.provider_id == "default-provider"
            && candidate.interval_minutes == 30
    }));
    assert!(targets.iter().any(|candidate| {
        candidate.target.credential_id == "scheduled-account"
            && candidate.target.provider_id == "pooled-provider"
            && candidate.target.payload.credential_id.as_deref() == Some("scheduled-account")
            && candidate.interval_minutes == 15
    }));
    assert!(!targets
        .iter()
        .any(|candidate| candidate.target.credential_id == "unscheduled-account"));
}

#[test]
fn provider_probe_targets_include_every_pooled_credential_and_provider_default() {
    let store = make_store_from_yaml(
        r#"
providers:
  - id: default-provider
    base_url: "https://default.example/v1"
    api_key: "default-key"
  - id: pooled-provider
    base_url: "https://pooled.example/v1"
    api_key: "provider-key"
    credentials:
      - id: enabled-account
        api_key: "enabled-key"
      - id: disabled-account
        api_key: "disabled-key"
        enabled: false
model_routes: []
aliases: {}
"#,
    );
    let snapshot = store.snapshot();

    let default_targets = snapshot
        .credential_probe_targets_for_provider("default-provider")
        .expect("default provider targets");
    assert_eq!(default_targets.len(), 1);
    assert_eq!(
        default_targets[0].credential_id,
        "default-provider::default"
    );

    let pooled_targets = snapshot
        .credential_probe_targets_for_provider("pooled-provider")
        .expect("pooled provider targets");
    assert_eq!(pooled_targets.len(), 2);
    assert!(pooled_targets.iter().any(|target| {
        target.credential_id == "enabled-account"
            && target.enabled
            && target.payload.credential_id.as_deref() == Some("enabled-account")
    }));
    assert!(pooled_targets.iter().any(|target| {
        target.credential_id == "disabled-account"
            && !target.enabled
            && target.payload.credential_id.as_deref() == Some("disabled-account")
    }));
    assert!(snapshot
        .credential_probe_targets_for_provider("missing-provider")
        .is_none());
}

#[test]
fn credential_probe_target_selects_explicit_global_credential_id() {
    let store = make_store_from_yaml(
        r#"
providers:
  - id: openai-pool
    adapter: openai_compatible
    base_url: "https://provider.example/v1"
    api_key: "provider-key"
    credentials:
      - id: credential-live
        base_url: "https://credential.example/v1"
        api_key: "credential-secret"
        enabled: false
model_routes: []
aliases: {}
"#,
    );

    let target = store
        .snapshot()
        .select_credential_probe_target("credential-live")
        .expect("explicit credential target");
    assert_eq!(target.credential_id, "credential-live");
    assert_eq!(target.provider_id, "openai-pool");
    assert!(!target.enabled);
    assert_eq!(target.payload.base_url, "https://credential.example/v1");
    assert_eq!(target.payload.api_key, "credential-secret");
}

#[test]
fn credential_probe_target_selects_provider_default_identity() {
    let store = make_store_from_yaml(
        r#"
providers:
  - id: default-provider
    adapter: openai_compatible
    base_url: "https://default.example/v1"
    api_key: "default-secret"
model_routes: []
aliases: {}
"#,
    );

    let snapshot = store.snapshot();
    let target = snapshot
        .select_credential_probe_target("default-provider::default")
        .expect("provider default target");
    assert_eq!(target.credential_id, "default-provider::default");
    assert_eq!(target.provider_id, "default-provider");
    assert!(target.enabled);
    assert_eq!(target.payload.api_key, "default-secret");
    assert!(snapshot
        .select_credential_probe_target("missing-credential")
        .is_none());
}

#[test]
fn credential_probe_target_trims_raw_id_for_ui_lookup_and_response() {
    let store = make_store_from_yaml(
        r#"
providers:
  - id: openai-pool
    adapter: openai_compatible
    base_url: "https://provider.example/v1"
    api_key: "provider-token"
    credentials:
      - id: "  credential-live  "
        api_key: "credential-token"
model_routes: []
aliases: {}
"#,
    );

    let target = store
        .snapshot()
        .select_credential_probe_target("credential-live")
        .expect("trimmed credential target");
    assert_eq!(target.credential_id, "credential-live");
    assert_eq!(
        target.payload.credential_id.as_deref(),
        Some("credential-live")
    );
}

#[test]
fn credential_probe_target_uses_latest_oauth_access_token() {
    let store = make_store_from_yaml(
        r#"
providers:
  - id: oauth-pool
    adapter: openai_compatible
    base_url: "https://provider.example/v1"
    api_key: "initial-token"
    credentials:
      - id: oauth-live
        api_key: "credential-token"
        refresh_token: "refresh-token"
        refresh_endpoint: "https://provider.example/oauth/token"
model_routes: []
aliases: {}
"#,
    );

    let snapshot = store.snapshot();
    let provider = snapshot
        .get_providers()
        .into_iter()
        .find(|provider| provider.id == "oauth-pool")
        .expect("oauth provider");
    let refresh_state = provider
        .credential_pool
        .first()
        .and_then(|credential| credential.refresh_config.as_ref())
        .expect("refresh state")
        .clone();
    *refresh_state.api_key_override.lock() = Some("latest-access-token".to_string());

    let target = snapshot
        .select_credential_probe_target("oauth-live")
        .expect("oauth credential target");
    assert_eq!(target.payload.api_key, "latest-access-token");
}
