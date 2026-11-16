//! An unusable access JWT must not discard an already received single-use refresh successor.
use super::*;
use neuro_gateway::protocol::nous_portal::{ReceivedTokens, TOKEN_ENDPOINT};

fn reserve(f: &Fixture) -> GrantRecord {
    let guard = f.persistence.try_writer_lock().unwrap();
    let GrantReservation::Reserved(record) = f
        .persistence
        .reserve_refresh_grant_locked(&guard, &f.revision, &f.request())
        .unwrap()
    else {
        panic!("not reserved")
    };
    record
}
fn receipt(access: &str, refresh: &str) -> GrantReceipt {
    let tokens = ReceivedTokens::from_json(
        serde_json::json!({"access_token":access,
        "refresh_token":refresh,"expires_in":"unusable"})
        .to_string()
        .as_bytes(),
    )
    .unwrap();
    GrantReceipt::NousPortal { tokens }
}
fn reopen(f: &Fixture) -> RouteConfigPersistence {
    RouteConfigPersistence::new(
        f.persistence.state_root().parent().unwrap(),
        f.persistence.routes_path(),
    )
    .unwrap()
}

#[test]
fn nous_receipt_is_recoverable_without_becoming_a_publishable_result() {
    let f = Fixture::new_for_endpoint("https://portal.nousresearch.com/api/oauth/token");
    let guard = f.persistence.try_writer_lock().unwrap();
    let GrantReservation::Reserved(record) = f
        .persistence
        .reserve_refresh_grant_locked(&guard, &f.revision, &f.request())
        .unwrap()
    else {
        panic!("not reserved")
    };
    let mut value = serde_json::to_value(&record).unwrap();
    value["receipt"] = serde_json::json!({"kind":"nousPortal", "tokens":{
        "response":{"access_token":"unusable-access", "refresh_token":"received-successor",
            "expires_in":"invalid"}, "received_at":1794150000
    }});
    let recovered = GrantRecord::from_json(&value.to_string())
        .expect("single-use successor needs a durable non-publishable receipt");
    assert_eq!(recovered.phase(), GrantPhase::Sending);
    assert!(recovered.result().is_none());
    assert!(!format!("{recovered:?}").contains("received-successor"));
}

#[test]
fn nous_receipt_is_immutable_durable_and_never_releases_the_consumed_old_grant() {
    let f = Fixture::new_for_endpoint(TOKEN_ENDPOINT);
    let first = reserve(&f);
    assert!(
        !first.to_json().unwrap().contains("receipt"),
        "old records retain their wire format"
    );
    let received = receipt("unusable-access", "received-successor");
    let saved = {
        let guard = f.persistence.try_writer_lock().unwrap();
        let saved = f
            .persistence
            .store_refresh_grant_receipt_locked(&guard, &first, &received)
            .unwrap();
        assert_eq!(
            f.persistence
                .store_refresh_grant_receipt_locked(&guard, &first, &received)
                .unwrap(),
            saved
        );
        assert!(f
            .persistence
            .store_refresh_grant_receipt_locked(
                &guard,
                &first,
                &receipt("other", "competing-successor")
            )
            .is_err());
        saved
    };
    assert_eq!(saved.phase(), GrantPhase::Sending);
    assert!(saved.result().is_none());
    assert_eq!(saved.receipt(), Some(&received));
    assert_eq!(saved.family_id(), first.family_id());
    assert_eq!(saved.material_id(), first.material_id());
    let reopened = reopen(&f);
    let guard = reopened.try_writer_lock().unwrap();
    let GrantReservation::Existing(restored) = reopened
        .reserve_refresh_grant_locked(&guard, &f.revision, &f.request())
        .unwrap()
    else {
        panic!("single-use grant became sendable again")
    };
    assert_eq!(restored, saved);
    let fabricated = f
        .request()
        .result(
            "fabricated-access",
            Some("received-successor"),
            3600,
            "2030-01-01T00:00:00Z",
            None,
        )
        .unwrap();
    assert!(reopened
        .store_refresh_grant_result_locked(&guard, &restored, &fabricated)
        .is_err());
    assert!(reopened
        .mark_refresh_grant_published_locked(&guard, &f.revision, &restored)
        .is_err());
}

#[test]
fn nous_received_result_requires_its_exact_admissible_successor() {
    let f = Fixture::new_for_endpoint(TOKEN_ENDPOINT);
    let first = reserve(&f);
    let guard = f.persistence.try_writer_lock().unwrap();
    let fabricated = f
        .request()
        .result(
            "fabricated-access",
            Some("fabricated-refresh"),
            60,
            "2030-01-01T00:00:00Z",
            None,
        )
        .unwrap();
    let received = receipt("invalid-jwt", "synthetic-refresh");
    let saved = f
        .persistence
        .store_refresh_grant_receipt_locked(&guard, &first, &received)
        .unwrap();
    assert!(f
        .persistence
        .store_refresh_grant_result_locked(&guard, &saved, &fabricated)
        .is_err());
    assert_eq!(saved.phase(), GrantPhase::Sending);
}

#[test]
fn nous_legacy_generic_result_without_receipt_remains_readable_and_unchanged() {
    let f = Fixture::new_for_endpoint(TOKEN_ENDPOINT);
    let first = reserve(&f);
    let result = f
        .request()
        .result(
            "legacy-access",
            Some("legacy-refresh"),
            60,
            "2030-01-01T00:00:00Z",
            None,
        )
        .unwrap();
    let guard = f.persistence.try_writer_lock().unwrap();
    let saved = f
        .persistence
        .store_refresh_grant_result_locked(&guard, &first, &result)
        .unwrap();
    assert_eq!(saved.phase(), GrantPhase::Result);
    assert!(saved.receipt().is_none());
    assert_eq!(
        GrantRecord::from_json(&saved.to_json().unwrap()).unwrap(),
        saved
    );
    assert!(!saved.to_json().unwrap().contains("receipt"));
}

