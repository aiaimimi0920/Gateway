//! Real import-phase regressions; no public hook or synthetic delete implementation.
use super::*;
mod fixture;
use fixture::Fixture;

#[tokio::test]
#[ignore = "requires dedicated guarded PostgreSQL and Redis; serialize tests"]
async fn explicit_then_missing_deletes_each_id_once_in_phase_order() {
    let mut fixture = Fixture::new().await;
    // Put missing first in the snapshot to distinguish phase order from row order.
    fixture.add("missing", "other.json").await;
    fixture.add("explicit", "removed.json").await;
    let outcome = fixture.run(true, &["removed.json"]).await;
    assert!(outcome.result.is_ok(), "{:?}", outcome.result);
    assert!(outcome.remaining.is_empty());
    assert_eq!(outcome.deleted, ["explicit", "missing"]);
    assert_eq!(outcome.counters.deleted_count, 2);
    assert!(outcome.key_counts.values().all(|count| *count == 0));
    assert_eq!(outcome.counters.explicit_delete_events.len(), 1);
    let event = &outcome.counters.explicit_delete_events[0];
    assert_eq!(event.deleted_count, 1);
    assert_eq!(event.deleted_paths, ["removed.json"]);
    assert_eq!(event.provider_credential_ids, ["explicit"]);
    assert!(!event.event_id.is_empty() && !event.occurred_at.is_empty());
}

#[tokio::test]
#[ignore = "requires dedicated guarded PostgreSQL and Redis; serialize tests"]
async fn directory_overlap_preserves_observed_ineligible_and_audit_semantics() {
    let mut fixture = Fixture::new().await;
    fixture.add("near", "group-other/c.json").await;
    fixture.add("first", "group/a.json").await;
    fixture.add("duplicate", "group/a.json").await;
    fixture.add("nested", "group/sub/b.json").await;
    fixture.add("recreated", "group/keep.json").await;
    fixture.add("archived", "group/archived.json").await;
    fixture.add("manual", "group/manual.json").await;
    fixture.add("pending", "group/pending.json").await;
    sqlx::raw_sql(
        "update gateway_provider_credentials set archived_at=now() where id='archived';
         update gateway_provider_credentials set sync_mode='manual' where id='manual';
         update gateway_provider_credentials set source_kind='manual',sync_state='idle' where id='pending';",
    ).execute(&fixture.pg).await.unwrap();
    std::fs::create_dir(fixture.root.join("group")).unwrap();
    std::fs::write(fixture.root.join("group/keep.json"), b"{}").unwrap();
    let outcome = fixture.run(true, &["group", "group/a.json"]).await;
    assert!(outcome.result.is_ok(), "{:?}", outcome.result);
    assert_eq!(outcome.deleted, ["first", "duplicate", "nested", "near"]);
    assert_eq!(
        outcome.remaining,
        ["archived", "manual", "pending", "recreated"]
    );
    assert_eq!(outcome.counters.deleted_count, 4);
    assert_eq!(outcome.counters.skipped_count, 1);
    for id in &outcome.deleted {
        assert_eq!(outcome.key_counts[id], 0);
    }
    for id in &outcome.remaining {
        assert_eq!(outcome.key_counts[id], 4);
    }
    assert_eq!(outcome.counters.explicit_delete_events.len(), 1);
    let event = &outcome.counters.explicit_delete_events[0];
    assert_eq!(
        event.deleted_count, 2,
        "audit counts distinct paths, not rows"
    );
    assert_eq!(event.deleted_paths, ["group/a.json", "group/sub/b.json"]);
    assert_eq!(
        event.provider_credential_ids,
        ["first", "duplicate", "nested"]
    );
}

#[tokio::test]
#[ignore = "requires dedicated guarded PostgreSQL and Redis; serialize tests"]
async fn explicit_only_leaves_unrelated_missing_row_and_runtime_keys() {
    let mut fixture = Fixture::new().await;
    fixture.add("missing", "other.json").await;
    fixture.add("explicit", "removed.json").await;
    let outcome = fixture.run(false, &["removed.json"]).await;
    assert!(outcome.result.is_ok());
    assert_eq!(outcome.deleted, ["explicit"]);
    assert_eq!(outcome.remaining, ["missing"]);
    assert_eq!(outcome.counters.deleted_count, 1);
    assert_eq!(outcome.key_counts["missing"], 4);
    assert_eq!(outcome.key_counts["explicit"], 0);
    assert_eq!(outcome.counters.explicit_delete_events.len(), 1);
}

#[tokio::test]
#[ignore = "requires dedicated guarded PostgreSQL and Redis; serialize tests"]
async fn empty_explicit_intent_retains_missing_deletion() {
    let mut fixture = Fixture::new().await;
    fixture.add("missing", "other.json").await;
    let outcome = fixture.run(true, &[]).await;
    assert!(outcome.result.is_ok());
    assert_eq!(outcome.deleted, ["missing"]);
    assert!(outcome.remaining.is_empty());
    assert_eq!(outcome.counters.deleted_count, 1);
    assert_eq!(outcome.key_counts["missing"], 0);
    assert!(outcome.counters.explicit_delete_events.is_empty());
}

#[tokio::test]
#[ignore = "requires dedicated guarded PostgreSQL and Redis; serialize tests"]
async fn explicit_failure_stops_before_missing_and_preserves_runtime_keys() {
    let mut fixture = Fixture::new().await;
    fixture.add("missing", "other.json").await;
    fixture.add("explicit", "removed.json").await;
    fixture.block("explicit").await;
    let outcome = fixture.run(true, &["removed.json"]).await;
    assert_eq!(outcome.result.unwrap_err().http_status, Some(404));
    assert!(outcome.deleted.is_empty());
    assert_eq!(outcome.remaining, ["explicit", "missing"]);
    assert_eq!(outcome.counters.deleted_count, 0);
    assert!(outcome.key_counts.values().all(|count| *count == 4));
    assert!(outcome.counters.explicit_delete_events.is_empty());
}

#[tokio::test]
#[ignore = "requires dedicated guarded PostgreSQL and Redis; serialize tests"]
async fn missing_failure_after_explicit_success_is_not_suppressed() {
    let mut fixture = Fixture::new().await;
    fixture.add("missing", "other.json").await;
    fixture.add("explicit", "removed.json").await;
    fixture.block("missing").await;
    let outcome = fixture.run(true, &["removed.json"]).await;
    assert_eq!(outcome.result.unwrap_err().http_status, Some(404));
    assert_eq!(outcome.deleted, ["explicit"]);
    assert_eq!(outcome.remaining, ["missing"]);
    assert_eq!(outcome.counters.deleted_count, 1);
    assert_eq!(outcome.key_counts["missing"], 4);
    assert_eq!(outcome.key_counts["explicit"], 0);
    assert_eq!(outcome.counters.explicit_delete_events.len(), 1);
    assert_eq!(
        outcome.counters.explicit_delete_events[0].provider_credential_ids,
        ["explicit"]
    );
}
