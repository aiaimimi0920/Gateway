use super::*;
use notify::EventKind;

fn build_test_credential(
    id: &str,
    sync_mode: &str,
    source_path: Option<&str>,
    status: &str,
    archived_at: Option<OffsetDateTime>,
) -> db::ProviderCredentialImportMetadata {
    db::ProviderCredentialImportMetadata {
        id: id.to_string(),
        label: format!("Credential {id}"),
        status: status.to_string(),
        source_kind: "folder_sync_import".to_string(),
        source_path: source_path.map(str::to_string),
        source_hash: None,
        sync_mode: sync_mode.to_string(),
        sync_state: "idle".to_string(),
        archived_at,
    }
}

#[test]
fn deleted_relative_paths_extract_removed_json_file() {
    let root = std::env::temp_dir().join("neuro-folder-sync-test-root");
    let deleted_path = root
        .join("codex")
        .join("codex-30c5b2fd-mo5e917c@sall.cc-team.json");
    let event =
        Event::new(EventKind::Remove(notify::event::RemoveKind::File)).add_path(deleted_path);

    assert_eq!(
        deleted_relative_paths_from_event(root.as_path(), &event),
        HashSet::from(["codex/codex-30c5b2fd-mo5e917c@sall.cc-team.json".to_string()])
    );
}

#[test]
fn missing_folder_file_deletes_only_active_folder_synced_credentials() {
    let observed_paths = HashSet::from(["codex/keep.json".to_string()]);
    let explicit_deleted_paths = HashSet::new();
    let synced_missing = build_test_credential(
        "cred-delete",
        "folder_sync",
        Some("codex/remove.json"),
        "active",
        None,
    );
    let synced_present = build_test_credential(
        "cred-keep",
        "folder_sync",
        Some("codex/keep.json"),
        "active",
        None,
    );
    let manual_missing = build_test_credential(
        "cred-manual",
        "manual",
        Some("codex/remove.json"),
        "active",
        None,
    );
    let archived_missing = build_test_credential(
        "cred-archived",
        "folder_sync",
        Some("codex/remove.json"),
        "archived",
        None,
    );
    let mut pending_export_missing = build_test_credential(
        "cred-pending-export",
        "folder_sync",
        Some("codex/pending.json"),
        "active",
        None,
    );
    pending_export_missing.source_kind = "manual".to_string();
    pending_export_missing.sync_state = "idle".to_string();

    assert!(should_delete_missing_folder_credential(
        &synced_missing,
        &observed_paths,
        &explicit_deleted_paths
    ));
    assert!(!should_delete_missing_folder_credential(
        &synced_present,
        &observed_paths,
        &explicit_deleted_paths
    ));
    assert!(!should_delete_missing_folder_credential(
        &manual_missing,
        &observed_paths,
        &explicit_deleted_paths
    ));
    assert!(!should_delete_missing_folder_credential(
        &archived_missing,
        &observed_paths,
        &explicit_deleted_paths
    ));
    assert!(!should_delete_missing_folder_credential(
        &pending_export_missing,
        &observed_paths,
        &explicit_deleted_paths
    ));
}

#[test]
fn explicit_delete_event_removes_only_materialized_folder_credentials() {
    let observed_paths = HashSet::new();
    let explicit_deleted_paths = HashSet::from(["codex/remove.json".to_string()]);
    let mut imported_missing = build_test_credential(
        "cred-imported-delete",
        "folder_sync",
        Some("codex/remove.json"),
        "active",
        None,
    );
    imported_missing.source_kind = "folder_sync_import".to_string();
    imported_missing.sync_state = "imported".to_string();

    let mut pending_export_missing = build_test_credential(
        "cred-pending-export",
        "folder_sync",
        Some("codex/remove.json"),
        "active",
        None,
    );
    pending_export_missing.source_kind = "manual".to_string();
    pending_export_missing.sync_state = "idle".to_string();

    let recreated_path = build_test_credential(
        "cred-recreated",
        "folder_sync",
        Some("codex/remove.json"),
        "active",
        None,
    );

    let different_path = build_test_credential(
        "cred-keep",
        "folder_sync",
        Some("codex/keep.json"),
        "active",
        None,
    );

    assert!(should_delete_explicitly_removed_folder_credential(
        &imported_missing,
        &observed_paths,
        &explicit_deleted_paths
    ));
    assert!(!should_delete_explicitly_removed_folder_credential(
        &pending_export_missing,
        &observed_paths,
        &explicit_deleted_paths
    ));
    assert!(!should_delete_explicitly_removed_folder_credential(
        &recreated_path,
        &HashSet::from(["codex/remove.json".to_string()]),
        &explicit_deleted_paths
    ));
    assert!(!should_delete_explicitly_removed_folder_credential(
        &different_path,
        &observed_paths,
        &explicit_deleted_paths
    ));
}

