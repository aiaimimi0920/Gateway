use super::*;
use crate::provider_credential_folder_sync::FolderSyncExplicitDeleteEventView;

#[test]
fn folder_sync_run_result_preserves_watch_state_and_uncompleted_phase() {
    let mut status = ProviderCredentialFolderSyncStatusView {
        watch_running: true,
        last_watch_event_at: Some("watch event".into()),
        last_watch_error: Some("watch error".into()),
        last_export_at: Some("concurrent export".into()),
        ..Default::default()
    };
    let counters = FolderSyncCounters {
        imported_count: 3,
        ..Default::default()
    };
    apply_run_results(
        &mut status,
        OffsetDateTime::UNIX_EPOCH,
        &CompletedSyncPhases {
            import: true,
            export: false,
        },
        &counters,
        None,
    );
    assert!(status.watch_running);
    assert_eq!(status.last_watch_event_at.as_deref(), Some("watch event"));
    assert_eq!(status.last_watch_error.as_deref(), Some("watch error"));
    assert_eq!(status.last_export_at.as_deref(), Some("concurrent export"));
    assert_eq!(status.last_import_at, status.last_run_at);
    assert_eq!(status.imported_count, 3);
}

#[test]
fn folder_sync_run_result_merges_latest_delete_history_once() {
    let prior = FolderSyncExplicitDeleteEventView {
        event_id: "prior".into(),
        ..Default::default()
    };
    let event = FolderSyncExplicitDeleteEventView {
        event_id: "new".into(),
        occurred_at: "delete timestamp".into(),
        deleted_count: 1,
        deleted_paths: vec!["provider/deleted.json".into()],
        ..Default::default()
    };
    let mut status = ProviderCredentialFolderSyncStatusView {
        recent_explicit_delete_events: vec![prior.clone()],
        ..Default::default()
    };
    let counters = FolderSyncCounters {
        explicit_delete_events: vec![event.clone()],
        ..Default::default()
    };
    apply_run_results(
        &mut status,
        OffsetDateTime::UNIX_EPOCH,
        &CompletedSyncPhases::default(),
        &counters,
        None,
    );
    assert_eq!(
        status.recent_explicit_delete_events,
        vec![event.clone(), prior]
    );
    assert_eq!(status.last_explicit_delete_paths, event.deleted_paths);
    assert_eq!(status.last_explicit_delete_count, 1);
}

#[test]
fn folder_sync_failed_run_retains_successful_phases_and_delete_history() {
    let mut status = ProviderCredentialFolderSyncStatusView {
        last_import_at: Some("prior import".into()),
        last_export_at: Some("prior export".into()),
        recent_explicit_delete_events: vec![FolderSyncExplicitDeleteEventView {
            event_id: "prior deletion".into(),
            ..Default::default()
        }],
        ..Default::default()
    };
    apply_run_results(
        &mut status,
        OffsetDateTime::UNIX_EPOCH,
        &CompletedSyncPhases::default(),
        &FolderSyncCounters::default(),
        Some(&GatewayError::server_error("run failed")),
    );
    assert_eq!(status.last_import_at.as_deref(), Some("prior import"));
    assert_eq!(status.last_export_at.as_deref(), Some("prior export"));
    assert_eq!(status.recent_explicit_delete_events.len(), 1);
    assert_eq!(status.last_error.as_deref(), Some("run failed"));
}

#[test]
fn folder_sync_older_run_does_not_overwrite_newer_result() {
    let mut status = ProviderCredentialFolderSyncStatusView {
        last_run_at: Some("2026-09-18T00:02:00Z".into()),
        last_import_at: Some("2026-09-18T00:02:00Z".into()),
        imported_count: 9,
        last_error: Some("newer failure".into()),
        ..Default::default()
    };

    apply_run_results(
        &mut status,
        OffsetDateTime::parse(
            "2026-09-18T00:01:00Z",
            &time::format_description::well_known::Rfc3339,
        )
        .unwrap(),
        &CompletedSyncPhases {
            import: true,
            export: false,
        },
        &FolderSyncCounters {
            imported_count: 1,
            ..Default::default()
        },
        None,
    );

    assert_eq!(status.last_run_at.as_deref(), Some("2026-09-18T00:02:00Z"));
    assert_eq!(
        status.last_import_at.as_deref(),
        Some("2026-09-18T00:02:00Z")
    );
    assert_eq!(status.imported_count, 9);
    assert_eq!(status.last_error.as_deref(), Some("newer failure"));
}

#[test]
fn folder_sync_older_run_keeps_delete_audit_without_overwriting_newer_summary() {
    let newer_event = FolderSyncExplicitDeleteEventView {
        event_id: "newer-delete".into(),
        occurred_at: "2026-09-18T00:02:00Z".into(),
        deleted_count: 2,
        deleted_paths: vec!["newer.json".into()],
        ..Default::default()
    };
    let older_event = FolderSyncExplicitDeleteEventView {
        event_id: "older-delete".into(),
        occurred_at: "2026-09-18T00:01:00Z".into(),
        deleted_count: 1,
        deleted_paths: vec!["older.json".into()],
        ..Default::default()
    };
    let mut status = ProviderCredentialFolderSyncStatusView {
        last_run_at: Some("2026-09-18T00:02:00Z".into()),
        last_explicit_delete_at: Some(newer_event.occurred_at.clone()),
        last_explicit_delete_count: newer_event.deleted_count,
        last_explicit_delete_paths: newer_event.deleted_paths.clone(),
        recent_explicit_delete_events: vec![newer_event.clone()],
        imported_count: 9,
        ..Default::default()
    };

    apply_run_results(
        &mut status,
        OffsetDateTime::parse(
            "2026-09-18T00:01:00Z",
            &time::format_description::well_known::Rfc3339,
        )
        .unwrap(),
        &CompletedSyncPhases {
            import: true,
            export: false,
        },
        &FolderSyncCounters {
            imported_count: 1,
            explicit_delete_events: vec![older_event],
            ..Default::default()
        },
        None,
    );

    assert_eq!(status.imported_count, 9);
    assert_eq!(status.last_explicit_delete_count, 2);
    assert_eq!(status.last_explicit_delete_paths, vec!["newer.json"]);
    assert_eq!(
        status
            .recent_explicit_delete_events
            .iter()
            .map(|event| event.event_id.as_str())
            .collect::<Vec<_>>(),
        vec!["newer-delete", "older-delete"]
    );
}
