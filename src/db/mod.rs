//! Database API aggregation and shared private SQL records.

pub mod access;
pub mod analysis_exports;
pub mod anomaly_incidents;
pub mod conversation_archives;
pub mod conversation_datasets;
pub mod operator;
pub mod provider_accounts;
pub mod provider_credential_model_states;
pub mod provider_credentials;
pub mod rate_limit_hotspots;
pub mod remediation;
pub mod request_audits;
pub mod routing;
pub mod sessions;
pub mod usage_aggregates;

mod account_models;
mod benefit_projects;
mod project_api_access;
mod project_details;
mod provider_payloads;
mod user_credentials;

pub use account_models::{
    BatchProviderCredentialOperation, BatchProviderCredentialOutcome,
    BatchProviderCredentialResult, BatchProviderCredentialSummary, GatewayApiKeyAuthRow,
    GatewayApiKeyView, GatewayBenefitProjectEnsureView, GatewayProjectApiAccessView,
    GatewayProjectView, GatewayProviderPayloadView, GatewayTenantView,
    GatewayUserCredentialCacheEntry, IssuedUserCredential, ProviderCredentialMutation,
    UserCredentialIssueInput, VerifiedUserCredential,
};
pub use benefit_projects::ensure_benefit_project;
pub use project_api_access::{
    find_gateway_api_key_auth, resolve_or_create_gateway_api_access, rotate_gateway_api_access,
};
use project_details::{
    gateway_project_view_from_row, gateway_tenant_view_from_row, get_active_project_detail,
    get_active_tenant_detail, get_project_detail,
};
pub use provider_payloads::{
    delete_provider_payload, get_provider_payload, list_active_provider_account_ids,
    save_provider_payload_inline,
};
pub use user_credentials::{
    find_user_credential, issue_user_credential, revoke_user_credential, verify_user_credential,
};

use std::time::Duration;

use serde::Serialize;
use serde_json::Value;
use sqlx::postgres::PgPoolOptions;
use sqlx::types::Json;
use sqlx::{FromRow, PgPool, Row};
use time::format_description::well_known::Rfc3339;
use time::OffsetDateTime;
use uuid::Uuid;

use crate::error::GatewayError;
use crate::object_storage::{
    build_gateway_provider_account_object_key, choose_provider_payload_storage_mode,
    gateway_object_storage,
};

pub use access::{
    adjust_access_key_balance, delete_access_bundle, delete_access_key,
    ensure_default_access_bundle_for_project, evaluate_access_key_balance,
    find_access_key_auth_by_external_key, find_access_key_auth_by_id, get_access_key_balance,
    inspect_access_sticky_affinity, list_access_catalog, list_models_for_access_key,
    pre_deduct_access_key_balance, preview_access_candidates, preview_route_decision,
    record_access_sticky_affinity, refund_access_key_balance, replace_access_bundle_items,
    replace_access_key_aggregate_memberships, reset_access_sticky_affinity,
    resolve_access_key_route_context, revoke_access_key, rotate_access_key, save_access_bundle,
    save_access_key, save_platform_access, save_provider_capability, scope_list_from_metadata,
    settle_access_key_balance, touch_access_key_last_used, validate_access_key_auth,
    AccessBalanceDecision, AccessKeyAuthRecord, AccessKeyBalanceAdjustInput,
    AggregateMembershipInput, DeleteAccessBundleResult, DeleteAccessKeyResult,
    GatewayAccessBundleItemView, GatewayAccessBundleView, GatewayAccessCandidatePreviewView,
    GatewayAccessCatalogView, GatewayAccessKeyAggregateMembershipView, GatewayAccessKeyBalanceView,
    GatewayAccessKeyView, GatewayAccessRouteDecisionPreviewView, GatewayAccessStickyAffinityView,
    GatewayPlatformAccessView, GatewayProviderCapabilityView, ProjectedPlatformAccessRow,
    ResolvedAccessKeyRouteContext, UpsertAccessBundleInput, UpsertAccessKeyInput,
    UpsertPlatformAccessInput, UpsertProviderCapabilityInput,
};

