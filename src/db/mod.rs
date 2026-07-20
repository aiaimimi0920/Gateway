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
    GatewaySourceProfileView, GatewaySummaryBucketView,
};
pub use provider_accounts::{
    create_provider_account, delete_provider_account, get_provider_account, list_provider_accounts,
    update_provider_account, GatewayProviderAccountView, UpsertProviderAccountInput,
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
    resolve_route_candidates, save_model_alias, save_route_policy, GatewayModelAliasView,
    GatewayRateLimitDefinition, GatewayRoutePolicyConfig, GatewayRoutePolicyConfigInput,
    GatewayRoutePolicyView, ResolvedProjectRouteContext, SaveModelAliasInput, SaveRoutePolicyInput,
};
pub use sessions::{
    resolve_gateway_session, upsert_gateway_session, GatewaySessionView, UpsertGatewaySessionInput,
};
pub use usage_aggregates::{
    list_usage_aggregate_buckets, summarize_usage_aggregates, upsert_usage_aggregate_buckets,
    GatewayUsageAggregateAlertView, GatewayUsageAggregateBucketView,
    GatewayUsageAggregateSummaryView, UsageAggregateFilters,
};

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GatewayTenantView {
    pub id: String,
    pub slug: String,
    pub display_name: String,
    pub status: String,
    pub owner_user_id: Option<String>,
    pub source_kind: String,
    pub source_key: String,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GatewayProjectView {
    pub id: String,
    pub tenant_id: String,
    pub slug: String,
    pub display_name: String,
    pub status: String,
    pub source_kind: String,
    pub source_key: String,
    pub default_route_policy_id: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GatewayApiKeyView {
    pub id: String,
    pub project_id: String,
    pub name: String,
    pub status: String,
    pub issued_at: String,
    pub revoked_at: Option<String>,
    pub rotated_from_api_key_id: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GatewayProjectApiAccessView {
    pub project: GatewayProjectView,
    pub tenant: GatewayTenantView,
    pub api_key: GatewayApiKeyView,
    pub token: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GatewayBenefitProjectEnsureView {
    pub tenant: GatewayTenantView,
    pub project: GatewayProjectView,
    pub route_policy: GatewayRoutePolicyView,
}

#[derive(Debug, Clone, Serialize)]
pub struct GatewayUserCredentialCacheEntry {
    pub id: String,
    pub user_id: String,
    pub project_id: String,
    pub scope: Vec<String>,
    pub expires_at: String,
    pub status: String,
    pub tenant_id: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct GatewayProviderPayloadView {
    pub provider_account_id: String,
    pub payload: Value,
    pub storage_mode: String,
    pub status: String,
    pub updated_at: String,
}

#[derive(Debug, Clone)]
pub struct ProviderCredentialMutation {
    pub provider_account_id: String,
    pub payload: Value,
    pub ttl_seconds: Option<u64>,
    pub pre_warm: bool,
}

#[derive(Debug, Clone)]
pub struct UserCredentialIssueInput {
    pub user_id: String,
    pub project_id: String,
    pub credential_type: String,
    pub duration_days: i64,
    pub scope: Vec<String>,
    pub metadata: Option<Value>,
}

#[derive(Debug, Clone, Serialize)]
pub struct IssuedUserCredential {
    pub id: String,
    pub credential_key: String,
    pub expires_at: String,
    pub scope: Vec<String>,
    pub user_id: String,
    pub project_id: String,
    pub tenant_id: String,
    pub credential_type: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct VerifiedUserCredential {
    pub valid: bool,
    pub credential: Option<GatewayUserCredentialCacheEntry>,
    pub reason: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct BatchProviderCredentialOutcome {
    pub provider_account_id: String,
    pub action: String,
    pub success: bool,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct BatchProviderCredentialSummary {
    pub total: usize,
    pub succeeded: usize,
    pub failed: usize,
    pub cache_invalidated: u64,
    pub pre_warmed: usize,
}

#[derive(Debug, Clone, Serialize)]
pub struct BatchProviderCredentialResult {
    pub results: Vec<BatchProviderCredentialOutcome>,
    pub summary: BatchProviderCredentialSummary,
}

#[derive(Debug, Clone)]
pub struct BatchProviderCredentialOperation {
    pub action: String,
    pub provider_account_id: String,
    pub payload: Option<Value>,
}

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
pub struct GatewayApiKeyAuthRow {
    pub api_key_id: String,
    pub project_id: String,
    pub tenant_id: String,
    pub api_key_status: String,
    pub project_status: String,
    pub tenant_status: String,
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

pub async fn find_gateway_api_key_auth(
    pool: &PgPool,
    api_key_id: &str,
) -> Result<Option<GatewayApiKeyAuthRow>, GatewayError> {
    sqlx::query_as::<_, GatewayApiKeyAuthRow>(
        r#"
        select
          ak.id as api_key_id,
          ak.project_id,
          p.tenant_id,
          ak.status as api_key_status,
          p.status as project_status,
          t.status as tenant_status
        from gateway_api_keys ak
        join gateway_projects p on p.id = ak.project_id
        join gateway_tenants t on t.id = p.tenant_id
        where ak.id = $1
        limit 1
        "#,
    )
    .bind(api_key_id)
    .fetch_optional(pool)
    .await
    .map_err(map_db_error)
}

pub async fn resolve_or_create_gateway_api_access(
    pool: &PgPool,
    project_id: &str,
    name: &str,
    api_key_secret: &str,
) -> Result<GatewayProjectApiAccessView, GatewayError> {
    let project = get_active_project_detail(pool, project_id).await?;
    let tenant = get_active_tenant_detail(pool, &project.tenant_id).await?;

    let existing = sqlx::query_as::<_, GatewayApiKeyDetailRow>(
        r#"
        select id, project_id, name, status, rotated_from_api_key_id, revoked_at, created_at
        from gateway_api_keys
        where project_id = $1 and status = 'active'
        order by created_at desc, id desc
        limit 1
        "#,
    )
    .bind(project_id)
    .fetch_optional(pool)
    .await
    .map_err(map_db_error)?;

    let api_key = if let Some(row) = existing {
        row
    } else {
        let now = OffsetDateTime::now_utc();
        let api_key_id = Uuid::new_v4().to_string();
        sqlx::query_as::<_, GatewayApiKeyDetailRow>(
            r#"
            insert into gateway_api_keys (
              id,
              project_id,
              name,
              status,
              rotated_from_api_key_id,
              revoked_at,
              revoked_by_user_id,
              revoke_reason,
              created_at,
              updated_at
            ) values ($1, $2, $3, 'active', null, null, null, null, $4, $4)
            returning id, project_id, name, status, rotated_from_api_key_id, revoked_at, created_at
            "#,
        )
        .bind(&api_key_id)
        .bind(project_id)
        .bind(name)
        .bind(now)
        .fetch_one(pool)
        .await
        .map_err(map_db_error)?
    };

    let token = crate::gateway_api_key::build_gateway_project_api_key(
        &api_key.id,
        &project.id,
        &tenant.id,
        api_key_secret,
    );

    Ok(GatewayProjectApiAccessView {
        project: gateway_project_view_from_row(project),
        tenant: gateway_tenant_view_from_row(tenant),
        api_key: gateway_api_key_view_from_row(api_key),
        token,
    })
}

pub async fn rotate_gateway_api_access(
    pool: &PgPool,
    project_id: &str,
    name: &str,
    actor_user_id: Option<&str>,
    api_key_secret: &str,
) -> Result<GatewayProjectApiAccessView, GatewayError> {
    let project = get_active_project_detail(pool, project_id).await?;
    let tenant = get_active_tenant_detail(pool, &project.tenant_id).await?;
    let now = OffsetDateTime::now_utc();

    let current = sqlx::query_as::<_, GatewayApiKeyDetailRow>(
        r#"
        select id, project_id, name, status, rotated_from_api_key_id, revoked_at, created_at
        from gateway_api_keys
        where project_id = $1 and status = 'active'
        order by created_at desc, id desc
        limit 1
        "#,
    )
    .bind(project_id)
    .fetch_optional(pool)
    .await
    .map_err(map_db_error)?;

    let rotated_from = if let Some(row) = current {
        sqlx::query(
            r#"
            update gateway_api_keys
            set
              status = 'revoked',
              revoked_at = $2,
              revoked_by_user_id = $3,
              revoke_reason = 'rotated',
              updated_at = $2
            where id = $1
            "#,
        )
        .bind(&row.id)
        .bind(now)
        .bind(actor_user_id)
        .execute(pool)
        .await
        .map_err(map_db_error)?;
        Some(row.id)
    } else {
        None
    };

    let next_id = Uuid::new_v4().to_string();
    let next = sqlx::query_as::<_, GatewayApiKeyDetailRow>(
        r#"
        insert into gateway_api_keys (
          id,
          project_id,
          name,
          status,
          rotated_from_api_key_id,
          revoked_at,
          revoked_by_user_id,
          revoke_reason,
          created_at,
          updated_at
        ) values ($1, $2, $3, 'active', $4, null, null, null, $5, $5)
        returning id, project_id, name, status, rotated_from_api_key_id, revoked_at, created_at
        "#,
    )
    .bind(&next_id)
    .bind(project_id)
    .bind(name)
    .bind(rotated_from.as_deref())
    .bind(now)
    .fetch_one(pool)
    .await
    .map_err(map_db_error)?;

    let token = crate::gateway_api_key::build_gateway_project_api_key(
        &next.id,
        &project.id,
        &tenant.id,
        api_key_secret,
    );

    Ok(GatewayProjectApiAccessView {
        project: gateway_project_view_from_row(project),
        tenant: gateway_tenant_view_from_row(tenant),
        api_key: gateway_api_key_view_from_row(next),
        token,
    })
}

pub async fn ensure_benefit_project(
    pool: &PgPool,
    service_id: &str,
    user_id: &str,
    service_title: Option<&str>,
) -> Result<GatewayBenefitProjectEnsureView, GatewayError> {
    let normalized_service_id = service_id.trim();
    let normalized_user_id = user_id.trim();
    if normalized_service_id.is_empty() {
        return Err(GatewayError::bad_request("serviceId 不能为空"));
    }
    if normalized_user_id.is_empty() {
        return Err(GatewayError::bad_request("userId 不能为空"));
    }

    let mut tx = pool.begin().await.map_err(map_db_error)?;
    let now = OffsetDateTime::now_utc();
    let tenant_source_key = build_benefit_tenant_source_key(normalized_user_id);
    let tenant = if let Some(existing) = sqlx::query_as::<_, GatewayTenantDetailRow>(
        r#"
        select id, slug, display_name, status, owner_user_id, source_kind, source_key, created_at, updated_at
        from gateway_tenants
        where source_kind = 'benefit_user' and source_key = $1
        limit 1
        "#,
    )
    .bind(&tenant_source_key)
    .fetch_optional(&mut *tx)
    .await
    .map_err(map_db_error)?
    {
        existing
    } else {
        sqlx::query_as::<_, GatewayTenantDetailRow>(
            r#"
            insert into gateway_tenants (
              id,
              slug,
              display_name,
              status,
              owner_user_id,
              source_kind,
              source_key,
              created_at,
              updated_at
            ) values ($1, $2, $3, 'active', $4, 'benefit_user', $5, $6, $6)
            returning id, slug, display_name, status, owner_user_id, source_kind, source_key, created_at, updated_at
            "#,
        )
        .bind(Uuid::new_v4().to_string())
        .bind(slugify_identifier(&format!("benefit-{normalized_user_id}")))
        .bind(format!("Benefit {normalized_user_id}"))
        .bind(normalized_user_id)
        .bind(&tenant_source_key)
        .bind(now)
        .fetch_one(&mut *tx)
        .await
        .map_err(map_db_error)?
    };

    let project_source_key =
        build_benefit_project_source_key(normalized_service_id, normalized_user_id);
    let service_display_name = service_title
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| normalized_service_id);
    let project = if let Some(existing) = sqlx::query_as::<_, GatewayProjectDetailRow>(
        r#"
        select
          id,
          tenant_id,
          slug,
          display_name,
          status,
          source_kind,
          source_key,
          default_route_policy_id,
          created_at,
          updated_at
        from gateway_projects
        where source_kind = 'benefit_service_user' and source_key = $1
        limit 1
        "#,
    )
    .bind(&project_source_key)
    .fetch_optional(&mut *tx)
    .await
    .map_err(map_db_error)?
    {
        existing
    } else {
        sqlx::query_as::<_, GatewayProjectDetailRow>(
            r#"
            insert into gateway_projects (
              id,
              tenant_id,
              slug,
              display_name,
              status,
              source_kind,
              source_key,
              default_route_policy_id,
              created_at,
              updated_at
            ) values ($1, $2, $3, $4, 'active', 'benefit_service_user', $5, null, $6, $6)
            returning
              id,
              tenant_id,
              slug,
              display_name,
              status,
              source_kind,
              source_key,
              default_route_policy_id,
              created_at,
              updated_at
            "#,
        )
        .bind(Uuid::new_v4().to_string())
        .bind(&tenant.id)
        .bind(slugify_identifier(&format!(
            "{service_display_name}-{normalized_user_id}"
        )))
        .bind(service_display_name)
        .bind(&project_source_key)
        .bind(now)
        .fetch_one(&mut *tx)
        .await
        .map_err(map_db_error)?
    };

    let default_route_policy_id =
        ensure_default_route_policy_in_tx(&mut tx, &project.id, now).await?;
    tx.commit().await.map_err(map_db_error)?;

    let project = get_project_detail(pool, &project.id)
        .await?
        .ok_or_else(|| GatewayError::not_found("AI gateway project 不存在"))?;
    let route_policy = list_route_policies(pool, &project.id)
        .await?
        .into_iter()
        .find(|policy| policy.id == default_route_policy_id)
        .ok_or_else(|| GatewayError::conflict("当前 benefit project 尚未生成默认 route policy"))?;

    Ok(GatewayBenefitProjectEnsureView {
        tenant: gateway_tenant_view_from_row(tenant),
        project: gateway_project_view_from_row(project),
        route_policy,
    })
}

pub async fn find_user_credential(
    pool: &PgPool,
    credential_key: &str,
) -> Result<Option<GatewayUserCredentialCacheEntry>, GatewayError> {
    let row = sqlx::query_as::<_, GatewayUserCredentialJoinRow>(
        r#"
        select
          c.id,
          c.user_id,
          c.project_id,
          c.credential_key,
          c.credential_type,
          c.status,
          c.expires_at,
          c.scope,
          c.metadata,
          p.tenant_id,
          p.status as project_status,
          t.status as tenant_status
        from gateway_user_credentials c
        join gateway_projects p on p.id = c.project_id
        join gateway_tenants t on t.id = p.tenant_id
        where c.credential_key = $1
        limit 1
        "#,
    )
    .bind(credential_key)
    .fetch_optional(pool)
    .await
    .map_err(map_db_error)?;

    Ok(row.map(|row| GatewayUserCredentialCacheEntry {
        id: row.id,
        user_id: row.user_id,
        project_id: row.project_id,
        scope: row.scope.0,
        expires_at: format_timestamp(row.expires_at),
        status: row.status,
        tenant_id: row.tenant_id,
    }))
}

pub async fn verify_user_credential(
    pool: &PgPool,
    credential_key: &str,
    required_scope: Option<&str>,
) -> Result<VerifiedUserCredential, GatewayError> {
    let row = sqlx::query_as::<_, GatewayUserCredentialJoinRow>(
        r#"
        select
          c.id,
          c.user_id,
          c.project_id,
          c.credential_key,
          c.credential_type,
          c.status,
          c.expires_at,
          c.scope,
          c.metadata,
          p.tenant_id,
          p.status as project_status,
          t.status as tenant_status
        from gateway_user_credentials c
        join gateway_projects p on p.id = c.project_id
        join gateway_tenants t on t.id = p.tenant_id
        where c.credential_key = $1
        limit 1
        "#,
    )
    .bind(credential_key)
    .fetch_optional(pool)
    .await
    .map_err(map_db_error)?;

    let Some(row) = row else {
        return Ok(VerifiedUserCredential {
            valid: false,
            credential: None,
            reason: Some("凭证不存在".to_string()),
        });
    };

    if row.status != "active" {
        return Ok(VerifiedUserCredential {
            valid: false,
            credential: None,
            reason: Some(format!("凭证状态为 {}", row.status)),
        });
    }
    if row.project_status != "active" || row.tenant_status != "active" {
        return Ok(VerifiedUserCredential {
            valid: false,
            credential: None,
            reason: Some("凭证所属 project 或 tenant 不可用".to_string()),
        });
    }
    if row.expires_at <= OffsetDateTime::now_utc() {
        return Ok(VerifiedUserCredential {
            valid: false,
            credential: None,
            reason: Some("凭证已过期".to_string()),
        });
    }
    if let Some(scope) = required_scope {
        if !row.scope.0.iter().any(|item| item == scope) {
            return Ok(VerifiedUserCredential {
                valid: false,
                credential: None,
                reason: Some(format!("凭证没有 {scope} 权限")),
            });
        }
    }

    let credential = GatewayUserCredentialCacheEntry {
        id: row.id.clone(),
        user_id: row.user_id.clone(),
        project_id: row.project_id.clone(),
        scope: row.scope.0.clone(),
        expires_at: format_timestamp(row.expires_at),
        status: row.status.clone(),
        tenant_id: row.tenant_id.clone(),
    };

    sqlx::query(
        r#"
        update gateway_user_credentials
        set last_used_at = $2, updated_at = $2
        where id = $1
        "#,
    )
    .bind(&row.id)
    .bind(OffsetDateTime::now_utc())
    .execute(pool)
    .await
    .map_err(map_db_error)?;

    Ok(VerifiedUserCredential {
        valid: true,
        credential: Some(credential),
        reason: None,
    })
}

pub async fn issue_user_credential(
    pool: &PgPool,
    input: UserCredentialIssueInput,
) -> Result<IssuedUserCredential, GatewayError> {
    let project = get_active_project(pool, &input.project_id).await?;
    let tenant = get_active_tenant(pool, &project.tenant_id).await?;
    let now = OffsetDateTime::now_utc();
    let expires_at = now + time::Duration::days(input.duration_days.max(1));
    let credential_id = format!("cred-{}", Uuid::new_v4().simple());
    let credential_key = format!("gw-user-{}", Uuid::new_v4().simple());

    sqlx::query(
        r#"
        insert into gateway_user_credentials (
          id,
          user_id,
          project_id,
          credential_key,
          credential_type,
          status,
          expires_at,
          scope,
          metadata,
          created_at,
          updated_at,
          last_used_at,
          revoked_at,
          revoke_reason
        ) values ($1, $2, $3, $4, $5, 'active', $6, $7, $8, $9, $9, null, null, null)
        "#,
    )
    .bind(&credential_id)
    .bind(&input.user_id)
    .bind(&input.project_id)
    .bind(&credential_key)
    .bind(&input.credential_type)
    .bind(expires_at)
    .bind(Json(input.scope.clone()))
    .bind(input.metadata.clone().map(Json))
    .bind(now)
    .execute(pool)
    .await
    .map_err(map_db_error)?;

    Ok(IssuedUserCredential {
        id: credential_id,
        credential_key,
        expires_at: format_timestamp(expires_at),
        scope: input.scope,
        user_id: input.user_id,
        project_id: project.id,
        tenant_id: tenant.id,
        credential_type: input.credential_type,
    })
}

pub async fn revoke_user_credential(
    pool: &PgPool,
    credential_key: &str,
    reason: Option<&str>,
) -> Result<(), GatewayError> {
    let now = OffsetDateTime::now_utc();
    let result = sqlx::query(
        r#"
        update gateway_user_credentials
        set
          status = 'revoked',
          revoked_at = $2,
          revoke_reason = $3,
          updated_at = $2
        where credential_key = $1
        "#,
    )
    .bind(credential_key)
    .bind(now)
    .bind(reason)
    .execute(pool)
    .await
    .map_err(map_db_error)?;

    if result.rows_affected() == 0 {
        return Err(GatewayError::not_found("凭证不存在"));
    }
    Ok(())
}

pub async fn get_provider_payload(
    pool: &PgPool,
    provider_account_id: &str,
) -> Result<Option<GatewayProviderPayloadView>, GatewayError> {
    let row = sqlx::query_as::<_, GatewayProviderPayloadRow>(
        r#"
        select id, payload_inline, payload_object_key, storage_mode, status, updated_at
        from gateway_provider_accounts
        where id = $1
        limit 1
        "#,
    )
    .bind(provider_account_id)
    .fetch_optional(pool)
    .await
    .map_err(map_db_error)?;

    match row {
        None => Ok(None),
        Some(row) => {
            let payload = match row.payload_inline {
                Some(payload) => payload.0,
                None if row.payload_object_key.is_some() => {
                    gateway_object_storage()?
                        .read_json(row.payload_object_key.as_deref().unwrap_or_default())
                        .await?
                }
                None => return Err(GatewayError::conflict("Provider account payload 缺失")),
            };
            Ok(Some(GatewayProviderPayloadView {
                provider_account_id: row.id,
                payload,
                storage_mode: row.storage_mode,
                status: row.status,
                updated_at: format_timestamp(row.updated_at),
            }))
        }
    }
}

pub async fn save_provider_payload_inline(
    pool: &PgPool,
    provider_account_id: &str,
    payload: Value,
) -> Result<GatewayProviderPayloadView, GatewayError> {
    let now = OffsetDateTime::now_utc();
    let existing = sqlx::query_as::<_, GatewayProviderPayloadRow>(
        r#"
        select id, payload_inline, payload_object_key, storage_mode, status, updated_at
        from gateway_provider_accounts
        where id = $1
        limit 1
        "#,
    )
    .bind(provider_account_id)
    .fetch_optional(pool)
    .await
    .map_err(map_db_error)?
    .ok_or_else(|| GatewayError::not_found("Provider account 不存在"))?;

    let storage_mode = choose_provider_payload_storage_mode(&payload);
    let object_storage = gateway_object_storage()?;
    let payload_object_key = if storage_mode == "inline" {
        if let Some(existing_key) = existing.payload_object_key.as_deref() {
            object_storage.delete_object(existing_key).await?;
        }
        None
    } else {
        let object_key = existing
            .payload_object_key
            .clone()
            .unwrap_or_else(|| build_gateway_provider_account_object_key(provider_account_id));
        object_storage.put_json(&object_key, &payload).await?;
        Some(object_key)
    };
    let updated = sqlx::query_as::<_, GatewayProviderPayloadRow>(
        r#"
        update gateway_provider_accounts
        set
          payload_inline = $2,
          payload_object_key = $3,
          payload_content_type = 'application/json',
          storage_mode = $4,
          updated_at = $5
        where id = $1
        returning id, payload_inline, payload_object_key, storage_mode, status, updated_at
        "#,
    )
    .bind(provider_account_id)
    .bind(
        (storage_mode == "inline")
            .then_some(Json(payload.clone()))
            .map(|value| value),
    )
    .bind(payload_object_key.as_deref())
    .bind(storage_mode)
    .bind(now)
    .fetch_optional(pool)
    .await
    .map_err(map_db_error)?;
    let row = updated.ok_or_else(|| GatewayError::not_found("Provider account 不存在"))?;

    Ok(GatewayProviderPayloadView {
        provider_account_id: row.id,
        payload,
        storage_mode: row.storage_mode,
        status: row.status,
        updated_at: format_timestamp(row.updated_at),
    })
}

pub async fn delete_provider_payload(
    pool: &PgPool,
    provider_account_id: &str,
) -> Result<(), GatewayError> {
    let existing = sqlx::query_as::<_, GatewayProviderPayloadRow>(
        r#"
        select id, payload_inline, payload_object_key, storage_mode, status, updated_at
        from gateway_provider_accounts
        where id = $1
        limit 1
        "#,
    )
    .bind(provider_account_id)
    .fetch_optional(pool)
    .await
    .map_err(map_db_error)?
    .ok_or_else(|| GatewayError::not_found("Provider account 不存在"))?;
    if let Some(existing_key) = existing.payload_object_key.as_deref() {
        gateway_object_storage()?
            .delete_object(existing_key)
            .await?;
    }
    let now = OffsetDateTime::now_utc();
    let result = sqlx::query(
        r#"
        update gateway_provider_accounts
        set
          payload_inline = null,
          payload_object_key = null,
          storage_mode = 'inline',
          updated_at = $2
        where id = $1
        "#,
    )
    .bind(provider_account_id)
    .bind(now)
    .execute(pool)
    .await
    .map_err(map_db_error)?;

    if result.rows_affected() == 0 {
        return Err(GatewayError::not_found("Provider account 不存在"));
    }
    Ok(())
}

pub async fn list_active_provider_account_ids(pool: &PgPool) -> Result<Vec<String>, GatewayError> {
    let rows = sqlx::query(
        r#"
        select id
        from gateway_provider_accounts
        where status = 'active'
        order by updated_at desc
        "#,
    )
    .fetch_all(pool)
    .await
    .map_err(map_db_error)?;

    rows.into_iter()
        .map(|row| row.try_get::<String, _>("id").map_err(map_db_decode_error))
        .collect()
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

fn build_benefit_tenant_source_key(user_id: &str) -> String {
    format!("benefit_user:{user_id}")
}

fn build_benefit_project_source_key(service_id: &str, user_id: &str) -> String {
    format!("benefit_service_user:{service_id}:{user_id}")
}

fn slugify_identifier(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    let mut last_dash = false;
    for ch in value.chars() {
        if ch.is_ascii_alphanumeric() {
            out.push(ch.to_ascii_lowercase());
            last_dash = false;
        } else if !last_dash && !out.is_empty() {
            out.push('-');
            last_dash = true;
        }
    }
    let normalized = out.trim_matches('-').to_string();
    if normalized.is_empty() {
        "gateway-item".to_string()
    } else {
        normalized
    }
}

fn gateway_tenant_view_from_row(row: GatewayTenantDetailRow) -> GatewayTenantView {
    GatewayTenantView {
        id: row.id,
        slug: row.slug,
        display_name: row.display_name,
        status: row.status,
        owner_user_id: row.owner_user_id,
        source_kind: row.source_kind,
        source_key: row.source_key,
        created_at: format_timestamp(row.created_at),
        updated_at: format_timestamp(row.updated_at),
    }
}

fn gateway_project_view_from_row(row: GatewayProjectDetailRow) -> GatewayProjectView {
    GatewayProjectView {
        id: row.id,
        tenant_id: row.tenant_id,
        slug: row.slug,
        display_name: row.display_name,
        status: row.status,
        source_kind: row.source_kind,
        source_key: row.source_key,
        default_route_policy_id: row.default_route_policy_id,
        created_at: format_timestamp(row.created_at),
        updated_at: format_timestamp(row.updated_at),
    }
}

fn gateway_api_key_view_from_row(row: GatewayApiKeyDetailRow) -> GatewayApiKeyView {
    GatewayApiKeyView {
        id: row.id,
        project_id: row.project_id,
        name: row.name,
        status: row.status,
        issued_at: format_timestamp(row.created_at),
        revoked_at: row.revoked_at.map(format_timestamp),
        rotated_from_api_key_id: row.rotated_from_api_key_id,
    }
}

async fn get_project_detail(
    pool: &PgPool,
    project_id: &str,
) -> Result<Option<GatewayProjectDetailRow>, GatewayError> {
    sqlx::query_as::<_, GatewayProjectDetailRow>(
        r#"
        select
          id,
          tenant_id,
          slug,
          display_name,
          status,
          source_kind,
          source_key,
          default_route_policy_id,
          created_at,
          updated_at
        from gateway_projects
        where id = $1
        limit 1
        "#,
    )
    .bind(project_id)
    .fetch_optional(pool)
    .await
    .map_err(map_db_error)
}

async fn get_active_project_detail(
    pool: &PgPool,
    project_id: &str,
) -> Result<GatewayProjectDetailRow, GatewayError> {
    let row = get_project_detail(pool, project_id)
        .await?
        .ok_or_else(|| GatewayError::not_found("AI gateway project 不存在"))?;
    if row.status != "active" {
        return Err(GatewayError::conflict("AI gateway project 未激活"));
    }
    Ok(row)
}

async fn get_tenant_detail(
    pool: &PgPool,
    tenant_id: &str,
) -> Result<Option<GatewayTenantDetailRow>, GatewayError> {
    sqlx::query_as::<_, GatewayTenantDetailRow>(
        r#"
        select id, slug, display_name, status, owner_user_id, source_kind, source_key, created_at, updated_at
        from gateway_tenants
        where id = $1
        limit 1
        "#,
    )
    .bind(tenant_id)
    .fetch_optional(pool)
    .await
    .map_err(map_db_error)
}

async fn get_active_tenant_detail(
    pool: &PgPool,
    tenant_id: &str,
) -> Result<GatewayTenantDetailRow, GatewayError> {
    let row = get_tenant_detail(pool, tenant_id)
        .await?
        .ok_or_else(|| GatewayError::not_found("AI gateway tenant 不存在"))?;
    if row.status != "active" {
        return Err(GatewayError::conflict("AI gateway tenant 未激活"));
    }
    Ok(row)
}

async fn ensure_default_route_policy_in_tx(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    project_id: &str,
    timestamp: OffsetDateTime,
) -> Result<String, GatewayError> {
    let existing = sqlx::query(
        r#"
        select id
        from gateway_route_policies
        where project_id = $1 and is_default = true
        limit 1
        "#,
    )
    .bind(project_id)
    .fetch_optional(&mut **tx)
    .await
    .map_err(map_db_error)?;

    let route_policy_id = if let Some(row) = existing {
        row.try_get::<String, _>("id")
            .map_err(map_db_decode_error)?
    } else {
        let route_policy_id = Uuid::new_v4().to_string();
        let config = normalize_route_policy_config(GatewayRoutePolicyConfigInput::default())?;
        let config_json = serde_json::to_value(config).map_err(|error| {
            GatewayError::server_error(format!("serialize default route policy config: {error}"))
        })?;
        sqlx::query(
            r#"
            insert into gateway_route_policies (
              id,
              project_id,
              name,
              is_default,
              enabled,
              config,
              created_at,
              updated_at
            ) values ($1, $2, 'default', true, true, $3, $4, $4)
            "#,
        )
        .bind(&route_policy_id)
        .bind(project_id)
        .bind(Json(config_json))
        .bind(timestamp)
        .execute(&mut **tx)
        .await
        .map_err(map_db_error)?;
        route_policy_id
    };

    sqlx::query(
        r#"
        update gateway_projects
        set default_route_policy_id = $2, updated_at = $3
        where id = $1 and (default_route_policy_id is null or default_route_policy_id <> $2)
        "#,
    )
    .bind(project_id)
    .bind(&route_policy_id)
    .bind(timestamp)
    .execute(&mut **tx)
    .await
    .map_err(map_db_error)?;

    Ok(route_policy_id)
}
