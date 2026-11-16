//! Material tickets survive routing-only replacement, never an actual authorization replacement.
use super::*;
use serde_json::json;

fn store() -> RouteConfigStore {
    RouteConfigStore::from_document(serde_json::from_value(json!({"providers":[{
        "id":"pool","base_url":"https://fixture.invalid/v1", "supported_models":["model"],
        "credentials":[{"id":"account","api_key":"old-access","refresh_token":"old-refresh",
            "refresh_endpoint":"https://fixture.invalid/token", "expires_at":"2000-01-01T00:00:00Z"}]
    }]})).unwrap()).unwrap()
}
fn owner(store: &RouteConfigStore) -> Arc<TokenRefreshState> {
    store
        .snapshot()
        .oauth_refresh_target("pool", "account")
        .unwrap()
        .1
        .refresh_config
        .clone()
        .unwrap()
}
fn publish(store: &RouteConfigStore, document: RouteConfigYaml, actor: RevisionActor) {
    let current = store.snapshot();
    let validated = validate_route_document(document).unwrap();
    let revision = RevisionMetadata::from_validated(
        current.revision().sequence() + 1,
        Some(current.revision().id().to_owned()),
        actor,
        OffsetDateTime::now_utc(),
        None,
        &validated,
    );
    store.replace_validated(validated, revision).unwrap();
}

#[test]
fn retired_any_policy_snapshot_cannot_select_fresh_shared_material_or_start_an_exchange() {
    let store = store();
    let snapshot = store.snapshot();
    let old = owner(&store);
    let attempt = old
        .begin_refresh(std::time::Duration::MAX, "old-access")
        .unwrap();
    let mut document = snapshot.document().clone();
    document.providers[0].credential_pool_auth_policy = CredentialPoolAuthPolicy::OAuthOnly;
    store.replace_document(document).unwrap();
    let current = owner(&store);
    assert!(!Arc::ptr_eq(&old, &current));
    let generation = attempt
        .apply(
            "new-access".into(),
            Some("new-refresh".into()),
            3600,
            future_rfc3339_after_secs(3600),
            None,
            None,
            None,
        )
        .unwrap();
    assert!(current.authorization_is_current(&old, generation));
    assert!(old
        .begin_refresh(std::time::Duration::MAX, "old-access")
        .is_none());
    assert!(old.dispatch_readiness().is_err());
    current.dispatch_readiness().unwrap();
    for model in ["model", "legacy-unsupported-model"] {
        assert!(snapshot.resolve_candidates(Some(model)).is_empty());
    }
    assert_eq!(
        store.resolve_candidates(Some("model"))[0].payload.api_key,
        "new-access"
    );
    assert!(attempt
        .apply(
            "stale-second-result".into(),
            None,
            60,
            "stale".into(),
            None,
            None,
            None,
        )
        .is_none());
    attempt.record_failure(true);
    assert!(
        !current
            .authorization
            .material
            .lock()
            .reauthorization_required
    );
}

#[test]
fn true_authorization_replacement_revokes_ticket_and_has_an_independent_single_flight() {
    for mode in ["access", "endpoint", "audience", "header"] {
        let store = store();
        let old = owner(&store);
        let attempt = old
            .begin_refresh(std::time::Duration::MAX, "old-access")
            .unwrap();
        let mut document = store.snapshot().document().clone();
        match mode {
            "access" => {
                document.providers[0].credentials[0].api_key =
                    Some("explicit-reauthorization".into())
            }
            "endpoint" => {
                document.providers[0].credentials[0].refresh_endpoint =
                    Some("https://other.invalid/token".into())
            }
            "audience" => document.providers[0].base_url = "https://other.invalid/v1".into(),
            _ => {
                document.providers[0].credentials[0]
                    .headers
                    .insert("X-Account".into(), "new-account".into());
            }
        }
        store.replace_document(document).unwrap();
        let current = owner(&store);
        assert!(current
            .begin_refresh(std::time::Duration::MAX, "new-access")
            .is_some());
        assert!(attempt
            .apply(
                "late-old-token".into(),
                None,
                60,
                "late".into(),
                None,
                None,
                None
            )
            .is_none());
        attempt.record_failure(true);
        assert!(
            !current
                .authorization
                .material
                .lock()
                .reauthorization_required
        );
        assert!(!old.is_active());
    }
}

#[test]
fn exact_current_rotation_advances_anchor_only_for_normal_automation_publication() {
    for actor in [
        RevisionActor::CredentialPoolAutomation,
        RevisionActor::ManagementToken,
    ] {
        let store = store();
        let old = owner(&store);
        let attempt = old
            .begin_refresh(std::time::Duration::MAX, "old-access")
            .unwrap();
        let expiry = future_rfc3339_after_secs(3600);
        let generation = attempt
            .apply(
                "rotated-access".into(),
                Some("rotated-refresh".into()),
                3600,
                expiry.clone(),
                None,
                None,
                None,
            )
            .unwrap();
        let mut document = store.snapshot().document().clone();
        let credential = &mut document.providers[0].credentials[0];
        credential.api_key = Some("rotated-access".into());
        credential.refresh_token = Some("rotated-refresh".into());
        credential.expires_at = Some(expiry);
        credential.token_expires_in_secs = Some(3600);
        publish(&store, document, actor);
        let current = owner(&store);
        let same = actor == RevisionActor::CredentialPoolAutomation;
        assert_eq!(Arc::ptr_eq(&old, &current), same);
        assert_eq!(current.authorization_is_current(&old, generation), same);
        assert_eq!(old.is_active(), same);
    }
}

#[test]
fn automation_does_not_mix_current_rotation_with_changed_endpoint_or_headers() {
    for mode in ["endpoint", "header"] {
        let store = store();
        let old = owner(&store);
        let attempt = old
            .begin_refresh(std::time::Duration::MAX, "old-access")
            .unwrap();
        let expiry = future_rfc3339_after_secs(3600);
        attempt
            .apply(
                "rotated-access".into(),
                None,
                3600,
                expiry.clone(),
                None,
                None,
                None,
            )
            .unwrap();
        let mut document = store.snapshot().document().clone();
        let credential = &mut document.providers[0].credentials[0];
        credential.api_key = Some("rotated-access".into());
        credential.expires_at = Some(expiry);
        credential.token_expires_in_secs = Some(3600);
        if mode == "endpoint" {
            credential.refresh_endpoint = Some("https://other.invalid/token".into());
        } else {
            credential
                .headers
                .insert("X-Account".into(), "other-account".into());
        }
        publish(&store, document, RevisionActor::CredentialPoolAutomation);
        assert!(!old.is_active());
        assert!(!Arc::ptr_eq(&old, &owner(&store)));
    }
}
