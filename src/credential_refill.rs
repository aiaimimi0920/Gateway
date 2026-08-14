use std::collections::HashSet;
use std::sync::Arc;

use redis::Script;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use time::format_description::well_known::Rfc3339;
use time::OffsetDateTime;
use uuid::Uuid;

use crate::error::GatewayError;
use crate::provider_credential_folder_sync::{run_folder_sync_once, FolderSyncDirection};
use crate::redis::keys;
use crate::routing::config::{ProviderConfigYaml, ProviderCredentialYaml};
use crate::state::AppState;

const MAX_REQUESTED_COUNT: usize = 10_000;
const MAX_TASK_LIST_LIMIT: usize = 200;
const MAX_WORKER_ID_LENGTH: usize = 128;
const MAX_IDEMPOTENCY_KEY_LENGTH: usize = 160;
const MAX_FAILURE_REASON_LENGTH: usize = 320;
const MAX_ARTIFACT_REFERENCE_LENGTH: usize = 2_048;
const MAX_FOLDER_PATHS: usize = 128;
const MAX_FOLDER_PATH_LENGTH: usize = 512;

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CredentialRefillTrigger {
    Notification,
    Inquiry,
    UserRequested,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CredentialRefillTaskState {
    Pending,
    Claimed,
    Succeeded,
    Failed,
}

impl CredentialRefillTaskState {
    fn is_terminal(self) -> bool {
        matches!(self, Self::Succeeded | Self::Failed)
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CredentialRefillDeliveryMode {
    FolderSync,
    GatewayPull,
    DirectCallback,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CredentialRefillDemandView {
    pub provider_id: String,
    pub provider_label: String,
    pub target_size: usize,
    pub credential_count: usize,
    pub active_credential_count: usize,
    pub deficit: usize,
    pub needs_refill: bool,
    pub auto_refill_enabled: bool,
    pub direct_driver_configured: bool,
    pub notification_enabled: bool,
    pub inquiry_enabled: bool,
    pub user_request_enabled: bool,
    pub outstanding_task_id: Option<String>,
    pub outstanding_task_state: Option<CredentialRefillTaskState>,
    pub revision_id: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct CredentialRefillTaskRecord {
    id: String,
    provider_id: String,
    provider_label: String,
    trigger: CredentialRefillTrigger,
    state: CredentialRefillTaskState,
    requested_count: usize,
    target_size: usize,
    active_credential_count: usize,
    route_revision: String,
    created_at: String,
    updated_at: String,
    #[serde(default)]
    worker_id: Option<String>,
    #[serde(default)]
    claim_token: Option<String>,
    #[serde(default)]
    lease_until: Option<String>,
    #[serde(default)]
    attempt: usize,
    #[serde(default)]
    delivery_mode: Option<CredentialRefillDeliveryMode>,
    #[serde(default)]
    created_count: usize,
    #[serde(default)]
    message: Option<String>,
    #[serde(default)]
    revision_id: Option<String>,
    #[serde(default)]
    idempotency_key: Option<String>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CredentialRefillTaskView {
    pub id: String,
    pub provider_id: String,
    pub provider_label: String,
    pub trigger: CredentialRefillTrigger,
    pub state: CredentialRefillTaskState,
    pub requested_count: usize,
    pub target_size: usize,
    pub active_credential_count: usize,
    pub route_revision: String,
    pub created_at: String,
    pub updated_at: String,
    pub worker_id: Option<String>,
    pub lease_until: Option<String>,
    pub attempt: usize,
    pub delivery_mode: Option<CredentialRefillDeliveryMode>,
    pub created_count: usize,
    pub message: Option<String>,
    pub revision_id: Option<String>,
}

impl From<&CredentialRefillTaskRecord> for CredentialRefillTaskView {
    fn from(record: &CredentialRefillTaskRecord) -> Self {
        Self {
            id: record.id.clone(),
            provider_id: record.provider_id.clone(),
            provider_label: record.provider_label.clone(),
            trigger: record.trigger,
            state: record.state,
            requested_count: record.requested_count,
            target_size: record.target_size,
            active_credential_count: record.active_credential_count,
            route_revision: record.route_revision.clone(),
            created_at: record.created_at.clone(),
            updated_at: record.updated_at.clone(),
            worker_id: record.worker_id.clone(),
            lease_until: record.lease_until.clone(),
            attempt: record.attempt,
            delivery_mode: record.delivery_mode,
            created_count: record.created_count,
            message: record.message.clone(),
            revision_id: record.revision_id.clone(),
        }
    }
}

#[derive(Clone, Debug, Deserialize, Default)]
#[serde(default, deny_unknown_fields, rename_all = "camelCase")]
pub struct CreateCredentialRefillRequestInput {
    pub requested_count: Option<usize>,
    pub idempotency_key: Option<String>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateCredentialRefillTaskResult {
    pub task: CredentialRefillTaskView,
    pub created: bool,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct ClaimCredentialRefillTaskInput {
    pub worker_id: String,
    #[serde(default)]
    pub task_id: Option<String>,
    #[serde(default)]
    pub provider_ids: Vec<String>,
    #[serde(default)]
    pub lease_seconds: Option<u64>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ClaimCredentialRefillTaskResult {
    pub task: CredentialRefillTaskView,
    pub claim_token: String,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct RenewCredentialRefillTaskInput {
    pub claim_token: String,
    #[serde(default)]
    pub lease_seconds: Option<u64>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct FailCredentialRefillTaskInput {
    pub claim_token: String,
    pub reason: String,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct CompleteCredentialRefillTaskInput {
    pub claim_token: String,
    pub delivery: CredentialRefillDeliveryInput,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(tag = "mode", rename_all = "snake_case")]
pub enum CredentialRefillDeliveryInput {
    FolderSync {
        #[serde(default, rename = "relativePaths")]
        relative_paths: Vec<String>,
    },
    GatewayPull {
        #[serde(rename = "artifactReference")]
        artifact_reference: String,
    },
    DirectCallback {
        credentials: Vec<ProviderCredentialYaml>,
    },
}

#[derive(Clone, Debug, Deserialize, Default)]
#[serde(default, deny_unknown_fields, rename_all = "camelCase")]
pub struct CredentialRefillTaskFilters {
    pub provider_id: Option<String>,
    pub state: Option<CredentialRefillTaskState>,
    pub limit: Option<usize>,
}

#[derive(Debug)]
struct DeliveryOutcome {
    mode: CredentialRefillDeliveryMode,
    created_count: usize,
    message: String,
    revision_id: Option<String>,
}

pub fn stream_key() -> &'static str {
    keys::credential_refill_stream_key()
}

pub async fn start_credential_refill_notification_task(state: Arc<AppState>) {
    if !state.credential_pool_automation.refill_queue_enabled() {
        tracing::info!("credential refill queue is disabled");
        return;
    }
    loop {
        if let Err(error) = publish_notification_refill_demands_once(state.as_ref()).await {
            tracing::warn!(
                code = ?error.code,
                "credential refill notification sweep failed"
            );
        }
        tokio::time::sleep(
            state
                .credential_pool_automation
                .refill_notification_interval(),
        )
        .await;
    }
}

pub async fn list_credential_refill_demands(
    state: &AppState,
) -> Result<Vec<CredentialRefillDemandView>, GatewayError> {
    let snapshot = state.route_config.snapshot();
    let mut demands = Vec::with_capacity(snapshot.document().providers.len());
    for provider in &snapshot.document().providers {
        let outstanding = load_outstanding_task(state, &provider.id).await?;
        demands.push(build_demand_view(
            state,
            provider,
            snapshot.revision().id(),
            outstanding.as_ref(),
        ));
    }
    Ok(demands)
}

pub async fn publish_notification_refill_demands_once(
    state: &AppState,
) -> Result<usize, GatewayError> {
    let demands = list_credential_refill_demands(state).await?;
    let mut published = 0usize;
    for demand in demands
        .into_iter()
        .filter(|demand| demand.notification_enabled && demand.needs_refill)
    {
        let result = create_task_from_demand(
            state,
            &demand,
            CredentialRefillTrigger::Notification,
            demand.deficit,
            Some(format!(
                "notification:{}:{}",
                demand.provider_id, demand.revision_id
            )),
        )
        .await?;
        published += usize::from(result.created);
    }
    Ok(published)
}

pub async fn create_user_requested_refill_task(
    state: &AppState,
    provider_id: &str,
    input: CreateCredentialRefillRequestInput,
) -> Result<CreateCredentialRefillTaskResult, GatewayError> {
    ensure_refill_enabled(state)?;
    let demand = demand_for_provider(state, provider_id).await?;
    let requested_count = input
        .requested_count
        .unwrap_or_else(|| demand.deficit.max(1));
    validate_requested_count(requested_count)?;
    let idempotency_key = normalize_optional_text(
        input.idempotency_key,
        "idempotencyKey",
        MAX_IDEMPOTENCY_KEY_LENGTH,
    )?;
    create_task_from_demand(
        state,
        &demand,
        CredentialRefillTrigger::UserRequested,
        requested_count,
        idempotency_key,
    )
    .await
}

pub async fn list_credential_refill_tasks(
    state: &AppState,
    filters: CredentialRefillTaskFilters,
) -> Result<Vec<CredentialRefillTaskView>, GatewayError> {
    let limit = filters.limit.unwrap_or(50).clamp(1, MAX_TASK_LIST_LIMIT);
    let provider_filter = filters
        .provider_id
        .as_deref()
        .map(|value| normalize_identifier(value, "providerId", MAX_WORKER_ID_LENGTH))
        .transpose()?;
    prune_expired_task_indexes(state).await?;
    let mut conn = redis_connection(state).await?;
    let ids: Vec<String> = redis::cmd("ZREVRANGE")
        .arg(keys::credential_refill_recent_tasks_key())
        .arg(0)
        .arg((limit.saturating_mul(4)).saturating_sub(1))
        .query_async(&mut conn)
        .await
        .map_err(|_| redis_operation_error())?;
    drop(conn);

    let mut tasks = Vec::with_capacity(limit);
    for id in ids {
        let Some(task) = load_task(state, &id).await? else {
            continue;
        };
        if provider_filter
            .as_deref()
            .is_some_and(|provider_id| task.provider_id != provider_id)
        {
            continue;
        }
        if filters.state.is_some_and(|state| task.state != state) {
            continue;
        }
        tasks.push(CredentialRefillTaskView::from(&task));
        if tasks.len() == limit {
            break;
        }
    }
    Ok(tasks)
}

pub async fn claim_credential_refill_task(
    state: &AppState,
    input: ClaimCredentialRefillTaskInput,
) -> Result<ClaimCredentialRefillTaskResult, GatewayError> {
    ensure_refill_enabled(state)?;
    let worker_id = normalize_identifier(&input.worker_id, "workerId", MAX_WORKER_ID_LENGTH)?;
    let provider_ids = input
        .provider_ids
        .iter()
        .map(|value| normalize_identifier(value, "providerIds", MAX_WORKER_ID_LENGTH))
        .collect::<Result<HashSet<_>, _>>()?;
    let lease_seconds = normalized_lease_seconds(state, input.lease_seconds);

    let candidates = if let Some(task_id) = input.task_id.as_deref() {
        vec![normalize_identifier(
            task_id,
            "taskId",
            MAX_WORKER_ID_LENGTH,
        )?]
    } else {
        pending_task_ids(state, MAX_TASK_LIST_LIMIT).await?
    };

    for task_id in candidates {
        let Some(task) = load_task(state, &task_id).await? else {
            continue;
        };
        if task.state.is_terminal()
            || (!provider_ids.is_empty() && !provider_ids.contains(&task.provider_id))
        {
            continue;
        }
        if let Some(claimed) = try_claim_task(state, task, &worker_id, lease_seconds).await? {
            return Ok(claimed);
        }
    }
    if input.task_id.is_none() {
        let demands = list_credential_refill_demands(state).await?;
        for demand in demands.into_iter().filter(|demand| {
            demand.inquiry_enabled
                && demand.needs_refill
                && (provider_ids.is_empty() || provider_ids.contains(&demand.provider_id))
        }) {
            let created = create_task_from_demand(
                state,
                &demand,
                CredentialRefillTrigger::Inquiry,
                demand.deficit,
                None,
            )
            .await?;
            let Some(task) = load_task(state, &created.task.id).await? else {
                continue;
            };
            if task.state.is_terminal() {
                continue;
            }
            if let Some(claimed) = try_claim_task(state, task, &worker_id, lease_seconds).await? {
                return Ok(claimed);
            }
        }
    }
    Err(GatewayError::conflict("当前没有可认领的补号任务")
        .with_code("credential_refill_task_unavailable"))
}

pub async fn renew_credential_refill_task(
    state: &AppState,
    task_id: &str,
    input: RenewCredentialRefillTaskInput,
) -> Result<CredentialRefillTaskView, GatewayError> {
    ensure_refill_enabled(state)?;
    let task_id = normalize_identifier(task_id, "taskId", MAX_WORKER_ID_LENGTH)?;
    let claim_token = normalize_identifier(&input.claim_token, "claimToken", MAX_WORKER_ID_LENGTH)?;
    let mut task = require_claimed_task(state, &task_id, &claim_token).await?;
    let lease_seconds = normalized_lease_seconds(state, input.lease_seconds);
    task.updated_at = now_rfc3339();
    task.lease_until = Some(future_rfc3339(lease_seconds));
    let payload = serialize_task(&task)?;
    let mut conn = redis_connection(state).await?;
    let renewed: i64 = Script::new(
        r#"
local token = redis.call('GET', KEYS[1])
if token ~= ARGV[1] then return 0 end
redis.call('EXPIRE', KEYS[1], ARGV[2])
redis.call('SET', KEYS[2], ARGV[3], 'EX', ARGV[4])
if redis.call('GET', KEYS[3]) == ARGV[5] then
  redis.call('EXPIRE', KEYS[3], ARGV[4])
end
return 1
"#,
    )
    .key(keys::credential_refill_lease_key(&task_id))
    .key(keys::credential_refill_task_key(&task_id))
    .key(keys::credential_refill_outstanding_key(&task.provider_id))
    .arg(&claim_token)
    .arg(lease_seconds)
    .arg(payload)
    .arg(state.credential_pool_automation.refill_task_ttl_seconds())
    .arg(&task_id)
    .invoke_async(&mut conn)
    .await
    .map_err(|_| redis_operation_error())?;
    if renewed != 1 {
        return Err(claim_conflict());
    }
    Ok(CredentialRefillTaskView::from(&task))
}

pub async fn complete_credential_refill_task(
    state: &Arc<AppState>,
    task_id: &str,
    input: CompleteCredentialRefillTaskInput,
) -> Result<CredentialRefillTaskView, GatewayError> {
    ensure_refill_enabled(state.as_ref())?;
    let task_id = normalize_identifier(task_id, "taskId", MAX_WORKER_ID_LENGTH)?;
    let claim_token = normalize_identifier(&input.claim_token, "claimToken", MAX_WORKER_ID_LENGTH)?;
    let mut task = require_claimed_task(state.as_ref(), &task_id, &claim_token).await?;
    let outcome = deliver_refill_result(state, &task, input.delivery).await?;
    task.state = CredentialRefillTaskState::Succeeded;
    task.updated_at = now_rfc3339();
    task.lease_until = None;
    task.claim_token = None;
    task.delivery_mode = Some(outcome.mode);
    task.created_count = outcome.created_count;
    task.message = Some(outcome.message);
    task.revision_id = outcome.revision_id;
    finish_task(state.as_ref(), &task, &claim_token).await?;
    Ok(CredentialRefillTaskView::from(&task))
}

pub async fn fail_credential_refill_task(
    state: &AppState,
    task_id: &str,
    input: FailCredentialRefillTaskInput,
) -> Result<CredentialRefillTaskView, GatewayError> {
    ensure_refill_enabled(state)?;
    let task_id = normalize_identifier(task_id, "taskId", MAX_WORKER_ID_LENGTH)?;
    let claim_token = normalize_identifier(&input.claim_token, "claimToken", MAX_WORKER_ID_LENGTH)?;
    let reason = normalize_required_text(&input.reason, "reason", MAX_FAILURE_REASON_LENGTH)?;
    let mut task = require_claimed_task(state, &task_id, &claim_token).await?;
    task.state = CredentialRefillTaskState::Failed;
    task.updated_at = now_rfc3339();
    task.lease_until = None;
    task.claim_token = None;
    task.message = Some(reason);
    finish_task(state, &task, &claim_token).await?;
    Ok(CredentialRefillTaskView::from(&task))
}

fn build_demand_view(
    state: &AppState,
    provider: &ProviderConfigYaml,
    revision_id: &str,
    outstanding: Option<&CredentialRefillTaskRecord>,
) -> CredentialRefillDemandView {
    let target_size = provider
        .pool_target_size
        .unwrap_or(1)
        .clamp(1, MAX_REQUESTED_COUNT);
    let active_credential_count = active_credential_count(provider);
    let category_deficit = identity_category_deficit(provider);
    let deficit = target_size
        .saturating_sub(active_credential_count)
        .max(category_deficit);
    let direct_driver_configured = state
        .credential_pool_automation
        .has_driver_for_provider(provider);
    let queue_enabled = state.credential_pool_automation.refill_queue_enabled();
    CredentialRefillDemandView {
        provider_id: provider.id.clone(),
        provider_label: provider
            .label
            .clone()
            .unwrap_or_else(|| provider.id.clone()),
        target_size,
        credential_count: provider.credentials.len(),
        active_credential_count,
        deficit,
        needs_refill: deficit > 0,
        auto_refill_enabled: provider.auto_refill_enabled,
        direct_driver_configured,
        notification_enabled: queue_enabled
            && provider.auto_refill_enabled
            && !direct_driver_configured,
        inquiry_enabled: queue_enabled,
        user_request_enabled: queue_enabled,
        outstanding_task_id: outstanding.map(|task| task.id.clone()),
        outstanding_task_state: outstanding.map(|task| task.state),
        revision_id: revision_id.to_string(),
    }
}

async fn demand_for_provider(
    state: &AppState,
    provider_id: &str,
) -> Result<CredentialRefillDemandView, GatewayError> {
    let provider_id = normalize_identifier(provider_id, "providerId", MAX_WORKER_ID_LENGTH)?;
    let snapshot = state.route_config.snapshot();
    let provider = snapshot
        .document()
        .providers
        .iter()
        .find(|provider| provider.id == provider_id)
        .ok_or_else(|| {
            GatewayError::not_found(format!("Provider '{provider_id}' 不存在"))
                .with_code("credential_refill_provider_not_found")
        })?;
    let outstanding = load_outstanding_task(state, &provider_id).await?;
    Ok(build_demand_view(
        state,
        provider,
        snapshot.revision().id(),
        outstanding.as_ref(),
    ))
}

async fn create_task_from_demand(
    state: &AppState,
    demand: &CredentialRefillDemandView,
    trigger: CredentialRefillTrigger,
    requested_count: usize,
    idempotency_key: Option<String>,
) -> Result<CreateCredentialRefillTaskResult, GatewayError> {
    ensure_refill_enabled(state)?;
    validate_requested_count(requested_count)?;
    let now = now_rfc3339();
    let task_id = Uuid::new_v4().to_string();
    let task = CredentialRefillTaskRecord {
        id: task_id.clone(),
        provider_id: demand.provider_id.clone(),
        provider_label: demand.provider_label.clone(),
        trigger,
        state: CredentialRefillTaskState::Pending,
        requested_count,
        target_size: demand.target_size,
        active_credential_count: demand.active_credential_count,
        route_revision: demand.revision_id.clone(),
        created_at: now.clone(),
        updated_at: now.clone(),
        worker_id: None,
        claim_token: None,
        lease_until: None,
        attempt: 0,
        delivery_mode: None,
        created_count: 0,
        message: None,
        revision_id: None,
        idempotency_key: idempotency_key.clone(),
    };
    let payload = serialize_task(&task)?;
    let idempotency_hash = idempotency_key
        .as_deref()
        .map(|value| hash_idempotency_key(&demand.provider_id, value))
        .unwrap_or_else(|| format!("unused-{task_id}"));
    let ttl = state.credential_pool_automation.refill_task_ttl_seconds();
    let timestamp = OffsetDateTime::now_utc().unix_timestamp();
    let mut conn = redis_connection(state).await?;
    let (created, resolved_task_id): (i64, String) = Script::new(
        r#"
local existing = redis.call('GET', KEYS[1])
if existing then return {0, existing} end
if ARGV[1] == '1' then
  local idempotent = redis.call('GET', KEYS[2])
  if idempotent then return {0, idempotent} end
end
redis.call('SET', KEYS[1], ARGV[2], 'EX', ARGV[3])
if ARGV[1] == '1' then redis.call('SET', KEYS[2], ARGV[2], 'EX', ARGV[3]) end
redis.call('SET', KEYS[3], ARGV[4], 'EX', ARGV[3])
redis.call('ZADD', KEYS[4], ARGV[5], ARGV[2])
redis.call('ZADD', KEYS[5], ARGV[5], ARGV[2])
redis.call('XADD', KEYS[6], 'MAXLEN', '~', ARGV[6], '*',
  'eventType', 'gateway.credential_pool.refill_requested',
  'taskId', ARGV[2], 'providerId', ARGV[7], 'trigger', ARGV[8],
  'requestedCount', ARGV[9], 'routeRevision', ARGV[10], 'createdAt', ARGV[11])
return {1, ARGV[2]}
"#,
    )
    .key(keys::credential_refill_outstanding_key(&demand.provider_id))
    .key(keys::credential_refill_idempotency_key(&idempotency_hash))
    .key(keys::credential_refill_task_key(&task_id))
    .key(keys::credential_refill_pending_tasks_key())
    .key(keys::credential_refill_recent_tasks_key())
    .key(keys::credential_refill_stream_key())
    .arg(if idempotency_key.is_some() { "1" } else { "0" })
    .arg(&task_id)
    .arg(ttl)
    .arg(payload)
    .arg(timestamp)
    .arg(state.credential_pool_automation.refill_stream_max_len())
    .arg(&demand.provider_id)
    .arg(trigger_name(trigger))
    .arg(requested_count)
    .arg(&demand.revision_id)
    .arg(&now)
    .invoke_async(&mut conn)
    .await
    .map_err(|_| redis_operation_error())?;
    drop(conn);
    let resolved = if resolved_task_id == task_id {
        task
    } else {
        load_task(state, &resolved_task_id)
            .await?
            .ok_or_else(|| GatewayError::conflict("补号任务正在创建，请稍后重试"))?
    };
    Ok(CreateCredentialRefillTaskResult {
        task: CredentialRefillTaskView::from(&resolved),
        created: created == 1,
    })
}

async fn try_claim_task(
    state: &AppState,
    mut task: CredentialRefillTaskRecord,
    worker_id: &str,
    lease_seconds: u64,
) -> Result<Option<ClaimCredentialRefillTaskResult>, GatewayError> {
    let expected_payload = serialize_task(&task)?;
    let claim_token = Uuid::new_v4().to_string();
    task.state = CredentialRefillTaskState::Claimed;
    task.worker_id = Some(worker_id.to_string());
    task.claim_token = Some(claim_token.clone());
    task.lease_until = Some(future_rfc3339(lease_seconds));
    task.updated_at = now_rfc3339();
    task.attempt = task.attempt.saturating_add(1);
    let payload = serialize_task(&task)?;
    let mut conn = redis_connection(state).await?;
    let result: i64 = Script::new(
        r#"
local current = redis.call('GET', KEYS[2])
if current ~= ARGV[3] then return 0 end
local acquired = redis.call('SET', KEYS[1], ARGV[1], 'NX', 'EX', ARGV[2])
if not acquired then return 0 end
redis.call('SET', KEYS[2], ARGV[4], 'EX', ARGV[5])
if redis.call('GET', KEYS[3]) == ARGV[6] then
  redis.call('EXPIRE', KEYS[3], ARGV[5])
end
return 1
"#,
    )
    .key(keys::credential_refill_lease_key(&task.id))
    .key(keys::credential_refill_task_key(&task.id))
    .key(keys::credential_refill_outstanding_key(&task.provider_id))
    .arg(&claim_token)
    .arg(lease_seconds)
    .arg(expected_payload)
    .arg(payload)
    .arg(state.credential_pool_automation.refill_task_ttl_seconds())
    .arg(&task.id)
    .invoke_async(&mut conn)
    .await
    .map_err(|_| redis_operation_error())?;
    if result != 1 {
        return Ok(None);
    }
    Ok(Some(ClaimCredentialRefillTaskResult {
        task: CredentialRefillTaskView::from(&task),
        claim_token,
    }))
}

async fn deliver_refill_result(
    state: &Arc<AppState>,
    task: &CredentialRefillTaskRecord,
    delivery: CredentialRefillDeliveryInput,
) -> Result<DeliveryOutcome, GatewayError> {
    match delivery {
        CredentialRefillDeliveryInput::FolderSync { relative_paths } => {
            validate_folder_paths(&relative_paths)?;
            let status = run_folder_sync_once(state.as_ref(), FolderSyncDirection::Import).await?;
            let created_count = status.imported_count.saturating_add(status.updated_count);
            Ok(DeliveryOutcome {
                mode: CredentialRefillDeliveryMode::FolderSync,
                created_count,
                message: format!("文件夹同步已完成，导入或更新 {created_count} 个凭证。"),
                revision_id: None,
            })
        }
        CredentialRefillDeliveryInput::GatewayPull { artifact_reference } => {
            let artifact_reference = normalize_required_text(
                &artifact_reference,
                "artifactReference",
                MAX_ARTIFACT_REFERENCE_LENGTH,
            )?;
            let credentials =
                crate::credential_pool_automation::collect_refill_credentials_from_driver(
                    state,
                    &task.provider_id,
                    &task.id,
                    task.requested_count,
                    &artifact_reference,
                )
                .await
                .map_err(|error| {
                    GatewayError::bad_request(error.to_string())
                        .with_code("credential_refill_driver_pull_failed")
                })?;
            let (created_count, revision_id) = commit_refill_credentials(
                state,
                &task.provider_id,
                task.requested_count,
                credentials,
            )
            .await?;
            Ok(DeliveryOutcome {
                mode: CredentialRefillDeliveryMode::GatewayPull,
                created_count,
                message: format!("Gateway 已通过可信驱动器拉取 {created_count} 个凭证。"),
                revision_id: Some(revision_id),
            })
        }
        CredentialRefillDeliveryInput::DirectCallback { credentials } => {
            let (created_count, revision_id) = commit_refill_credentials(
                state,
                &task.provider_id,
                task.requested_count,
                credentials,
            )
            .await?;
            Ok(DeliveryOutcome {
                mode: CredentialRefillDeliveryMode::DirectCallback,
                created_count,
                message: format!("补号程序已直接回传 {created_count} 个凭证。"),
                revision_id: Some(revision_id),
            })
        }
    }
}

async fn commit_refill_credentials(
    state: &Arc<AppState>,
    provider_id: &str,
    requested_count: usize,
    credentials: Vec<ProviderCredentialYaml>,
) -> Result<(usize, String), GatewayError> {
    if credentials.is_empty() {
        return Err(GatewayError::bad_request("补号结果没有包含任何凭证")
            .with_code("credential_refill_empty_delivery"));
    }
    if credentials.len() > requested_count {
        return Err(GatewayError::bad_request("补号结果数量超过任务请求数量")
            .with_code("credential_refill_delivery_exceeds_request"));
    }
    let runtime = state.route_config_runtime.as_ref().ok_or_else(|| {
        GatewayError::service_unavailable("路由配置运行时不可用")
            .with_code("credential_refill_route_runtime_unavailable")
    })?;
    for _ in 0..3 {
        let snapshot = state.route_config.snapshot();
        let mut document = snapshot.document().clone();
        let provider = document
            .providers
            .iter_mut()
            .find(|provider| provider.id == provider_id)
            .ok_or_else(|| GatewayError::not_found("补号任务对应的 Provider 已不存在"))?;
        let created_count = append_refill_credentials(provider, credentials.clone())?;
        if created_count == 0 {
            return Ok((0, snapshot.revision().id().to_string()));
        }
        match runtime
            .commit_automation_document(
                snapshot.revision().id(),
                document,
                Some(format!(
                    "credential refill delivery for {provider_id}: +{created_count}"
                )),
            )
            .await
        {
            Ok(committed) => {
                return Ok((created_count, committed.revision().id().to_string()));
            }
            Err(error) if error.code() == "console_revision_conflict" => continue,
            Err(error) => {
                return Err(GatewayError::server_error(error.to_string())
                    .with_code("credential_refill_route_commit_failed"));
            }
        }
    }
    Err(GatewayError::conflict("路由配置持续变化，补号结果暂未提交")
        .with_code("credential_refill_route_revision_conflict"))
}

fn append_refill_credentials(
    provider: &mut ProviderConfigYaml,
    credentials: Vec<ProviderCredentialYaml>,
) -> Result<usize, GatewayError> {
    let mut existing_ids = provider
        .credentials
        .iter()
        .filter_map(|credential| credential.id.clone())
        .collect::<HashSet<_>>();
    let mut created_count = 0usize;
    for credential in credentials {
        let id = credential
            .id
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .ok_or_else(|| GatewayError::bad_request("补号结果中的凭证缺少 ID"))?;
        normalize_identifier(id, "credentialId", MAX_WORKER_ID_LENGTH)?;
        if existing_ids.insert(id.to_string()) {
            provider.credentials.push(credential);
            created_count += 1;
        }
    }
    Ok(created_count)
}

async fn finish_task(
    state: &AppState,
    task: &CredentialRefillTaskRecord,
    claim_token: &str,
) -> Result<(), GatewayError> {
    let payload = serialize_task(task)?;
    let mut conn = redis_connection(state).await?;
    let finished: i64 = Script::new(
        r#"
local token = redis.call('GET', KEYS[1])
if token ~= ARGV[1] then return 0 end
redis.call('SET', KEYS[2], ARGV[2], 'EX', ARGV[3])
redis.call('ZREM', KEYS[3], ARGV[4])
if redis.call('GET', KEYS[4]) == ARGV[4] then redis.call('DEL', KEYS[4]) end
redis.call('DEL', KEYS[1])
return 1
"#,
    )
    .key(keys::credential_refill_lease_key(&task.id))
    .key(keys::credential_refill_task_key(&task.id))
    .key(keys::credential_refill_pending_tasks_key())
    .key(keys::credential_refill_outstanding_key(&task.provider_id))
    .arg(claim_token)
    .arg(payload)
    .arg(state.credential_pool_automation.refill_task_ttl_seconds())
    .arg(&task.id)
    .invoke_async(&mut conn)
    .await
    .map_err(|_| redis_operation_error())?;
    if finished != 1 {
        return Err(claim_conflict());
    }
    Ok(())
}

async fn require_claimed_task(
    state: &AppState,
    task_id: &str,
    claim_token: &str,
) -> Result<CredentialRefillTaskRecord, GatewayError> {
    let task = load_task(state, task_id)
        .await?
        .ok_or_else(|| GatewayError::not_found("补号任务不存在"))?;
    if task.state != CredentialRefillTaskState::Claimed {
        return Err(claim_conflict());
    }
    let mut conn = redis_connection(state).await?;
    let stored_token: Option<String> = redis::cmd("GET")
        .arg(keys::credential_refill_lease_key(task_id))
        .query_async(&mut conn)
        .await
        .map_err(|_| redis_operation_error())?;
    if stored_token.as_deref() != Some(claim_token) {
        return Err(claim_conflict());
    }
    Ok(task)
}

async fn pending_task_ids(state: &AppState, limit: usize) -> Result<Vec<String>, GatewayError> {
    prune_expired_task_indexes(state).await?;
    let mut conn = redis_connection(state).await?;
    redis::cmd("ZRANGE")
        .arg(keys::credential_refill_pending_tasks_key())
        .arg(0)
        .arg(limit.saturating_sub(1))
        .query_async(&mut conn)
        .await
        .map_err(|_| redis_operation_error())
}

async fn prune_expired_task_indexes(state: &AppState) -> Result<(), GatewayError> {
    let ttl = i64::try_from(state.credential_pool_automation.refill_task_ttl_seconds())
        .unwrap_or(i64::MAX);
    let cutoff = OffsetDateTime::now_utc()
        .unix_timestamp()
        .saturating_sub(ttl);
    let mut conn = redis_connection(state).await?;
    redis::pipe()
        .atomic()
        .cmd("ZREMRANGEBYSCORE")
        .arg(keys::credential_refill_pending_tasks_key())
        .arg("-inf")
        .arg(cutoff)
        .ignore()
        .cmd("ZREMRANGEBYSCORE")
        .arg(keys::credential_refill_recent_tasks_key())
        .arg("-inf")
        .arg(cutoff)
        .ignore()
        .query_async::<()>(&mut conn)
        .await
        .map_err(|_| redis_operation_error())
}

async fn load_outstanding_task(
    state: &AppState,
    provider_id: &str,
) -> Result<Option<CredentialRefillTaskRecord>, GatewayError> {
    let mut conn = redis_connection(state).await?;
    let task_id: Option<String> = redis::cmd("GET")
        .arg(keys::credential_refill_outstanding_key(provider_id))
        .query_async(&mut conn)
        .await
        .map_err(|_| redis_operation_error())?;
    drop(conn);
    let Some(task_id) = task_id else {
        return Ok(None);
    };
    load_task(state, &task_id).await
}

async fn load_task(
    state: &AppState,
    task_id: &str,
) -> Result<Option<CredentialRefillTaskRecord>, GatewayError> {
    let mut conn = redis_connection(state).await?;
    let payload: Option<String> = redis::cmd("GET")
        .arg(keys::credential_refill_task_key(task_id))
        .query_async(&mut conn)
        .await
        .map_err(|_| redis_operation_error())?;
    payload
        .map(|payload| {
            serde_json::from_str(&payload).map_err(|_| {
                GatewayError::server_error("补号任务状态无法解析")
                    .with_code("credential_refill_task_corrupt")
            })
        })
        .transpose()
}

async fn redis_connection(state: &AppState) -> Result<deadpool_redis::Connection, GatewayError> {
    state
        .redis_pool
        .get()
        .await
        .map_err(|_| redis_operation_error())
}

fn serialize_task(task: &CredentialRefillTaskRecord) -> Result<String, GatewayError> {
    serde_json::to_string(task).map_err(|_| {
        GatewayError::server_error("补号任务状态无法序列化")
            .with_code("credential_refill_task_serialize_failed")
    })
}

fn ensure_refill_enabled(state: &AppState) -> Result<(), GatewayError> {
    if state.credential_pool_automation.refill_queue_enabled() {
        Ok(())
    } else {
        Err(GatewayError::service_unavailable("补号任务框架未启用")
            .with_code("credential_refill_disabled"))
    }
}

fn active_credential_count(provider: &ProviderConfigYaml) -> usize {
    if provider.credentials.is_empty() {
        usize::from(!provider.api_key.trim().is_empty() || provider.auth_token.is_some())
    } else {
        provider
            .credentials
            .iter()
            .filter(|credential| credential.enabled.unwrap_or(true))
            .count()
    }
}

fn identity_category_deficit(provider: &ProviderConfigYaml) -> usize {
    provider
        .credential_identity_categories
        .iter()
        .filter_map(serde_json::Value::as_object)
        .filter(|category| {
            category
                .get("auto_refill_enabled")
                .and_then(serde_json::Value::as_bool)
                .unwrap_or(false)
        })
        .filter_map(|category| {
            let category_id = category.get("id")?.as_str()?.trim();
            if category_id.is_empty() {
                return None;
            }
            let target = category
                .get("pool_target_size")
                .and_then(serde_json::Value::as_u64)
                .and_then(|value| usize::try_from(value).ok())
                .unwrap_or(1)
                .clamp(1, MAX_REQUESTED_COUNT);
            let active = provider
                .credentials
                .iter()
                .filter(|credential| credential.enabled.unwrap_or(true))
                .filter(|credential| {
                    credential.credential_identity_category_id.as_deref() == Some(category_id)
                })
                .count();
            Some(target.saturating_sub(active))
        })
        .fold(0usize, usize::saturating_add)
        .min(MAX_REQUESTED_COUNT)
}

fn validate_requested_count(requested_count: usize) -> Result<(), GatewayError> {
    if (1..=MAX_REQUESTED_COUNT).contains(&requested_count) {
        Ok(())
    } else {
        Err(GatewayError::bad_request(format!(
            "requestedCount 必须位于 1..={MAX_REQUESTED_COUNT}"
        )))
    }
}

fn validate_folder_paths(paths: &[String]) -> Result<(), GatewayError> {
    if paths.len() > MAX_FOLDER_PATHS {
        return Err(GatewayError::bad_request("relativePaths 数量过多"));
    }
    for path in paths {
        let normalized = path.trim();
        if normalized.is_empty()
            || normalized.chars().count() > MAX_FOLDER_PATH_LENGTH
            || std::path::Path::new(normalized).is_absolute()
            || std::path::Path::new(normalized)
                .components()
                .any(|part| matches!(part, std::path::Component::ParentDir))
        {
            return Err(GatewayError::bad_request(
                "relativePaths 必须是根目录内不包含 '..' 的相对路径",
            ));
        }
    }
    Ok(())
}

fn normalize_identifier(value: &str, field: &str, max: usize) -> Result<String, GatewayError> {
    let normalized = value.trim();
    if normalized.is_empty()
        || normalized.chars().count() > max
        || !normalized.chars().all(|character| {
            character.is_ascii_alphanumeric() || matches!(character, '-' | '_' | '.')
        })
    {
        return Err(GatewayError::bad_request(format!(
            "{field} 只能包含 ASCII 字母、数字、'-'、'_' 或 '.'"
        )));
    }
    Ok(normalized.to_string())
}

fn normalize_required_text(value: &str, field: &str, max: usize) -> Result<String, GatewayError> {
    let normalized = value.trim();
    if normalized.is_empty() || normalized.chars().count() > max {
        return Err(GatewayError::bad_request(format!(
            "{field} 不能为空且长度不能超过 {max}"
        )));
    }
    Ok(normalized.to_string())
}

fn normalize_optional_text(
    value: Option<String>,
    field: &str,
    max: usize,
) -> Result<Option<String>, GatewayError> {
    value
        .map(|value| normalize_required_text(&value, field, max))
        .transpose()
}

fn normalized_lease_seconds(state: &AppState, requested: Option<u64>) -> u64 {
    requested
        .unwrap_or_else(|| {
            state
                .credential_pool_automation
                .refill_default_lease_seconds()
        })
        .clamp(
            30,
            state.credential_pool_automation.refill_max_lease_seconds(),
        )
}

fn hash_idempotency_key(provider_id: &str, value: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(provider_id.as_bytes());
    hasher.update([0]);
    hasher.update(value.as_bytes());
    hex::encode(hasher.finalize())
}

fn trigger_name(trigger: CredentialRefillTrigger) -> &'static str {
    match trigger {
        CredentialRefillTrigger::Notification => "notification",
        CredentialRefillTrigger::Inquiry => "inquiry",
        CredentialRefillTrigger::UserRequested => "user_requested",
    }
}

fn now_rfc3339() -> String {
    OffsetDateTime::now_utc()
        .format(&Rfc3339)
        .unwrap_or_else(|_| "unknown".to_string())
}

fn future_rfc3339(seconds: u64) -> String {
    let seconds = i64::try_from(seconds).unwrap_or(i64::MAX);
    OffsetDateTime::now_utc()
        .saturating_add(time::Duration::seconds(seconds))
        .format(&Rfc3339)
        .unwrap_or_else(|_| "unknown".to_string())
}

fn redis_operation_error() -> GatewayError {
    GatewayError::server_error("补号任务 Redis 操作失败").with_code("credential_refill_redis_error")
}

fn claim_conflict() -> GatewayError {
    GatewayError::conflict("补号任务认领已失效或已被其他补号程序接管")
        .with_code("credential_refill_claim_conflict")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn provider(value: serde_json::Value) -> ProviderConfigYaml {
        serde_json::from_value(value).unwrap()
    }

    #[test]
    fn demand_combines_provider_and_identity_category_deficits() {
        let provider = provider(serde_json::json!({
            "id": "codex",
            "base_url": "https://example.invalid",
            "pool_target_size": 2,
            "credentials": [
                {"id": "plus-1", "credential_identity_category_id": "plus"},
                {"id": "free-1", "credential_identity_category_id": "free"}
            ],
            "credential_identity_categories": [
                {"id": "plus", "pool_target_size": 4, "auto_refill_enabled": true},
                {"id": "free", "pool_target_size": 9, "auto_refill_enabled": false}
            ]
        }));

        assert_eq!(active_credential_count(&provider), 2);
        assert_eq!(identity_category_deficit(&provider), 3);
    }

    #[test]
    fn task_view_never_serializes_the_claim_token_or_idempotency_key() {
        let record = CredentialRefillTaskRecord {
            id: "task-1".to_string(),
            provider_id: "provider-a".to_string(),
            provider_label: "Provider A".to_string(),
            trigger: CredentialRefillTrigger::Inquiry,
            state: CredentialRefillTaskState::Claimed,
            requested_count: 1,
            target_size: 2,
            active_credential_count: 1,
            route_revision: "r1".to_string(),
            created_at: "now".to_string(),
            updated_at: "now".to_string(),
            worker_id: Some("worker-a".to_string()),
            claim_token: Some("claim-secret".to_string()),
            lease_until: Some("later".to_string()),
            attempt: 1,
            delivery_mode: None,
            created_count: 0,
            message: None,
            revision_id: None,
            idempotency_key: Some("caller-secret".to_string()),
        };

        let serialized = serde_json::to_string(&CredentialRefillTaskView::from(&record)).unwrap();
        assert!(!serialized.contains("claim-secret"));
        assert!(!serialized.contains("caller-secret"));
        assert!(!serialized.contains("claimToken"));
        assert!(!serialized.contains("idempotencyKey"));
    }

    #[test]
    fn direct_delivery_is_idempotent_by_credential_id() {
        let mut provider = provider(serde_json::json!({
            "id": "provider-a",
            "base_url": "https://example.invalid",
            "credentials": [{"id": "account-a", "api_key": "existing"}]
        }));
        let incoming: Vec<ProviderCredentialYaml> = serde_json::from_value(serde_json::json!([
            {"id": "account-a", "api_key": "retry"},
            {"id": "account-b", "api_key": "new"}
        ]))
        .unwrap();

        assert_eq!(
            append_refill_credentials(&mut provider, incoming).unwrap(),
            1
        );
        assert_eq!(provider.credentials.len(), 2);
    }

    #[test]
    fn folder_delivery_rejects_parent_directory_escape() {
        let error = validate_folder_paths(&["../outside.json".to_string()]).unwrap_err();
        assert_eq!(error.http_status, Some(400));
    }

    #[test]
    fn caller_idempotency_keys_are_scoped_to_the_provider() {
        let provider_a = hash_idempotency_key("provider-a", "request-1");
        let provider_b = hash_idempotency_key("provider-b", "request-1");

        assert_ne!(provider_a, provider_b);
        assert_eq!(provider_a, hash_idempotency_key("provider-a", "request-1"));
    }

    #[test]
    fn delivery_inputs_follow_the_documented_camel_case_contract() {
        let folder: CredentialRefillDeliveryInput = serde_json::from_value(serde_json::json!({
            "mode": "folder_sync",
            "relativePaths": ["provider/account.json"]
        }))
        .unwrap();
        assert!(matches!(
            folder,
            CredentialRefillDeliveryInput::FolderSync { relative_paths }
                if relative_paths == ["provider/account.json"]
        ));

        let pull: CredentialRefillDeliveryInput = serde_json::from_value(serde_json::json!({
            "mode": "gateway_pull",
            "artifactReference": "artifact-1"
        }))
        .unwrap();
        assert!(matches!(
            pull,
            CredentialRefillDeliveryInput::GatewayPull { artifact_reference }
                if artifact_reference == "artifact-1"
        ));

        let direct: CredentialRefillDeliveryInput = serde_json::from_value(serde_json::json!({
            "mode": "direct_callback",
            "credentials": [{"id": "account-1", "api_key": "secret"}]
        }))
        .unwrap();
        assert!(matches!(
            direct,
            CredentialRefillDeliveryInput::DirectCallback { credentials }
                if credentials.len() == 1
        ));
    }
}