#[test]
fn explicit_delete_summary_updates_only_when_new_hits_exist() {
    let mut status = ProviderCredentialFolderSyncStatusView {
        last_explicit_delete_at: Some("2026-04-18T00:00:00Z".to_string()),
        last_explicit_delete_count: 1,
        last_explicit_delete_paths: vec!["codex/older.json".to_string()],
        recent_explicit_delete_events: vec![FolderSyncExplicitDeleteEventView {
            event_id: "event-old".to_string(),
            occurred_at: "2026-04-18T00:00:00Z".to_string(),
            deleted_count: 1,
            deleted_paths: vec!["codex/older.json".to_string()],
            provider_credential_ids: vec!["cred-old".to_string()],
        }],
        ..ProviderCredentialFolderSyncStatusView::default()
    };
    let new_events = vec![
        FolderSyncExplicitDeleteEventView {
            event_id: "event-new-1".to_string(),
            occurred_at: "2026-04-19T00:00:00Z".to_string(),
            deleted_count: 1,
            deleted_paths: vec!["codex/remove.json".to_string()],
            provider_credential_ids: vec!["cred-remove".to_string()],
        },
        FolderSyncExplicitDeleteEventView {
            event_id: "event-new-2".to_string(),
            occurred_at: "2026-04-19T00:02:00Z".to_string(),
            deleted_count: 2,
            deleted_paths: vec![
                "codex/remove.json".to_string(),
                "codex/second.json".to_string(),
            ],
            provider_credential_ids: vec!["cred-remove".to_string(), "cred-second".to_string()],
        },
    ];

    append_explicit_delete_events(&mut status, &new_events);

    assert_eq!(
        status.last_explicit_delete_at.as_deref(),
        Some("2026-04-19T00:02:00Z")
    );
    assert_eq!(status.last_explicit_delete_count, 2);
    assert_eq!(
        status.last_explicit_delete_paths,
        vec![
            "codex/remove.json".to_string(),
            "codex/second.json".to_string()
        ]
    );
    assert_eq!(
        status
            .recent_explicit_delete_events
            .iter()
            .map(|entry| entry.event_id.as_str())
            .collect::<Vec<_>>(),
        vec!["event-new-2", "event-new-1", "event-old"]
    );

    let preserved_status = status.clone();
    append_explicit_delete_events(&mut status, &[]);

    assert_eq!(
        status.last_explicit_delete_at,
        preserved_status.last_explicit_delete_at
    );
    assert_eq!(
        status.last_explicit_delete_count,
        preserved_status.last_explicit_delete_count
    );
    assert_eq!(
        status.last_explicit_delete_paths,
        preserved_status.last_explicit_delete_paths
    );
    assert_eq!(
        status.recent_explicit_delete_events,
        preserved_status.recent_explicit_delete_events
    );
}

#[test]
fn build_explicit_delete_event_deduplicates_paths_and_ids() {
    let event = build_explicit_delete_event(&[
        FolderSyncExplicitDeleteHit {
            provider_credential_id: "cred-1".to_string(),
            source_path: "codex/remove.json".to_string(),
        },
        FolderSyncExplicitDeleteHit {
            provider_credential_id: "cred-1".to_string(),
            source_path: "codex/remove.json".to_string(),
        },
        FolderSyncExplicitDeleteHit {
            provider_credential_id: "cred-2".to_string(),
            source_path: "codex/second.json".to_string(),
        },
    ]);

    assert!(!event.event_id.is_empty());
    assert_eq!(event.deleted_count, 2);
    assert_eq!(
        event.deleted_paths,
        vec![
            "codex/remove.json".to_string(),
            "codex/second.json".to_string()
        ]
    );
    assert_eq!(
        event.provider_credential_ids,
        vec!["cred-1".to_string(), "cred-2".to_string()]
    );
}

#[test]
fn materialized_folder_copy_requires_import_or_export_state() {
    let mut imported = build_test_credential(
        "cred-imported",
        "folder_sync",
        Some("codex/imported.json"),
        "active",
        None,
    );
    imported.source_kind = "folder_sync_import".to_string();
    imported.sync_state = "imported".to_string();

    let mut exported = build_test_credential(
        "cred-exported",
        "folder_sync",
        Some("codex/exported.json"),
        "active",
        None,
    );
    exported.source_kind = "manual".to_string();
    exported.sync_state = "exported".to_string();

    let mut pending = build_test_credential(
        "cred-pending",
        "folder_sync",
        Some("codex/pending.json"),
        "active",
        None,
    );
    pending.source_kind = "manual".to_string();
    pending.sync_state = "idle".to_string();

    assert!(has_materialized_folder_copy(&imported));
    assert!(has_materialized_folder_copy(&exported));
    assert!(!has_materialized_folder_copy(&pending));
}
