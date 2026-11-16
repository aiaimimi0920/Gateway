//! Object staging cannot overwrite the predecessor when the PostgreSQL publication rolls back.
use super::{fixture::Fixture, journal, publication};
use crate::{console::refresh_grants::GrantPhase, object_storage::gateway_object_storage};

#[tokio::test]
#[ignore = "requires dedicated guarded PostgreSQL and isolated local objects; serialize tests"]
async fn accio_db_refresh_object_publication_rollback_preserves_old_pointer_bytes_and_result() {
    let f = Fixture::new().await;
    assert_eq!(
        std::env::var("AI_GATEWAY_OBJECT_STORAGE_DRIVER").as_deref(),
        Ok("local")
    );
    let root =
        std::path::PathBuf::from(std::env::var("AI_GATEWAY_OBJECT_STORAGE_LOCAL_DIR").unwrap());
    assert!(root.starts_with("C:/Users/Public/nas_home/AI/GameEditor/linshi"));
    let storage = gateway_object_storage().unwrap();
    let original = f.snapshot().await.payload;
    let old_key = format!(
        "accio-db-refresh-test/{}.json",
        uuid::Uuid::new_v4().simple()
    );
    storage.put_json(&old_key, &original).await.unwrap();
    let old_bytes = std::fs::read(root.join(&old_key)).unwrap();
    sqlx::query("update gateway_provider_credentials set payload_inline=null,payload_object_key=$2,storage_mode='r2' where id=$1")
        .bind(&f.credential).bind(&old_key).execute(&f.pool).await.unwrap();
    let (snapshot, sending) = f.sending().await;
    let result = journal::store_result(&f.pool, &sending, &Fixture::result(&snapshot))
        .await
        .unwrap();
    let staged = publication::staged_object_key(&result).unwrap();
    sqlx::raw_sql("create function reject_synthetic_publication() returns trigger language plpgsql as $$ begin raise exception 'synthetic_publication_fail'; end $$; create trigger reject_synthetic_publication before update on gateway_provider_refresh_grants for each row when ((new.record::jsonb->>'phase')='published') execute function reject_synthetic_publication();")
        .execute(&f.pool).await.unwrap();
    assert!(publication::publish_result(&f.pool, &result).await.is_err());
    assert_eq!(f.row().await["payload_object_key"], old_key);
    assert_eq!(std::fs::read(root.join(&old_key)).unwrap(), old_bytes);
    assert_eq!(storage.read_json(&old_key).await.unwrap(), original);
    assert_eq!(f.record(&sending).await.phase(), GrantPhase::Result);
    assert_eq!(
        storage.read_json(&staged).await.unwrap()["apiKey"],
        "synthetic-successor"
    );
    sqlx::query("drop trigger reject_synthetic_publication on gateway_provider_refresh_grants")
        .execute(&f.pool)
        .await
        .unwrap();
    let next = publication::publish_result(&f.pool, &result)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(next.storage_mode, "r2");
    assert_eq!(f.row().await["payload_object_key"], staged);
    assert_eq!(std::fs::read(root.join(&old_key)).unwrap(), old_bytes);
    assert_eq!(f.record(&sending).await.phase(), GrantPhase::Published);
    storage.delete_object(&old_key).await.unwrap();
    storage.delete_object(&staged).await.unwrap();
    f.finish().await;
}
