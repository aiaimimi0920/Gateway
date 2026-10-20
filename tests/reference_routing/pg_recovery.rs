//! Guarded PG fixture only: a waiting CAS must recheck the committed physical row version.
use neuro_gateway::db::{
    self,
    provider_credential_model_states::recovery::{self, ModelHealthRecovery},
};
use sqlx::{PgConnection, PgPool};
use std::time::Duration;

pub async fn waiting_writer_contract(pool: &PgPool, admin: &mut PgConnection) {
    let identity = db::RecordCredentialModelSuccessInput {
        provider_account_id: "pg-cas-writer".into(),
        provider_credential_id: None,
        provider_credential_ref: Some("one".into()),
        protocol_profile: Some("openai_compatible_generic".into()),
        model: "model".into(),
    };
    let row = db::record_provider_credential_model_success(pool, identity.clone())
        .await
        .unwrap();
    let expected = recovery::snapshot(pool, identity.clone()).await.unwrap();
    let mut writer = pool.begin().await.unwrap();
    let writer_pid: i32 = sqlx::query_scalar("SELECT pg_backend_pid()")
        .fetch_one(&mut *writer)
        .await
        .unwrap();
    // Same timestamp and same visible fields, but a different physical version.
    sqlx::query(
        "UPDATE gateway_provider_credential_model_states SET status = status WHERE id = $1",
    )
    .bind(&row.id)
    .execute(&mut *writer)
    .await
    .unwrap();
    let reader_pool = pool.clone();
    let mut reader =
        tokio::spawn(async move { recovery::apply(&reader_pool, &expected, None).await });
    let waiting =
        tokio::time::timeout(Duration::from_secs(10), async {
            loop {
                let blocked: bool = sqlx::query_scalar(
                "SELECT EXISTS (SELECT 1 FROM pg_stat_activity WHERE datname = current_database()
                 AND $1 = ANY(pg_blocking_pids(pid)))")
                .bind(writer_pid).fetch_one(&mut *admin).await.unwrap();
                if blocked {
                    break;
                }
                tokio::time::sleep(Duration::from_millis(20)).await;
            }
        })
        .await;
    if waiting.is_err() {
        reader.abort();
        let _ = reader.await;
        writer.rollback().await.unwrap();
        panic!("conditional recovery did not wait for the owned row writer");
    }
    writer.commit().await.unwrap();
    let result = tokio::time::timeout(Duration::from_secs(10), &mut reader).await;
    if result.is_err() {
        reader.abort();
        let _ = reader.await;
        panic!("conditional recovery did not complete after writer commit");
    }
    assert_eq!(
        result.unwrap().unwrap().unwrap(),
        ModelHealthRecovery::StaleHealth
    );
    let current = db::list_provider_credential_model_states(
        pool,
        db::CredentialModelStateFilters {
            provider_account_id: Some(identity.provider_account_id),
            ..Default::default()
        },
    )
    .await
    .unwrap();
    assert_eq!(current.len(), 1);
    assert_eq!(current[0].updated_at, row.updated_at);
}
