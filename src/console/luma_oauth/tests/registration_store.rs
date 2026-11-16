//! Storage corruption and permanent claims fail closed, without changing a live database.
use super::fixture::{fixture, redirect};
use crate::console::{
    luma_oauth::registration_storage::persistence, persistence::LumaRegistrationRecord,
};

fn record(owner: &str) -> LumaRegistrationRecord {
    LumaRegistrationRecord {
        offline_access: false,
        id: uuid::Uuid::new_v4().to_string(),
        owner: owner.into(),
        provider_id: "luma-mcp".into(),
        expected_revision: "revision".into(),
        redirect_uri: "http://127.0.0.1:12345/callback".into(),
        client_id: None,
    }
}

#[tokio::test]
async fn luma_registration_recovered_unknown_observes_late_durable_success() {
    use crate::console::luma_oauth::{RegistrationStatus, Registry};
    let f = fixture(&redirect(), false);
    let store = persistence(&f.state).unwrap();
    let revision = f.state.route_config.snapshot().revision().id().to_string();
    let mut accepted = record("owner");
    accepted.expected_revision = revision.clone();
    {
        let guard = store.try_writer_lock().unwrap();
        assert!(store
            .claim_luma_registration_locked(&guard, &accepted)
            .unwrap());
    }
    let restarted = Registry::default();
    let receipt = restarted
        .prepare_registration(&f.state, "owner", "luma-mcp", &revision)
        .await
        .unwrap();
    assert!(receipt.view().status == RegistrationStatus::Unconfirmed);
    {
        let guard = store.try_writer_lock().unwrap();
        store
            .finish_luma_registration_locked(&guard, &accepted, "late-client".into())
            .unwrap();
    }
    let updated = restarted
        .load_registration(&f.state, &accepted.id, "owner")
        .await
        .unwrap();
    assert!(std::sync::Arc::ptr_eq(&receipt, &updated));
    assert_eq!(updated.view().client_id.as_deref(), Some("late-client"));
    assert!(updated.view().status == RegistrationStatus::Registered);
    let prepared = restarted
        .prepare_registration(&f.state, "owner", "luma-mcp", &revision)
        .await
        .unwrap();
    assert!(prepared.view().status == RegistrationStatus::Registered);
    restarted.join().await;
}

#[tokio::test]
async fn luma_registration_store_preserves_identity_and_permanent_capacity() {
    let f = fixture(&redirect(), false);
    let store = persistence(&f.state).unwrap();
    let guard = store.try_writer_lock().unwrap();
    let first = record("owner");
    assert!(store
        .claim_luma_registration_locked(&guard, &first)
        .unwrap());
    assert!(!store
        .claim_luma_registration_locked(&guard, &record("owner"))
        .unwrap());
    store
        .finish_luma_registration_locked(&guard, &first, "client".into())
        .unwrap();
    store
        .finish_luma_registration_locked(&guard, &first, "client".into())
        .unwrap();
    assert!(store
        .finish_luma_registration_locked(&guard, &first, "different-client".into())
        .is_err());
    for i in 1..16 {
        assert!(store
            .claim_luma_registration_locked(&guard, &record(&format!("owner-{i}")))
            .unwrap());
    }
    assert!(store
        .claim_luma_registration_locked(&guard, &record("overflow"))
        .is_err());
    let records = store.load_luma_registrations_locked(&guard).unwrap();
    assert_eq!(records.len(), 16);
    assert_eq!(records[0].id, first.id);
    assert_eq!(records[0].client_id.as_deref(), Some("client"));
}

#[tokio::test]
async fn luma_registration_corrupt_store_never_becomes_an_empty_registry() {
    let f = fixture(&redirect(), false);
    let store = persistence(&f.state).unwrap();
    let revision = f.state.route_config.snapshot().revision().id().to_string();
    let path = store.state_root().join("luma-client-registrations.json");
    let valid = record("owner");
    let mut bad_redirect = valid.clone();
    bad_redirect.redirect_uri = "https://attacker.invalid/callback".into();
    for bytes in [
        b"broken".to_vec(),
        serde_json::to_vec(&serde_json::json!({"version":2,"registrations":[]})).unwrap(),
        serde_json::to_vec(&serde_json::json!({"version":1,"registrations":[valid.clone(),valid]}))
            .unwrap(),
        serde_json::to_vec(&serde_json::json!({"version":1,"registrations":[bad_redirect]}))
            .unwrap(),
    ] {
        std::fs::write(&path, &bytes).unwrap();
        assert!(f
            .state
            .lifecycle
            .luma_oauth
            .prepare_registration(&f.state, "owner", "luma-mcp", &revision)
            .await
            .is_err());
        assert_eq!(std::fs::read(&path).unwrap(), bytes);
    }
    f.state.lifecycle.luma_oauth.join().await;
}