#[tokio::test]
async fn nous_receipt_storage_obligation_survives_revoked_authority_without_publishing_secrets() {
    let f = Fixture::new_for_endpoint(TOKEN_ENDPOINT);
    let first = reserve(&f);
    let next = f
        .publish(
            "replacement-access",
            "replacement-refresh",
            "2030-01-01T00:00:00Z",
        )
        .await;
    let received = receipt("unusable-access", "delivered-after-revocation");
    let before_yaml = std::fs::read(f.persistence.routes_path()).unwrap();
    let guard = f.persistence.try_writer_lock().unwrap();
    let saved = f
        .persistence
        .store_refresh_grant_receipt_locked(&guard, &first, &received)
        .unwrap();
    assert_eq!(saved.receipt(), Some(&received));
    assert_eq!(
        std::fs::read(f.persistence.routes_path()).unwrap(),
        before_yaml
    );
    assert_eq!(
        f.runtime.route_config().snapshot().revision().id(),
        next.metadata().id()
    );
    assert!(f
        .persistence
        .mark_refresh_grant_published_locked(&guard, &next, &saved)
        .is_err());
    assert!(matches!(
        f.persistence
            .reserve_refresh_grant_locked(&guard, &f.revision, &f.request())
            .unwrap(),
        GrantReservation::Stale
    ));
}

#[test]
fn nous_receipts_cannot_be_attached_to_another_issuer_or_malformed_record() {
    let f = Fixture::new();
    let first = reserve(&f);
    let guard = f.persistence.try_writer_lock().unwrap();
    assert!(f
        .persistence
        .store_refresh_grant_receipt_locked(&guard, &first, &receipt("access", "new-refresh"))
        .is_err());
    let f = Fixture::new_for_endpoint(TOKEN_ENDPOINT);
    let first = reserve(&f);
    let mut value = serde_json::to_value(first).unwrap();
    value["receipt"] = serde_json::json!({"kind":"nousPortal","tokens":{
        "response":{"refresh_token":"private-successor"},"received_at":u64::MAX}});
    let error = GrantRecord::from_json(&value.to_string()).unwrap_err();
    assert!(!error.to_string().contains("private-successor"));
}

#[tokio::test]
async fn nous_exact_validated_receipt_can_advance_without_downgrading_on_late_acknowledgement() {
    use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
    let f = Fixture::new_for_endpoint(TOKEN_ENDPOINT);
    let first = reserve(&f);
    let access = format!(
        "{}.{}.{}",
        URL_SAFE_NO_PAD.encode(br#"{"alg":"RS256"}"#),
        URL_SAFE_NO_PAD.encode(
            serde_json::json!({"exp":time::OffsetDateTime::now_utc().unix_timestamp()+7200,
            "scope":"inference:invoke","sub":"subject-hint"})
            .to_string()
        ),
        URL_SAFE_NO_PAD.encode(b"synthetic-signature")
    );
    let received = ReceivedTokens::from_json(
        serde_json::json!({"access_token":access,"refresh_token":"valid-successor",
        "expires_in":3600,"inference_base_url":"https://inference-api.nousresearch.com/v1"})
        .to_string()
        .as_bytes(),
    )
    .unwrap();
    let admitted = received
        .admit(
            &neuro_gateway::protocol::nous_portal::OAuthClient::new("synthetic-client").unwrap(),
            None,
        )
        .unwrap();
    let expiry = time::OffsetDateTime::from_unix_timestamp(admitted.expires_at_unix as i64)
        .unwrap()
        .format(&time::format_description::well_known::Rfc3339)
        .unwrap();
    let received = GrantReceipt::NousPortal { tokens: received };
    let result = f
        .request()
        .result(&access, Some("valid-successor"), 3600, &expiry, None)
        .unwrap();
    let saved = {
        let guard = f.persistence.try_writer_lock().unwrap();
        let receipt_record = f
            .persistence
            .store_refresh_grant_receipt_locked(&guard, &first, &received)
            .unwrap();
        let mut invalid_expiry = serde_json::to_value(&result).unwrap();
        invalid_expiry["expiresAt"] = serde_json::json!("2035-01-01T00:00:00Z");
        invalid_expiry["material"]["expiresAt"] = invalid_expiry["expiresAt"].clone();
        let invalid_expiry: GrantResult = serde_json::from_value(invalid_expiry).unwrap();
        assert!(f
            .persistence
            .store_refresh_grant_result_locked(&guard, &receipt_record, &invalid_expiry)
            .is_err());
        let saved = f
            .persistence
            .store_refresh_grant_result_locked(&guard, &receipt_record, &result)
            .unwrap();
        assert_eq!(saved.phase(), GrantPhase::Result);
        assert_eq!(
            f.persistence
                .store_refresh_grant_receipt_locked(&guard, &first, &received)
                .unwrap(),
            saved
        );
        saved
    };
    let next = f.publish(&access, "valid-successor", &expiry).await;
    let guard = f.persistence.try_writer_lock().unwrap();
    let published = f
        .persistence
        .mark_refresh_grant_published_locked(&guard, &next, &saved)
        .unwrap();
    assert_eq!(published.phase(), GrantPhase::Published);
    assert_eq!(
        f.persistence
            .store_refresh_grant_receipt_locked(&guard, &first, &received)
            .unwrap(),
        published
    );
    assert_eq!(
        GrantRecord::from_json(&published.to_json().unwrap()).unwrap(),
        published
    );
}