pub use crate::provider_quota::{GatewayProviderQuotaView, GatewayProviderQuotaWindowView};
pub use analysis_exports::{
    cleanup_expired_analysis_exports, export_analysis_rows, get_analysis_export_anomaly_report,
    get_analysis_export_baseline_report, get_analysis_export_timeline_report,
    get_analysis_export_trend_report, get_persisted_analysis_export,
    get_persisted_analysis_export_diff, list_persisted_analysis_exports, persist_analysis_export,
    summarize_persisted_analysis_exports, update_persisted_analysis_export_metadata,
    GatewayAnalysisExportAnomalyOverrides, GatewayAnalysisExportAnomalyReportView,
    GatewayAnalysisExportAnomalyThresholdConfig, GatewayAnalysisExportAnomalyView,
    GatewayAnalysisExportBaselineReportFilterView, GatewayAnalysisExportBaselineReportView,
    GatewayAnalysisExportBucketDeltaView, GatewayAnalysisExportCleanupEntryView,
    GatewayAnalysisExportCleanupInput, GatewayAnalysisExportCleanupResult,
    GatewayAnalysisExportDiffView, GatewayAnalysisExportFileView, GatewayAnalysisExportFilterView,
    GatewayAnalysisExportInventorySummaryView, GatewayAnalysisExportManifest,
    GatewayAnalysisExportMessageView, GatewayAnalysisExportMetadataUpdateInput,
    GatewayAnalysisExportMetricDeltaView, GatewayAnalysisExportRowView,
    GatewayAnalysisExportTimelinePairView, GatewayAnalysisExportTimelineReportView,
    GatewayAnalysisExportTrendMetricSummaryView, GatewayAnalysisExportTrendPointView,
    GatewayAnalysisExportTrendReportView, GatewayAnalysisExportTrendSummaryView,
    GatewayAnalysisExportView, GatewayPersistedAnalysisExportFilters,
    GatewayPersistedAnalysisExportView, PersistGatewayAnalysisExportInput,
};
pub use anomaly_incidents::{
    acknowledge_anomaly_incident, list_anomaly_incident_history, list_anomaly_incidents,
    record_anomaly_incident_alert_dispatch, resolve_anomaly_incident, summarize_anomaly_incidents,
    sync_analysis_export_anomaly_incidents, sync_provider_routing_anomaly_incidents,
    sync_rate_limit_hotspot_anomaly_incidents, update_anomaly_incident_follow_up,
    GatewayAnalysisAnomalyIncidentFilters, GatewayAnalysisAnomalyIncidentFollowUpInput,
    GatewayAnalysisAnomalyIncidentHistoryView, GatewayAnalysisAnomalyIncidentSummaryView,
    GatewayAnalysisAnomalyIncidentView, GatewayAnalysisExportAutoEscalationConfig,
    GatewaySyncAnalysisExportAnomalyIncidentsInput,
    GatewaySyncAnalysisExportAnomalyIncidentsResult,
    GatewaySyncProviderRoutingAnalysisAnomalyIncidentsResult,
    GatewaySyncRateLimitHotspotAnomalyIncidentsResult,
    RecordGatewayAnalysisAnomalyIncidentAlertDispatchInput,
};
pub use conversation_archives::{
    create_conversation_archive, export_conversation_archives, get_conversation_archive,
    list_conversation_archives, ConversationArchiveFilters, CreateConversationArchiveInput,
    GatewayConversationArchiveExportView, GatewayConversationArchiveView,
};
pub use conversation_datasets::{
    create_conversation_dataset_export, get_conversation_dataset_export,
    list_conversation_dataset_exports, publish_conversation_dataset_export,
    review_conversation_dataset_export, CreateConversationDatasetExportInput,
    GatewayConversationDatasetExportView, ReviewConversationDatasetExportInput,
};
pub use operator::{
    clear_provider_runtime_keys, get_cost_overview, get_model_association_matrix,
    get_provider_inventory, get_readiness_provider_stats, get_runtime_pressure,
    list_expired_cooling_provider_account_ids, mark_provider_cooling_retry_failure,
    mark_provider_probe_failure, mark_provider_probe_success, note_provider_runtime_failure,
    note_provider_runtime_success, GatewayCatalogMetadataView, GatewayCostModelBucketView,
    GatewayCostModelProviderRowView, GatewayCostOverviewSummaryView, GatewayCostOverviewView,
    GatewayCostProviderBucketView, GatewayCostProviderModelRowView,
    GatewayModelAssociationAliasRowView, GatewayModelAssociationMatrixSummaryView,
    GatewayModelAssociationMatrixView, GatewayModelAssociationProviderAliasLinkView,
    GatewayModelAssociationProviderLinkView, GatewayModelAssociationProviderRowView,
    GatewayPriceRateView, GatewayProjectPressureView, GatewayProviderCostHintsView,
    GatewayProviderHealthView, GatewayProviderInventoryEntryView,
    GatewayProviderInventorySummaryView, GatewayProviderInventoryView, GatewayProviderPressureView,
    GatewayProviderPricingEditorModelRowView, GatewayProviderPricingEditorView,
    GatewayReadinessProviderStatsView, GatewayRuntimePressureFilters, GatewayRuntimePressureView,
    GatewayRuntimeProviderIdentity, GatewaySourceProfileView, GatewaySummaryBucketView,
};
pub use provider_accounts::{
    create_provider_account, delete_provider_account, get_provider_account, list_provider_accounts,
    update_provider_account, GatewayProviderAccountView, UpsertProviderAccountInput,
};
pub(crate) use provider_accounts::{
    get_provider_account_for_folder_sync, list_provider_account_import_metadata,
    ProviderAccountFolderSyncMetadata,
};
pub use provider_credential_model_states::{
    apply_provider_credential_model_states_to_candidates, list_provider_credential_model_states,
    provider_credential_model_state_id, record_provider_credential_model_failure,
    record_provider_credential_model_success, CredentialModelStateFilters,
    GatewayProviderCredentialModelStateView, RecordCredentialModelFailureInput,
    RecordCredentialModelSuccessInput,
};
pub use provider_credentials::{
    create_provider_credential, delete_provider_credential, get_provider_credential,
    get_provider_credential_by_source_path, list_active_provider_credentials_for_accounts,
    list_active_provider_credentials_for_adapter, list_expired_cooling_provider_credential_ids,
    list_provider_credentials, mark_provider_credential_probe_failure,
    mark_provider_credential_probe_success, merge_provider_account_and_credential_payloads,
    note_provider_credential_runtime_failure, note_provider_credential_runtime_success,
    update_provider_credential, update_provider_credential_sync_metadata,
    update_provider_credential_sync_state, GatewayProviderCredentialView,
    UpsertProviderCredentialInput,
};
pub(crate) use provider_credentials::{
    list_provider_credential_import_metadata, ProviderCredentialImportMetadata,
};
pub use rate_limit_hotspots::{
    get_rate_limit_hotspot_anomaly_report, get_rate_limit_hotspot_anomaly_snapshot,
    get_rate_limit_hotspot_snapshot, get_rate_limit_hotspot_snapshot_trend_report,
    get_rate_limit_hotspot_trend_report, list_rate_limit_hotspot_anomaly_snapshots,
    list_rate_limit_hotspot_snapshots, persist_rate_limit_hotspot_anomaly_snapshot,
    persist_rate_limit_hotspot_snapshot, summarize_rate_limit_hotspot_snapshot_inventory,
    summarize_rate_limit_hotspots, GatewayRateLimitHotspotAnomalyOverrides,
    GatewayRateLimitHotspotAnomalyReportView, GatewayRateLimitHotspotAnomalySnapshotFilterView,
    GatewayRateLimitHotspotAnomalySnapshotFilters, GatewayRateLimitHotspotAnomalySnapshotView,
    GatewayRateLimitHotspotSnapshotFilterView, GatewayRateLimitHotspotSnapshotFilters,
    GatewayRateLimitHotspotSnapshotInventorySummaryView,
    GatewayRateLimitHotspotSnapshotReportFilterView, GatewayRateLimitHotspotSnapshotTrendPointView,
    GatewayRateLimitHotspotSnapshotTrendReportView,
    GatewayRateLimitHotspotSnapshotTrendSummaryView, GatewayRateLimitHotspotSnapshotView,
    GatewayRateLimitHotspotSummaryView, GatewayRateLimitHotspotTrendReportView,
};
pub use remediation::{
    capture_anomaly_incident_remediation_run_impact, execute_anomaly_incident_remediation,
    get_anomaly_incident_remediation_plan, get_anomaly_incident_remediation_run_impact,
    get_anomaly_remediation_effectiveness, get_anomaly_remediation_effectiveness_anomaly_snapshot,
    get_anomaly_remediation_effectiveness_snapshot,
    get_anomaly_remediation_effectiveness_snapshot_anomaly_report,
    get_anomaly_remediation_effectiveness_trend_report, list_anomaly_incident_alert_queue,
    list_anomaly_incident_remediation_runs, list_anomaly_policies,
    list_anomaly_remediation_effectiveness_anomaly_snapshots,
    list_anomaly_remediation_effectiveness_snapshots, list_anomaly_remediation_queue,
    persist_anomaly_remediation_effectiveness_anomaly_snapshot,
    persist_anomaly_remediation_effectiveness_snapshot, save_anomaly_policy,
    summarize_anomaly_incident_remediation_runs, summarize_anomaly_policies,
    summarize_anomaly_remediation_effectiveness_snapshots, sweep_anomaly_policies,
    sweep_anomaly_remediations, sync_anomaly_incidents, sync_anomaly_policy,
    ExecuteGatewayAnalysisAnomalyIncidentRemediationInput,
    GatewayAnalysisAnomalyIncidentAlertQueueItemView, GatewayAnalysisAnomalyIncidentAlertQueueView,
    GatewayAnalysisAnomalyIncidentRemediationActionView,
    GatewayAnalysisAnomalyIncidentRemediationPlanView,
    GatewayAnalysisAnomalyIncidentRemediationQueueItemView,
    GatewayAnalysisAnomalyIncidentRemediationQueueView,
    GatewayAnalysisAnomalyIncidentRemediationRoutePolicyPatchInput,
    GatewayAnalysisAnomalyIncidentRemediationRunView, GatewayAnalysisAnomalyIncidentSyncInput,
    GatewayAnalysisAnomalyIncidentSyncView, GatewayAnalysisAnomalyPolicyFilters,
    GatewayAnalysisAnomalyPolicySummaryView, GatewayAnalysisAnomalyPolicySweepItemView,
    GatewayAnalysisAnomalyPolicySweepView, GatewayAnalysisAnomalyPolicySyncView,
    GatewayAnalysisAnomalyPolicyView, GatewayAnalysisAnomalyRemediationActionEffectivenessView,
    GatewayAnalysisAnomalyRemediationEffectivenessAnomalyOverrides,
    GatewayAnalysisAnomalyRemediationEffectivenessAnomalyReportView,
    GatewayAnalysisAnomalyRemediationEffectivenessAnomalySnapshotFilterView,
    GatewayAnalysisAnomalyRemediationEffectivenessAnomalySnapshotFilters,
    GatewayAnalysisAnomalyRemediationEffectivenessAnomalySnapshotView,
    GatewayAnalysisAnomalyRemediationEffectivenessAnomalyThresholdConfig,
    GatewayAnalysisAnomalyRemediationEffectivenessAnomalyView,
    GatewayAnalysisAnomalyRemediationEffectivenessMetricView,
    GatewayAnalysisAnomalyRemediationEffectivenessSnapshotFilterView,
    GatewayAnalysisAnomalyRemediationEffectivenessSnapshotFilters,
    GatewayAnalysisAnomalyRemediationEffectivenessSnapshotInventorySummaryView,
    GatewayAnalysisAnomalyRemediationEffectivenessSnapshotReportFilterView,
    GatewayAnalysisAnomalyRemediationEffectivenessSnapshotView,
    GatewayAnalysisAnomalyRemediationEffectivenessSummaryView,
    GatewayAnalysisAnomalyRemediationEffectivenessTrendMetricPointView,
    GatewayAnalysisAnomalyRemediationEffectivenessTrendMetricSummaryView,
    GatewayAnalysisAnomalyRemediationEffectivenessTrendPointView,
    GatewayAnalysisAnomalyRemediationEffectivenessTrendReportView,
    GatewayAnalysisAnomalyRemediationEffectivenessTrendSummaryView,
    GatewayAnalysisAnomalyRemediationImpactCaptureView,
    GatewayAnalysisAnomalyRemediationImpactMetricView,
    GatewayAnalysisAnomalyRemediationImpactWindowView,
    GatewayAnalysisAnomalyRemediationQueueFilters, GatewayAnalysisAnomalyRemediationRunFilters,
    GatewayAnalysisAnomalyRemediationRunImpactMetricsView,
    GatewayAnalysisAnomalyRemediationRunImpactView,
    GatewayAnalysisAnomalyRemediationRunSummaryView,
    GatewayAnalysisAnomalyRemediationSweepItemView, GatewayAnalysisAnomalyRemediationSweepView,
    UpsertGatewayAnalysisAnomalyPolicyInput,
};
pub use request_audits::{
    create_request_audit, finalize_request_audit, get_prompt_cache_metrics,
    get_prompt_cache_trend_report, get_provider_routing_anomaly_report, get_request_audit,
    list_analysis_samples, list_request_audits, summarize_analysis, summarize_prompt_cache,
    summarize_provider_routing_analysis, summarize_request_audits,
    update_request_audit_artifact_keys, CreateRequestAuditInput, FinalizeRequestAuditInput,
    GatewayAnalysisSampleView, GatewayAnalysisSummaryView, GatewayPromptCacheMetricsView,
    GatewayPromptCacheSummaryView, GatewayPromptCacheTrendPointView,
    GatewayPromptCacheTrendReportView, GatewayProviderRoutingAnalysisAnomalyOverrides,
    GatewayProviderRoutingAnalysisAnomalyReportView, GatewayProviderRoutingAnalysisSummaryView,
    GatewayRequestAuditSummaryView, GatewayRequestAuditView, RequestAuditFilters,
};
pub use routing::{
    delete_model_alias, find_active_route_policy_for_rate_limits, list_model_aliases,
    list_models_for_project, list_route_policies, normalize_route_policy_config,
    resolve_route_candidates, resolve_route_candidates_allowing_empty, save_model_alias,
    save_route_policy, GatewayModelAliasView, GatewayRateLimitDefinition, GatewayRoutePolicyConfig,
    GatewayRoutePolicyConfigInput, GatewayRoutePolicyView, ResolvedProjectRouteContext,
    SaveModelAliasInput, SaveRoutePolicyInput,
};
pub use sessions::{
    resolve_gateway_session, upsert_gateway_session, GatewaySessionView, UpsertGatewaySessionInput,
};
pub use usage_aggregates::{
    list_usage_aggregate_buckets, summarize_usage_aggregates, upsert_usage_aggregate_buckets,
    GatewayUsageAggregateAlertView, GatewayUsageAggregateBucketView,
    GatewayUsageAggregateSummaryView, UsageAggregateFilters,
};

