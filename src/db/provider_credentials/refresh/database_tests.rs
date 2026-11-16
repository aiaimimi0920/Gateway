//! Real PostgreSQL proves permanent admission and exact authority/publication CAS, not upstream wire.
use super::{fixture::Fixture, journal, publication};
use crate::console::refresh_grants::{GrantPhase, GrantReservation};
use serde_json::json;

#[tokio::test]
#[ignore = "requires dedicated guarded PostgreSQL; serialize tests"]
async fn accio_db_refresh_concurrent_and_restarted_admission_never_reserves_twice() {
    let f = Fixture::new().await;
    let snapshot = f.snapshot().await;
    let (left, right) = tokio::join!(
        journal::reserve(&f.pool, &snapshot),
        journal::reserve(&f.pool, &snapshot)
    );
    let (left, right) = (left.unwrap(), right.unwrap());
    assert!(matches!(
        (&left, &right),
        (GrantReservation::Reserved(_), GrantReservation::Existing(_))
            | (GrantReservation::Existing(_), GrantReservation::Reserved(_))
    ));
    let restarted = f.snapshot().await;
    let GrantReservation::Existing(record) = journal::reserve(&f.pool, &restarted).await.unwrap()
    else {
        panic!("Sending is permanent")
    };
    assert_eq!(record.phase(), GrantPhase::Sending);
    // A duplicate credential with the same token family cannot acquire a second send.
    let duplicate = format!("duplicate-{}", uuid::Uuid::new_v4().simple());
    sqlx::query("insert into gateway_provider_credentials(id,provider_account_id,label,payload_inline,source_kind,sync_mode) select $2,provider_account_id,'duplicate',payload_inline,'manual','manual' from gateway_provider_credentials where id=$1")
        .bind(&f.credential).bind(&duplicate).execute(&f.pool).await.unwrap();
    let duplicate = journal::read_snapshot(&f.pool, &duplicate)
        .await
        .unwrap()
        .unwrap();
    assert!(matches!(
        journal::reserve(&f.pool, &duplicate).await.unwrap(),
        GrantReservation::Blocked
    ));
    f.finish().await;
}

#[tokio::test]
#[ignore = "requires dedicated guarded PostgreSQL; serialize tests"]
async fn accio_db_refresh_result_survives_deletion_and_recreated_authority_cannot_replay() {
    let f = Fixture::new().await;
    let (snapshot, sending) = f.sending().await;
    let original = f.row().await;
    sqlx::query("delete from gateway_provider_credentials where id=$1")
        .bind(&f.credential)
        .execute(&f.pool)
        .await
        .unwrap();
    let result = journal::store_result(&f.pool, &sending, &Fixture::result(&snapshot))
        .await
        .unwrap();
    assert_eq!(result.phase(), GrantPhase::Result);
    assert!(publication::publish_result(&f.pool, &result)
        .await
        .unwrap()
        .is_none());
    sqlx::query("insert into gateway_provider_credentials(id,provider_account_id,label,payload_inline,source_kind,sync_mode,created_at) values($1,$2,'recreated',$3,'manual','manual',$4::text::timestamptz)")
        .bind(&f.credential).bind(&f.account).bind(sqlx::types::Json(snapshot.payload.clone()))
        .bind(original["created_at"].as_str().unwrap()).execute(&f.pool).await.unwrap();
    let recreated = f.snapshot().await;
    assert_ne!(recreated.request.revision, snapshot.request.revision);
    assert!(matches!(
        journal::reserve(&f.pool, &recreated).await.unwrap(),
        GrantReservation::Blocked
    ));
    assert!(publication::publish_result(&f.pool, &result)
        .await
        .unwrap()
        .is_none());
    assert_eq!(f.record(&sending).await.phase(), GrantPhase::Result);
    f.finish().await;
}

#[tokio::test]
#[ignore = "requires dedicated guarded PostgreSQL; serialize tests"]
async fn accio_db_refresh_health_does_not_revoke_but_disable_restore_and_payload_update_do() {
    let f = Fixture::new().await;
    let (snapshot, sending) = f.sending().await;
    sqlx::query("update gateway_provider_credentials set failure_count=9,last_error='synthetic-health',updated_at=now() where id=$1")
        .bind(&f.credential).execute(&f.pool).await.unwrap();
    assert_eq!(snapshot.request, f.snapshot().await.request);
    let result = journal::store_result(&f.pool, &sending, &Fixture::result(&snapshot))
        .await
        .unwrap();
    crate::db::note_provider_credential_runtime_failure(
        &f.pool,
        &f.credential,
        9,
        true,
        0,
        "synthetic-breaker",
    )
    .await
    .unwrap();
    assert!(publication::publish_result(&f.pool, &result)
        .await
        .unwrap()
        .is_none());
    let active = crate::db::list_active_provider_credential_refs_for_accounts(
        &f.pool,
        std::slice::from_ref(&f.account),
    )
    .await
    .unwrap();
    assert_eq!(active.len(), 1);
    assert_eq!(snapshot.request, f.snapshot().await.request);
    sqlx::query("update gateway_provider_credentials set status='disabled' where id=$1")
        .bind(&f.credential)
        .execute(&f.pool)
        .await
        .unwrap();
    assert!(publication::publish_result(&f.pool, &result)
        .await
        .unwrap()
        .is_none());
    sqlx::query("update gateway_provider_credentials set status='active' where id=$1")
        .bind(&f.credential)
        .execute(&f.pool)
        .await
        .unwrap();
    assert!(publication::publish_result(&f.pool, &result)
        .await
        .unwrap()
        .is_none());
    assert_eq!(f.record(&sending).await.phase(), GrantPhase::Result);
    let fresh = f.snapshot().await;
    sqlx::query("update gateway_provider_accounts set payload_inline=payload_inline where id=$1")
        .bind(&f.account)
        .execute(&f.pool)
        .await
        .unwrap();
    assert!(matches!(
        journal::reserve(&f.pool, &fresh).await.unwrap(),
        GrantReservation::Stale
    ));
    f.finish().await;
}

