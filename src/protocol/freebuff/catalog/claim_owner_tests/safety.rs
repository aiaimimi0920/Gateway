//! Unknown/foreign/expired replies and corrupt local records cannot clear or rotate a purchase claim.
use super::*;
#[tokio::test]
async fn freebuff_claim_cancelled_body_and_none_remain_unknown_after_restart() {
    let (root, db) = setup().await;
    let peer = fixture::Peer::start(vec![]).await;
    let (session, device) = account();
    let requests = session.business_requests(&device, "open").unwrap();
    let prepared = pending(&db, &requests).await;
    let claim = prepared.claim().clone();
    let persisted = row(&db).await;
    let mut reply = fixture::Spec::json(200, active(&claim, "open"));
    reply.body_delay = Duration::from_millis(150);
    peer.push(reply);
    let started = Arc::new(tokio::sync::Notify::new());
    with_test_origin(peer.origin.clone(), TEST_BODY_STARTED.scope(started.clone(), async {
        let send = prepared.send(BUDGET);
        tokio::pin!(send);
        tokio::select! {
            _ = started.notified() => {},
            result = &mut send => panic!("must cancel while reading response: {}", result.is_ok()),
        }
    })).await;
    db.close().await;
    let db = LocalRuntime::open(&root).await.unwrap();
    assert_eq!(row(&db).await, persisted);
    let requests = session.recovery_requests(&device).unwrap();
    for (status, body) in [
        (200, json!({"status":"none"})),
        (
            200,
            json!({"status":"ended", "instanceId":claim.instance_id(), "freebucksRefund":0}),
        ),
        (
            200,
            active(
                &SessionClaim {
                    scope: claim.scope,
                    instance: "foreign-instance".into(),
                    attempt: claim.attempt_id().into(),
                },
                "open",
            ),
        ),
        (200, active(&claim, "other-model")),
        (503, active(&claim, "open")),
    ] {
        peer.push(fixture::Spec::json(status, body));
        let recovered = recovery(&db, &requests).await;
        assert!(
            with_test_origin(peer.origin.clone(), recovered.heartbeat(&requests, BUDGET))
                .await
                .is_err()
        );
        assert_eq!(row(&db).await, persisted);
    }
    assert_eq!(
        peer.hits
            .lock()
            .unwrap()
            .iter()
            .filter(|hit| hit.method == "POST")
            .count(),
        1
    );
    peer.close().await;
    cleanup(&root, db).await;
}

#[tokio::test]
async fn freebuff_claim_changed_authority_and_corrupt_receipts_are_not_absence() {
    let (root, db) = setup().await;
    let (session, device) = account();
    let requests = session.business_requests(&device, "open").unwrap();
    drop(pending(&db, &requests).await);
    let persisted = row(&db).await;
    for (token, client, origin) in [
        (
            "changed-token",
            ClientSurface::Cli,
            BusinessOrigin::Codebuff,
        ),
        (
            "synthetic-account",
            ClientSurface::Desktop,
            BusinessOrigin::Codebuff,
        ),
        (
            "synthetic-account",
            ClientSurface::Cli,
            BusinessOrigin::CodebuffWww,
        ),
    ] {
        let (session, device) = synthetic(token, client, origin);
        let changed = session.business_requests(&device, "open").unwrap();
        assert_eq!(
            db.prepare_freebuff_admission(
                "pool",
                "account",
                &changed,
                SessionChargeConsent::MaySpendFreebucks
            )
            .await
            .err()
            .unwrap()
            .code
            .as_deref(),
            Some("freebuff_session_claim_unavailable")
        );
        assert_eq!(row(&db).await, persisted);
    }
    // AAD binds provider+credential; copying ciphertext to a different stable slot fails closed.
    let AdmissionPreparation::Send(other) = db
        .prepare_freebuff_admission(
            "other",
            "account",
            &requests,
            SessionChargeConsent::MaySpendFreebucks,
        )
        .await
        .unwrap()
    else {
        panic!("new exact slot")
    };
    let other_id = other.claim().attempt_id().to_owned();
    drop(other);
    sqlx::query("UPDATE local_freebuff_session_claims SET sealed=? WHERE attempt_id=?")
        .bind(&persisted.2)
        .bind(&other_id)
        .execute(&db.pool)
        .await
        .unwrap();
    assert_eq!(
        db.prepare_freebuff_admission(
            "other",
            "account",
            &requests,
            SessionChargeConsent::MaySpendFreebucks
        )
        .await
        .err()
        .unwrap()
        .code
        .as_deref(),
        Some("freebuff_session_claim_unavailable")
    );
    sqlx::query("UPDATE local_freebuff_session_claims SET sealed='corrupt' WHERE binding=?")
        .bind(&persisted.0)
        .execute(&db.pool)
        .await
        .unwrap();
    assert_eq!(
        db.prepare_freebuff_admission(
            "pool",
            "account",
            &requests,
            SessionChargeConsent::MaySpendFreebucks
        )
        .await
        .err()
        .unwrap()
        .code
        .as_deref(),
        Some("freebuff_session_claim_unavailable")
    );
    cleanup(&root, db).await;
}