#[derive(Debug, Clone, FromRow)]
pub(crate) struct GatewayTenantRow {
    id: String,
    status: String,
}

#[derive(Debug, Clone, FromRow)]
struct GatewayTenantDetailRow {
    id: String,
    slug: String,
    display_name: String,
    status: String,
    owner_user_id: Option<String>,
    source_kind: String,
    source_key: String,
    created_at: OffsetDateTime,
    updated_at: OffsetDateTime,
}

#[derive(Debug, Clone, FromRow)]
pub(crate) struct GatewayProjectRow {
    id: String,
    tenant_id: String,
    status: String,
    default_route_policy_id: Option<String>,
}

#[derive(Debug, Clone, FromRow)]
struct GatewayProjectDetailRow {
    id: String,
    tenant_id: String,
    slug: String,
    display_name: String,
    status: String,
    source_kind: String,
    source_key: String,
    default_route_policy_id: Option<String>,
    created_at: OffsetDateTime,
    updated_at: OffsetDateTime,
}

#[derive(Debug, Clone, FromRow)]
struct GatewayApiKeyDetailRow {
    id: String,
    project_id: String,
    name: String,
    status: String,
    rotated_from_api_key_id: Option<String>,
    revoked_at: Option<OffsetDateTime>,
    created_at: OffsetDateTime,
}

