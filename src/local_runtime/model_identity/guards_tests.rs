//! Bounded admission and checkpoint invalidation for the storage-only rename step.
use super::tests::{finish, fixture, identity};
use super::*;

#[tokio::test]
async fn failure_fields_survive_rename_and_stale_undo_cannot_restore_old_identity() {
    let (root, local) = fixture().await;
    let input = identity("old");
    let failure = crate::db::RecordCredentialModelFailureInput {
        provider_account_id: input.provider_account_id,
        provider_credential_id: input.provider_credential_id,
        provider_credential_ref: input.provider_credential_ref,
        protocol_profile: input.protocol_profile,
        model: input.model,
        upstream_status: Some(429),
        upstream_retry_after_ms: Some(60_000),
        error_message: Some("synthetic rate limit".into()),
        classification: crate::provider_failure::classify_provider_failure(
            Some(429),
            None,
            Some("rate limit"),
        ),
    };
    let row = local.model_failure(failure.clone()).await.unwrap().unwrap();
    let checkpoint = local.read_health_row(&row.id).await.unwrap().unwrap();
    rename(&local, "old", "new").await.unwrap();
    let renamed = local.read_health_row(&row.id).await.unwrap().unwrap();
    let mut expected = serde_json::to_value(row).unwrap();
    expected["id"] = renamed.row.id.clone().into();
    expected["providerAccountId"] = "new".into();
    assert_eq!(expected, serde_json::to_value(&renamed.row).unwrap());
    local.restore_health_row(&checkpoint, None).await.unwrap();
    assert_eq!(
        local
            .read_health_row(&renamed.row.id)
            .await
            .unwrap()
            .unwrap()
            .version,
        renamed.version
    );
    let next = local.model_failure(failure).await.unwrap().unwrap();
    assert_eq!(next.provider_account_id, "new");
    assert_eq!(next.failure_count, renamed.row.failure_count + 1);
    let latest = local.read_health_row(&next.id).await.unwrap().unwrap();
    local
        .restore_health_row(&latest, Some(&renamed))
        .await
        .unwrap();
    let restored = local.read_health_row(&next.id).await.unwrap().unwrap().row;
    assert_eq!(restored.provider_account_id, "new");
    assert_eq!(restored.failure_count, renamed.row.failure_count);
    finish(root, local).await;
}

#[tokio::test]
async fn empty_state_rename_resolves_first_old_request_and_new_queries() {
    let (root, local) = fixture().await;
    assert_eq!(rename(&local, "old", "new").await.unwrap(), 0);
    let row = local.model_success(identity("old")).await.unwrap();
    let snapshot = local.model_health_snapshot(identity("new")).await.unwrap();
    assert_eq!(snapshot.identity.provider_account_id, "new");
    assert!(snapshot.version.is_some());
    assert_eq!(row.provider_account_id, "new");
    finish(root, local).await;
}

#[tokio::test]
async fn oversized_inventory_and_alias_capacity_fail_without_writes() {
    let (root, local) = fixture().await;
    sqlx::query("WITH RECURSIVE n(i) AS (SELECT 1 UNION ALL SELECT i+1 FROM n WHERE i<257)
        INSERT INTO credential_model_states(id,payload,updated) SELECT 'fixture-'||i,'{}','time' FROM n")
        .execute(&local.pool).await.unwrap();
    assert!(rename(&local, "old", "new").await.is_err());
    let aliases: i64 = sqlx::query_scalar("SELECT count(*) FROM model_provider_aliases")
        .fetch_one(&local.pool)
        .await
        .unwrap();
    assert_eq!(aliases, 0);
    finish(root, local).await;
    let (root, local) = fixture().await;
    sqlx::query(
        "WITH RECURSIVE n(i) AS (SELECT 1 UNION ALL SELECT i+1 FROM n WHERE i<256)
        INSERT INTO model_provider_aliases(alias,canonical) SELECT 'old-'||i,'new-'||i FROM n",
    )
    .execute(&local.pool)
    .await
    .unwrap();
    assert!(rename(&local, "old", "new").await.is_err());
    assert_eq!(local.canonical_model_provider("old").await.unwrap(), "old");
    finish(root, local).await;
}
