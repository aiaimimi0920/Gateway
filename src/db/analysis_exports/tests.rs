use super::{
    apply_text_mode, build_analysis_export_file_view, build_analysis_export_inventory_summary,
    build_manifest_artifacts, matches_persisted_analysis_export_view_filters,
    normalize_export_tags, redact_sensitive_text, GatewayAnalysisExportFilterView,
    GatewayAnalysisExportManifest, GatewayPersistedAnalysisExportFilters,
    GatewayPersistedAnalysisExportView,
};
use time::OffsetDateTime;

#[test]
fn redact_sensitive_text_scrubs_common_secrets() {
    let redacted = redact_sensitive_text(
        "contact me at demo@example.com with neuro_abc and nl_tk_bundle-1_key-1 and sk-secret-123456789012 plus Bearer tokenvalue123456",
    );
    assert!(redacted.contains("[REDACTED_EMAIL]"));
    assert!(redacted.contains("[REDACTED_API_KEY]"));
    assert!(redacted.contains("[REDACTED_SECRET]"));
    assert!(redacted.contains("Bearer [REDACTED_TOKEN]"));
}

#[test]
fn apply_text_mode_hides_text_for_none() {
    let applied = apply_text_mode("hello", "none", 10);
    assert!(applied.text.is_none());
    assert!(!applied.truncated);
}

#[test]
fn normalize_export_tags_deduplicates_and_lowercases() {
    let tags = normalize_export_tags(Some(&vec![
        "Pinned".to_string(),
        "pinned".to_string(),
        "Trace".to_string(),
    ]))
    .expect("normalize tags");
    assert_eq!(tags, vec!["pinned".to_string(), "trace".to_string()]);
}

#[test]
fn build_manifest_artifacts_emits_manifest_and_dataset_files() {
    let dataset = build_analysis_export_file_view(
        "dataset_jsonl",
        "ai-gateway/analysis-exports/export-1/dataset.jsonl".to_string(),
        "application/x-ndjson",
        b"{\"row\":1}\n",
        Some(1),
    );
    let manifest = build_manifest_artifacts(
        "export-1",
        Some("Export".to_string()),
        vec!["tag".to_string()],
        "2026-04-13T00:00:00Z".to_string(),
        None,
        GatewayAnalysisExportFilterView {
            project_id: Some("project-1".to_string()),
            route_policy_id: None,
            provider_account_id: None,
            session_id: None,
            api_key_id: None,
            response_id: None,
            protocol_family: None,
            status: None,
            endpoint_kind: None,
            stream: None,
            error_code: None,
            fallback_eligible: None,
            created_from: None,
            created_to: None,
            artifact_available: None,
            limit: 10,
            text_mode: "preview_redacted".to_string(),
            max_text_chars: 4000,
        },
        1,
        1,
        1,
        dataset,
        "ai-gateway/analysis-exports/export-1/manifest.json".to_string(),
    )
    .expect("build manifest");
    assert_eq!(manifest.manifest.files.len(), 2);
    assert_eq!(manifest.manifest.files[0].kind, "manifest");
    assert_eq!(manifest.manifest.files[1].kind, "dataset_jsonl");
    assert!(!manifest.manifest_body.is_empty());
}

#[test]
fn persisted_export_filters_match_label_tag_and_created_window() {
    let export = sample_persisted_export(
        "export-1",
        Some("Pinned Export"),
        vec!["Pinned".to_string(), "trace".to_string()],
        Some("project-1"),
        "preview_redacted",
        Some("2026-04-14T10:00:00Z"),
        "2026-04-14T09:00:00Z",
    );
    let filters = GatewayPersistedAnalysisExportFilters {
        export_id: None,
        label: Some("pinned".to_string()),
        tag: Some("trace".to_string()),
        project_id: Some("project-1".to_string()),
        status: Some("active".to_string()),
        text_mode: Some("preview_redacted".to_string()),
        created_from: None,
        created_to: None,
        limit: None,
    };
    let created_from = OffsetDateTime::parse(
        "2026-04-14T08:00:00Z",
        &time::format_description::well_known::Rfc3339,
    )
    .expect("parse created_from");
    let created_to = OffsetDateTime::parse(
        "2026-04-14T12:00:00Z",
        &time::format_description::well_known::Rfc3339,
    )
    .expect("parse created_to");
    assert!(matches_persisted_analysis_export_view_filters(
        &export,
        &filters,
        Some(created_from),
        Some(created_to),
    ));
}

