//! Synthetic SQLite only: never opens a user's data root or calls an upstream.
use super::*;
use crate::db::provider_credential_model_states::recovery::ModelHealthRecovery;
use crate::db::{CredentialModelStateFilters, RecordCredentialModelSuccessInput};

pub(super) fn identity(provider: &str) -> RecordCredentialModelSuccessInput {
    RecordCredentialModelSuccessInput {
        provider_account_id: provider.into(),
        provider_credential_id: Some("unchanged-account".into()),
        provider_credential_ref: None,
        protocol_profile: Some("unchanged-protocol".into()),
        model: "synthetic-model".into(),
    }
}

pub(super) async fn fixture() -> (std::path::PathBuf, LocalRuntime) {
    let root =
        std::env::temp_dir().join(format!("gateway-local-identity-{}", uuid::Uuid::new_v4()));
    let local = LocalRuntime::open(&root).await.unwrap();
    (root, local)
}

pub(super) async fn finish(root: std::path::PathBuf, local: LocalRuntime) {
    local.close().await;
    super::super::test_support::remove_test_root(&root).await;
}

#[tokio::test]
async fn old_completion_after_rename_does_not_recreate_old_state() {
    let (root, local) = fixture().await;
    let before = local.model_success(identity("old")).await.unwrap();
    let checkpoint = local.model_health_snapshot(identity("old")).await.unwrap();
    assert_eq!(rename(&local, "old", "new").await.unwrap(), 1);
    let row = local.model_success(identity("old")).await.unwrap();
    assert_eq!(row.provider_account_id, "new");
    assert_ne!(row.id, before.id);
    assert_eq!(row.provider_credential_id, before.provider_credential_id);
    assert_eq!(
        local
            .list_model_states(Default::default())
            .await
            .unwrap()
            .len(),
        1
    );
    assert_eq!(
        local
            .apply_model_health_snapshot(&checkpoint, None)
            .await
            .unwrap(),
        ModelHealthRecovery::StaleHealth
    );
    let filters = CredentialModelStateFilters {
        provider_account_id: Some("old".into()),
        ..Default::default()
    };
    assert_eq!(
        local.list_model_states(filters).await.unwrap()[0].id,
        row.id
    );
    assert_eq!(
        local
            .read_health_row(&before.id)
            .await
            .unwrap()
            .unwrap()
            .row
            .id,
        row.id
    );
    assert_eq!(rename(&local, "old", "new").await.unwrap(), 0);
    finish(root, local).await;
}

#[tokio::test]
async fn concurrent_old_new_completions_converge_and_survive_reopen() {
    let (root, local) = fixture().await;
    local.model_success(identity("old")).await.unwrap();
    // New identities are admitted only after the coordinator publishes the rename.
    let old = local.clone();
    let completing = tokio::spawn(async move {
        for _ in 0..12 {
            old.model_success(identity("old")).await.unwrap();
        }
    });
    rename(&local, "old", "new").await.unwrap();
    for _ in 0..12 {
        local.model_success(identity("new")).await.unwrap();
    }
    completing.await.unwrap();
    assert_eq!(
        local
            .list_model_states(Default::default())
            .await
            .unwrap()
            .len(),
        1
    );
    local.close().await;
    let local = LocalRuntime::open(&root).await.unwrap();
    assert_eq!(
        local
            .model_health_snapshot(identity("old"))
            .await
            .unwrap()
            .identity
            .provider_account_id,
        "new"
    );
    assert_eq!(
        local
            .model_success(identity("old"))
            .await
            .unwrap()
            .provider_account_id,
        "new"
    );
    let version: i64 = sqlx::query_scalar("PRAGMA user_version")
        .fetch_one(&local.pool)
        .await
        .unwrap();
    assert_eq!(version, 1);
    finish(root, local).await;
}

#[tokio::test]
async fn collision_chain_cycle_and_retarget_are_rejected_without_partial_updates() {
    let (root, local) = fixture().await;
    local.model_success(identity("old")).await.unwrap();
    local.model_success(identity("occupied")).await.unwrap();
    let before: Vec<(String, String)> =
        sqlx::query_as("SELECT id,payload FROM credential_model_states ORDER BY id")
            .fetch_all(&local.pool)
            .await
            .unwrap();
    assert!(rename(&local, "old", "occupied").await.is_err());
    let after: Vec<(String, String)> =
        sqlx::query_as("SELECT id,payload FROM credential_model_states ORDER BY id")
            .fetch_all(&local.pool)
            .await
            .unwrap();
    assert_eq!(before, after);
    assert_eq!(local.canonical_model_provider("old").await.unwrap(), "old");
    rename(&local, "old", "new").await.unwrap();
    for (old, new) in [
        ("old", "other"),
        ("new", "other"),
        ("other", "old"),
        ("new", "old"),
        ("old", "old"),
        ("old\nname", "new-name"),
        ("old-name", "new\nname"),
    ] {
        assert!(rename(&local, old, new).await.is_err());
    }
    finish(root, local).await;
}

#[tokio::test]
async fn blocked_health_is_preserved_and_old_id_reset_targets_same_row() {
    let (root, local) = fixture().await;
    let row = local.model_success(identity("old")).await.unwrap();
    let mut value = serde_json::to_value(row).unwrap();
    value["status"] = "blocked".into();
    value["failureCount"] = 7.into();
    sqlx::query("UPDATE credential_model_states SET payload=?")
        .bind(value.to_string())
        .execute(&local.pool)
        .await
        .unwrap();
    rename(&local, "old", "new").await.unwrap();
    let snapshot = local.model_health_snapshot(identity("old")).await.unwrap();
    assert!(snapshot.protected);
    let row = local
        .list_model_states(Default::default())
        .await
        .unwrap()
        .remove(0);
    assert_eq!(row.failure_count, 7);
    assert_eq!(row.updated_at, value["updatedAt"].as_str().unwrap());
    local.reset_provider_health("old").await.unwrap();
    assert!(
        !local
            .model_health_snapshot(identity("new"))
            .await
            .unwrap()
            .protected
    );
    finish(root, local).await;
}