#[tokio::test]
#[ignore = "requires dedicated guarded PostgreSQL; serialize tests"]
async fn accio_db_refresh_cooling_recovery_can_publish_the_original_acknowledged_result() {
    let f = Fixture::new().await;
    let (snapshot, sending) = f.sending().await;
    let result = journal::store_result(&f.pool, &sending, &Fixture::result(&snapshot))
        .await
        .unwrap();
    crate::db::note_provider_credential_runtime_failure(
        &f.pool,
        &f.credential,
        9,
        true,
        0,
        "synthetic-breaker",
    )
    .await
    .unwrap();
    assert!(publication::publish_result(&f.pool, &result)
        .await
        .unwrap()
        .is_none());
    crate::db::list_active_provider_credential_refs_for_accounts(
        &f.pool,
        std::slice::from_ref(&f.account),
    )
    .await
    .unwrap();
    let next = publication::publish_result(&f.pool, &result)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(next.payload["apiKey"], "synthetic-successor");
    assert_eq!(f.record(&sending).await.phase(), GrantPhase::Published);
    f.finish().await;
}

#[tokio::test]
#[ignore = "requires dedicated guarded PostgreSQL; serialize tests"]
async fn accio_db_refresh_inline_publication_preserves_metadata_and_allows_only_exact_successor() {
    let f = Fixture::new().await;
    sqlx::query("update gateway_provider_credentials set payload_inline=payload_inline || '{\"token\":\"retained-non-auth-data\"}'::jsonb where id=$1")
        .bind(&f.credential).execute(&f.pool).await.unwrap();
    let (snapshot, sending) = f.sending().await;
    let (edit_snapshot, edit_view) =
        super::super::get_provider_credential_for_update(&f.pool, &f.credential)
            .await
            .unwrap()
            .unwrap();
    let before = f.row().await;
    let result = journal::store_result(&f.pool, &sending, &Fixture::result(&snapshot))
        .await
        .unwrap();
    let next = publication::publish_result(&f.pool, &result)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(next.payload["apiKey"], "synthetic-successor");
    assert_eq!(next.payload["token"], "retained-non-auth-data");
    assert_eq!(next.payload["refreshToken"], "synthetic-refresh");
    assert_eq!(next.payload["accioSgcookie"], "synthetic opaque sg");
    assert_eq!(next.payload["expiresAt"], "2027-01-01T00:00:00.123456789Z");
    assert_eq!(
        next.payload["extraBody"],
        json!({"empid":"retained-context"})
    );
    let after = f.row().await;
    for key in [
        "label",
        "status",
        "source_kind",
        "source_hash",
        "sync_mode",
        "sync_error",
        "last_error",
        "failure_count",
        "created_at",
    ] {
        assert_eq!(before[key], after[key], "metadata {key} must be preserved");
    }
    assert!(publication::publish_result(&f.pool, &result)
        .await
        .unwrap()
        .is_some());
    let rejected = super::super::update_provider_credential_from_snapshot(
        &f.pool,
        &f.credential,
        &edit_snapshot,
        crate::db::UpsertProviderCredentialInput {
            provider_account_id: f.account.clone(),
            label: "stale rename".into(),
            status: None,
            payload: edit_view.payload,
            source_kind: None,
            source_path: None,
            source_hash: None,
            sync_mode: None,
            sync_state: None,
            sync_error: None,
        },
    )
    .await
    .unwrap_err();
    assert_eq!(
        rejected.code.as_deref(),
        Some("provider_credential_snapshot_conflict")
    );
    assert_eq!(f.snapshot().await.payload["apiKey"], "synthetic-successor");
    let ack = journal::store_result(&f.pool, &sending, &Fixture::result(&snapshot))
        .await
        .unwrap();
    assert_eq!(ack.phase(), GrantPhase::Published);
    assert!(matches!(
        journal::reserve(&f.pool, &snapshot).await.unwrap(),
        GrantReservation::Stale
    ));
    assert!(matches!(
        journal::reserve(&f.pool, &next).await.unwrap(),
        GrantReservation::Reserved(_)
    ));
    f.finish().await;
}