#[derive(Debug, Clone, FromRow)]
struct GatewayUserCredentialJoinRow {
    id: String,
    user_id: String,
    project_id: String,
    status: String,
    expires_at: OffsetDateTime,
    scope: Json<Vec<String>>,
    tenant_id: String,
    project_status: String,
    tenant_status: String,
}

#[derive(Debug, Clone, FromRow)]
struct GatewayProviderPayloadRow {
    id: String,
    payload_inline: Option<Json<Value>>,
    payload_object_key: Option<String>,
    storage_mode: String,
    status: String,
    updated_at: OffsetDateTime,
}

pub async fn create_pg_pool(database_url: &str) -> Result<PgPool, GatewayError> {
    PgPoolOptions::new()
        .max_connections(20)
        .min_connections(2)
        .acquire_timeout(Duration::from_secs(5))
        .idle_timeout(Duration::from_secs(600))
        .max_lifetime(Duration::from_secs(1800))
        .test_before_acquire(true)
        .connect(database_url)
        .await
        .map_err(|error| GatewayError::service_unavailable(format!("connect postgres: {error}")))
}

fn get_active_project<'a>(
    pool: &'a PgPool,
    project_id: &'a str,
) -> impl std::future::Future<Output = Result<GatewayProjectRow, GatewayError>> + 'a {
    async move {
        let row = sqlx::query_as::<_, GatewayProjectRow>(
            r#"
            select id, tenant_id, status, default_route_policy_id
            from gateway_projects
            where id = $1
            limit 1
            "#,
        )
        .bind(project_id)
        .fetch_optional(pool)
        .await
        .map_err(map_db_error)?;

        let Some(row) = row else {
            return Err(GatewayError::not_found("AI gateway project 不存在"));
        };
        if row.status != "active" {
            return Err(GatewayError::conflict("AI gateway project 未激活"));
        }
        Ok(row)
    }
}

