use super::*;

#[test]
fn replaced_provider_retires_old_refresh_state_and_drops_delayed_result() {
    let old_document = oauth_fixture("https://old.example/token", "old-access");
    let new_document = oauth_fixture("https://new.example/token", "new-access");
    let store = RouteConfigStore::from_document(old_document).expect("old fixture");
    let old_state = store
        .snapshot()
        .compiled()
        .providers
        .first()
        .and_then(|provider| provider.credential_pool.first())
        .and_then(|credential| credential.refresh_config.clone())
        .expect("old refresh state");
    let attempt = old_state
        .refresh_snapshot(std::time::Duration::MAX)
        .expect("refresh attempt");

    let validated =
        crate::console::document::validate_route_document(new_document).expect("new fixture");
    let revision = RevisionMetadata::from_validated(
        store.snapshot().revision().sequence() + 1,
        Some(store.snapshot().revision().id().to_string()),
        RevisionActor::ManagementToken,
        OffsetDateTime::now_utc(),
        None,
        &validated,
    );
    store
        .replace_validated(validated, revision)
        .expect("replacement");

    assert!(!old_state.is_active());
    assert!(old_state
        .apply_refresh_if_active(
            attempt.0,
            "stale-access".to_string(),
            Some("stale-refresh".to_string()),
            60,
            "stale-expiry".to_string(),
        )
        .is_none());
    let snapshot = store.snapshot();
    let new_state = snapshot
        .compiled()
        .providers
        .first()
        .and_then(|provider| provider.credential_pool.first())
        .and_then(|credential| credential.refresh_config.as_ref())
        .expect("new refresh state");
    assert!(!std::sync::Arc::ptr_eq(&old_state, new_state));
    assert_eq!(new_state.runtime_override().0, None);
}

#[test]
fn unchanged_provider_reuses_refresh_state_and_generation() {
    let old_document = oauth_fixture("https://same.example/token", "old-access");
    let mut new_document = oauth_fixture("https://same.example/token", "old-access");
    new_document
        .aliases
        .insert("oauth-alias".to_string(), "oauth-model".to_string());
    let store = RouteConfigStore::from_document(old_document).expect("old fixture");
    let old_state = store
        .snapshot()
        .compiled()
        .providers
        .first()
        .and_then(|provider| provider.credential_pool.first())
        .and_then(|credential| credential.refresh_config.clone())
        .expect("old refresh state");
    let attempt = old_state
        .refresh_snapshot(std::time::Duration::MAX)
        .expect("refresh attempt");
    let generation = old_state
        .apply_refresh_if_active(
            attempt.0,
            "refreshed-access".to_string(),
            Some("refreshed-refresh".to_string()),
            60,
            "refreshed-expiry".to_string(),
        )
        .expect("first apply");
    assert!(old_state.is_generation_active(generation));
    assert!(old_state
        .apply_refresh_if_active(
            attempt.0,
            "stale-second-attempt".to_string(),
            None,
            60,
            "stale".to_string(),
        )
        .is_none());

    let validated =
        crate::console::document::validate_route_document(new_document).expect("new fixture");
    let revision = RevisionMetadata::from_validated(
        store.snapshot().revision().sequence() + 1,
        Some(store.snapshot().revision().id().to_string()),
        RevisionActor::ManagementToken,
        OffsetDateTime::now_utc(),
        None,
        &validated,
    );
    store
        .replace_validated(validated, revision)
        .expect("replacement");
    let snapshot = store.snapshot();
    let new_state = snapshot
        .compiled()
        .providers
        .first()
        .and_then(|provider| provider.credential_pool.first())
        .and_then(|credential| credential.refresh_config.as_ref())
        .expect("new refresh state");
    assert!(std::sync::Arc::ptr_eq(&old_state, new_state));
    assert_eq!(
        new_state.runtime_override().0.as_deref(),
        Some("refreshed-access")
    );
}

#[test]
fn legacy_anonymous_refresh_state_survives_first_strict_save() {
    let mut legacy_document = oauth_fixture("https://same.example/token", "old-access");
    legacy_document.providers[0].credentials[0].id = None;
    let mut replacement = legacy_document.clone();
    replacement
        .aliases
        .insert("oauth-alias".to_string(), "oauth-model".to_string());
    let store = RouteConfigStore::from_unchecked_document(
        legacy_document,
        ActiveConfigSource::Yaml,
        RevisionActor::Bootstrap,
        0,
        None,
        None,
    )
    .expect("legacy fixture");
    let old_state = store
        .snapshot()
        .compiled()
        .providers
        .first()
        .and_then(|provider| provider.credential_pool.first())
        .and_then(|credential| credential.refresh_config.clone())
        .expect("old refresh state");

    let validated =
        crate::console::document::validate_route_document(replacement).expect("strict replacement");
    let revision = RevisionMetadata::from_validated(
        store.snapshot().revision().sequence() + 1,
        Some(store.snapshot().revision().id().to_string()),
        RevisionActor::ManagementToken,
        OffsetDateTime::now_utc(),
        None,
        &validated,
    );
    store
        .replace_validated(validated, revision)
        .expect("replacement");
    let snapshot = store.snapshot();
    let new_state = snapshot
        .compiled()
        .providers
        .first()
        .and_then(|provider| provider.credential_pool.first())
        .and_then(|credential| credential.refresh_config.as_ref())
        .expect("new refresh state");

    assert!(std::sync::Arc::ptr_eq(&old_state, new_state));
}

#[test]
fn huge_refresh_lifetime_is_capped_without_clock_panic() {
    assert_eq!(
        effective_refresh_lifetime_secs(u64::MAX),
        100 * 365 * 24 * 60 * 60
    );
    let deadline = safe_refresh_deadline(u64::MAX);
    assert!(deadline >= std::time::Instant::now());
    assert_eq!(
        future_rfc3339_after_secs(effective_refresh_lifetime_secs(u64::MAX)).len(),
        24
    );
}