#[tokio::test]
async fn freebuff_claim_prepare_cancellation_on_both_sides_of_commit_never_exposes_send() {
    use crate::local_runtime::freebuff_sessions::TEST_PREPARE_CHECKPOINT;
    for after_commit in [false, true] {
        let (root, db) = setup().await;
        let (session, device) = account();
        let requests = session.business_requests(&device, "open").unwrap();
        let reached = Arc::new(tokio::sync::Notify::new());
        let resume = Arc::new(tokio::sync::Notify::new());
        {
            let prepare = TEST_PREPARE_CHECKPOINT.scope(
                (after_commit, reached.clone(), resume),
                db.prepare_freebuff_admission(
                    "pool",
                    "account",
                    &requests,
                    SessionChargeConsent::MaySpendFreebucks,
                ),
            );
            tokio::pin!(prepare);
            tokio::select! {
                _ = reached.notified() => {},
                result = &mut prepare => panic!("cancel before Send escapes: {}", result.is_ok()),
            }
        }
        db.close().await;
        let db = LocalRuntime::open(&root).await.unwrap();
        let recovered = db
            .recover_freebuff_admission(
                "pool",
                "account",
                &session.recovery_requests(&device).unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(recovered.is_some(), after_commit);
        drop(recovered);
        let preparation = db
            .prepare_freebuff_admission(
                "pool",
                "account",
                &requests,
                SessionChargeConsent::MaySpendFreebucks,
            )
            .await
            .unwrap();
        assert_eq!(
            matches!(preparation, AdmissionPreparation::Recover(_)),
            after_commit
        );
        drop(preparation);
        cleanup(&root, db).await;
    }
}

#[tokio::test]
async fn freebuff_claim_active_ttl_uses_both_limits_and_never_renews_a_record() {
    let (root, db) = setup().await;
    let (session, device) = account();
    let requests = session.business_requests(&device, "open").unwrap();
    let prepared = pending(&db, &requests).await;
    let claim = prepared.claim().clone();
    drop(prepared);
    let original = row(&db).await;
    for short_remaining in [true, false] {
        let now = OffsetDateTime::now_utc();
        let mut body = active(&claim, "open");
        if short_remaining {
            body["remainingMs"] = json!(1);
        } else {
            body["expiresAt"] = json!((now + time::Duration::milliseconds(1))
                .format(&time::format_description::well_known::Rfc3339)
                .unwrap());
        }
        let active =
            OwnedActiveSession::parse(BusinessReply { status: 200, body }, &claim, "open", now)
                .unwrap();
        tokio::time::sleep(Duration::from_millis(10)).await;
        assert!(!active.is_live());
        assert_eq!(row(&db).await, original);
        assert!(matches!(
            db.prepare_freebuff_admission(
                "pool",
                "account",
                &requests,
                SessionChargeConsent::MaySpendFreebucks
            )
            .await
            .unwrap(),
            AdmissionPreparation::Recover(_)
        ));
    }
    cleanup(&root, db).await;
}

#[tokio::test]
async fn freebuff_claim_only_database_does_not_replace_a_missing_seal() {
    let (root, db) = setup().await;
    let (session, device) = account();
    let requests = session.business_requests(&device, "open").unwrap();
    drop(pending(&db, &requests).await);
    let persisted = row(&db).await;
    let path = root.join("access-key-seal.json");
    let seal = std::fs::read(&path).unwrap();
    db.close().await;
    std::fs::remove_file(&path).unwrap();
    let db = LocalRuntime::open(&root).await.unwrap();
    assert!(!path.exists());
    assert_eq!(
        db.prepare_freebuff_admission(
            "pool",
            "account",
            &requests,
            SessionChargeConsent::MaySpendFreebucks
        )
        .await
        .err()
        .unwrap()
        .code
        .as_deref(),
        Some("freebuff_session_claim_unavailable")
    );
    assert_eq!(row(&db).await, persisted);
    db.close().await;
    std::fs::write(&path, seal).unwrap();
    let db = LocalRuntime::open(&root).await.unwrap();
    let recovered = recovery(&db, &session.recovery_requests(&device).unwrap()).await;
    assert_eq!(recovered.claim().attempt_id(), persisted.1);
    drop(recovered);
    cleanup(&root, db).await;
}

#[test]
fn freebuff_claim_active_validation_checks_time_identity_and_types() {
    let (session, device) = account();
    let requests = session.business_requests(&device, "open").unwrap();
    let admission = requests
        .prepare_cli_admission(SessionChargeConsent::MaySpendFreebucks)
        .unwrap();
    let claim = admission.claim();
    let original = active(claim, "open");
    for (field, value) in [
        ("status", json!("ended")),
        ("model", json!("fbm1.synthetic-open")),
        ("instanceId", json!("foreign")),
        ("remainingMs", json!(0)),
        ("remainingMs", json!(-1)),
        ("remainingMs", json!("3600000")),
        ("expiresAt", json!("2000-01-01T00:00:00Z")),
        ("admittedAt", original["expiresAt"].clone()),
        ("expiresAt", json!(null)),
        ("accessTier", json!(" full ")),
    ] {
        let mut body = original.clone();
        body[field] = value;
        assert!(
            OwnedActiveSession::parse(
                BusinessReply { status: 200, body },
                claim,
                "open",
                OffsetDateTime::now_utc()
            )
            .is_err(),
            "{field}"
        );
    }
    let live = OwnedActiveSession::parse(
        BusinessReply {
            status: 200,
            body: original,
        },
        claim,
        "open",
        OffsetDateTime::now_utc(),
    )
    .unwrap();
    assert!(live.is_live());
    assert!(!format!("{live:?}").contains(claim.instance_id()));
}

#[test]
fn freebuff_claim_receipt_is_strict_and_does_not_offer_a_public_claim_constructor() {
    let (session, device) = account();
    let requests = session.business_requests(&device, "open").unwrap();
    let admission = requests
        .prepare_cli_admission(SessionChargeConsent::MaySpendFreebucks)
        .unwrap();
    let bytes = requests.claim_receipt(admission.claim()).unwrap();
    let recover = session.recovery_requests(&device).unwrap();
    let (claim, key) = recover.restore_claim(&bytes).unwrap();
    assert_eq!(claim.attempt_id(), admission.claim().attempt_id());
    assert_eq!(key, "open");
    for (field, value) in [
        ("version", json!(2)),
        ("attempt", json!("arbitrary-caller-id")),
        ("attempt", json!(uuid::Uuid::nil().to_string())),
        ("model_key", json!("")),
        ("scope", json!(vec![0; 32])),
        ("extra", json!(true)),
    ] {
        let mut receipt: Value = serde_json::from_slice(&bytes).unwrap();
        receipt[field] = value;
        assert!(
            recover
                .restore_claim(&serde_json::to_vec(&receipt).unwrap())
                .is_err(),
            "{field}"
        );
    }
    assert!(recover.restore_claim(&vec![b' '; 4097]).is_err());
    assert!(!format!("{claim:?}").contains(claim.instance_id()));
}