fn get_active_tenant<'a>(
    pool: &'a PgPool,
    tenant_id: &'a str,
) -> impl std::future::Future<Output = Result<GatewayTenantRow, GatewayError>> + 'a {
    async move {
        let row = sqlx::query_as::<_, GatewayTenantRow>(
            r#"
            select id, status
            from gateway_tenants
            where id = $1
            limit 1
            "#,
        )
        .bind(tenant_id)
        .fetch_optional(pool)
        .await
        .map_err(map_db_error)?;

        let Some(row) = row else {
            return Err(GatewayError::not_found("AI gateway tenant 不存在"));
        };
        if row.status != "active" {
            return Err(GatewayError::conflict("AI gateway tenant 未激活"));
        }
        Ok(row)
    }
}

pub(crate) fn map_db_error(error: sqlx::Error) -> GatewayError {
    match error {
        sqlx::Error::RowNotFound => GatewayError::not_found("资源不存在"),
        sqlx::Error::PoolTimedOut => GatewayError::service_unavailable("数据库连接池已耗尽"),
        sqlx::Error::Database(db_err) => match db_err.code().as_deref() {
            Some("23505") => GatewayError::conflict("资源已存在"),
            Some("23503") => GatewayError::bad_request("引用的资源不存在"),
            _ => GatewayError::server_error(format!("database error: {db_err}")),
        },
        other => GatewayError::server_error(format!("database error: {other}")),
    }
}

pub(crate) fn map_db_decode_error(error: sqlx::Error) -> GatewayError {
    GatewayError::server_error(format!("decode database row: {error}"))
}

pub(crate) fn format_timestamp(value: OffsetDateTime) -> String {
    value
        .format(&Rfc3339)
        .unwrap_or_else(|_| value.unix_timestamp().to_string())
}
