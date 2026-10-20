//! Old payloads remain readable; CAS tokens advance even under wall-clock rollback.
use super::*;

async fn fixture() -> (
    std::path::PathBuf,
    LocalRuntime,
    RecordCredentialModelSuccessInput,
) {
    let root = std::env::temp_dir().join(format!(
        "gateway-local-health-time-{}",
        uuid::Uuid::new_v4()
    ));
    let local = LocalRuntime::open(&root).await.unwrap();
    let input = RecordCredentialModelSuccessInput {
        provider_account_id: "synthetic".into(),
        provider_credential_id: None,
        provider_credential_ref: Some("one".into()),
        protocol_profile: Some("openai_compatible_generic".into()),
        model: "model".into(),
    };
    (root, local, input)
}

async fn set_timestamp(
    local: &LocalRuntime,
    row: GatewayProviderCredentialModelStateView,
    timestamp: &str,
) {
    let id = row.id.clone();
    let mut payload = serde_json::to_value(row).unwrap();
    payload["updatedAt"] = serde_json::json!(timestamp);
    sqlx::query("UPDATE credential_model_states SET payload = ? WHERE id = ?")
        .bind(payload.to_string())
        .bind(id)
        .execute(&local.pool)
        .await
        .unwrap();
}

#[tokio::test]
async fn local_model_clock_rollback_preserves_strict_token_progress_without_a_schema_change() {
    let (root, local, input) = fixture().await;
    let initial = local.model_success(input.clone()).await.unwrap();
    set_timestamp(&local, initial, "2099-01-01T00:00:00Z").await;
    let expected = local.model_health_snapshot(input.clone()).await.unwrap();
    let row = local.model_success(input.clone()).await.unwrap();
    let next = OffsetDateTime::parse(&row.updated_at, &Rfc3339).unwrap();
    assert!(next > OffsetDateTime::parse("2099-01-01T00:00:00Z", &Rfc3339).unwrap());
    assert_eq!(
        local
            .apply_model_health_snapshot(&expected, None)
            .await
            .unwrap(),
        crate::db::provider_credential_model_states::recovery::ModelHealthRecovery::StaleHealth
    );
    let version: i64 = sqlx::query_scalar("PRAGMA user_version")
        .fetch_one(&local.pool)
        .await
        .unwrap();
    assert_eq!(version, 1);
    local.close().await;
    crate::local_runtime::test_support::remove_test_root(&root).await;
}

#[tokio::test]
async fn maximum_persisted_model_timestamp_fails_closed_instead_of_panicking() {
    let (root, local, input) = fixture().await;
    let initial = local.model_success(input.clone()).await.unwrap();
    set_timestamp(&local, initial, "9999-12-31T23:59:59.999999999Z").await;
    let expected = local.model_health_snapshot(input.clone()).await.unwrap();
    assert!(local
        .apply_model_health_snapshot(&expected, None)
        .await
        .is_err());
    assert_eq!(
        local.list_model_states(Default::default()).await.unwrap()[0].updated_at,
        "9999-12-31T23:59:59.999999999Z"
    );
    local.close().await;
    crate::local_runtime::test_support::remove_test_root(&root).await;
}