#[test]
fn analysis_export_inventory_summary_tracks_pinned_and_expiring_counts() {
    let now = OffsetDateTime::parse(
        "2026-04-14T12:00:00Z",
        &time::format_description::well_known::Rfc3339,
    )
    .expect("parse now");
    let exports = vec![
        sample_persisted_export(
            "export-1",
            Some("Pinned Export"),
            vec!["pinned".to_string(), "trace".to_string()],
            Some("project-1"),
            "preview_redacted",
            Some("2026-04-14T18:00:00Z"),
            "2026-04-14T09:00:00Z",
        ),
        sample_persisted_export(
            "export-2",
            Some("Expired Export"),
            vec![],
            Some("project-1"),
            "full",
            Some("2026-04-14T11:00:00Z"),
            "2026-04-14T08:00:00Z",
        ),
    ];
    let summary = build_analysis_export_inventory_summary(&exports, now);
    assert_eq!(summary.total_exports, 2);
    assert_eq!(summary.active_exports, 2);
    assert_eq!(summary.pinned_exports, 1);
    assert_eq!(summary.expiring_within_24_hours, 1);
    assert_eq!(summary.expired_active_exports, 1);
    assert_eq!(summary.by_project[0].key, "project-1");
    assert_eq!(summary.by_text_mode.len(), 2);
}

fn sample_persisted_export(
    export_id: &str,
    label: Option<&str>,
    tags: Vec<String>,
    project_id: Option<&str>,
    text_mode: &str,
    retention_expires_at: Option<&str>,
    created_at: &str,
) -> GatewayPersistedAnalysisExportView {
    let filters = GatewayAnalysisExportFilterView {
        project_id: project_id.map(ToString::to_string),
        route_policy_id: None,
        provider_account_id: None,
        session_id: None,
        api_key_id: None,
        response_id: None,
        protocol_family: None,
        status: None,
        endpoint_kind: None,
        stream: None,
        error_code: None,
        fallback_eligible: None,
        created_from: None,
        created_to: None,
        artifact_available: None,
        limit: 10,
        text_mode: text_mode.to_string(),
        max_text_chars: 4000,
    };
    let manifest = GatewayAnalysisExportManifest {
        schema_version: 1,
        export_id: export_id.to_string(),
        label: label.map(ToString::to_string),
        tags: tags.clone(),
        created_at: created_at.to_string(),
        retention_expires_at: retention_expires_at.map(ToString::to_string),
        filters: filters.clone(),
        sample_count: 3,
        request_artifact_count: 2,
        response_artifact_count: 1,
        files: vec![build_analysis_export_file_view(
            "manifest",
            format!("ai-gateway/analysis-exports/{export_id}/manifest.json"),
            "application/json",
            b"{}",
            None,
        )],
    };
    GatewayPersistedAnalysisExportView {
        export_id: export_id.to_string(),
        label: label.map(ToString::to_string),
        tags,
        status: "active".to_string(),
        created_at: created_at.to_string(),
        updated_at: created_at.to_string(),
        object_prefix: format!("ai-gateway/analysis-exports/{export_id}"),
        filters,
        sample_count: 3,
        request_artifact_count: 2,
        response_artifact_count: 1,
        retention_expires_at: retention_expires_at.map(ToString::to_string),
        cleaned_up_at: None,
        last_cleanup_error: None,
        files: manifest.files.clone(),
        manifest,
    }
}
