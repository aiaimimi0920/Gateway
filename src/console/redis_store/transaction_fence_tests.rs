//! Record identity is checked before trusting a remote gate, including legacy JSON ordering.
use super::*;
use time::OffsetDateTime;

fn record() -> TransactionRecord {
    TransactionRecord::new_prepared_with_backup_leaf(
        "tx-fence",
        Some("r1-aaaaaaaaaaaa"),
        "r2-bbbbbbbbbbbb",
        "revisions/r2-bbbbbbbbbbbb",
        true,
        Some(&"a".repeat(64)),
        "b".repeat(64),
        Some(".reserved-bak"),
        OffsetDateTime::from_unix_timestamp(1_700_000_000).unwrap(),
    )
    .unwrap()
}

fn classify(
    record: &TransactionRecord,
    prepared: &TransactionRecord,
) -> Result<&'static str, RouteConfigRedisStoreError> {
    classify_gate(
        Some(&serde_json::to_string(record).unwrap()),
        prepared,
        &prepared.revocation_tombstone().unwrap(),
    )
}

#[test]
fn prepared_identity_and_tombstone_are_stable_across_phases_and_retry_times() {
    let prepared = record();
    let tombstone = prepared.revocation_tombstone().unwrap();
    let mut progressed = prepared.clone();
    progressed
        .transition(
            TransactionPhase::YamlReplaced,
            prepared.created_at() + time::Duration::seconds(10),
            None,
        )
        .unwrap();
    assert_eq!(progressed.prepared_identity().unwrap(), prepared);
    assert_eq!(progressed.revocation_tombstone().unwrap(), tombstone);
    progressed
        .transition(
            TransactionPhase::RedisActivated,
            prepared.created_at() + time::Duration::seconds(20),
            None,
        )
        .unwrap();
    assert_eq!(progressed.revocation_tombstone().unwrap(), tombstone);
    assert_eq!(tombstone.updated_at(), prepared.created_at());
    assert_eq!(tombstone.prepared_identity().unwrap(), prepared);
}

#[test]
fn gate_accepts_only_exact_pending_activated_or_canonical_tombstone() {
    let prepared = record();
    assert_eq!(
        classify_gate(None, &prepared, &prepared.revocation_tombstone().unwrap()).unwrap(),
        "pending"
    );
    assert_eq!(classify(&prepared, &prepared).unwrap(), "pending");
    assert_eq!(
        classify(&prepared.revocation_tombstone().unwrap(), &prepared).unwrap(),
        "revoked"
    );
    let mut activated = prepared.clone();
    activated
        .transition(TransactionPhase::YamlReplaced, prepared.created_at(), None)
        .unwrap();
    assert!(classify(&activated, &prepared).is_err());
    activated
        .transition(
            TransactionPhase::RedisActivated,
            prepared.created_at(),
            None,
        )
        .unwrap();
    assert_eq!(classify(&activated, &prepared).unwrap(), "activated");
    activated
        .transition(TransactionPhase::Committed, prepared.created_at(), None)
        .unwrap();
    assert!(classify(&activated, &prepared).is_err());
}

#[test]
fn gate_rejects_changed_identity_unknown_fields_corruption_and_noncanonical_abort() {
    let prepared = record();
    let tombstone = prepared.revocation_tombstone().unwrap();
    let base = serde_json::to_value(&prepared).unwrap();
    for (field, value) in [
        ("txId", serde_json::json!("tx-other")),
        ("newRevision", serde_json::json!("r2-cccccccccccc")),
        ("oldRevision", serde_json::Value::Null),
        ("newYamlDigest", serde_json::json!("c".repeat(64))),
        ("previousYamlDigest", serde_json::json!("c".repeat(64))),
        (
            "archiveRelativePath",
            serde_json::json!("revisions/r2-cccccccccccc"),
        ),
        ("sameDirBackupLeaf", serde_json::json!(".other-bak")),
        ("createdAt", serde_json::json!("2020-01-01T00:00:00Z")),
        ("updatedAt", serde_json::json!("2030-01-01T00:00:00Z")),
        ("phase", serde_json::json!("futurePhase")),
        ("unknown", serde_json::json!(true)),
    ] {
        let mut changed = base.clone();
        changed[field] = value;
        assert!(
            classify_gate(Some(&changed.to_string()), &prepared, &tombstone).is_err(),
            "accepted changed {field}"
        );
    }
    assert!(classify_gate(Some("{broken"), &prepared, &tombstone).is_err());
    let mut abort = prepared.clone();
    abort
        .transition(
            TransactionPhase::Aborted,
            prepared.created_at(),
            Some("another_reason"),
        )
        .unwrap();
    assert!(classify(&abort, &prepared).is_err());
}
