use crate::error::GatewayError;
use crate::redis::keys;
use crate::state::AppState;
use redis::Script;
use sha2::{Digest, Sha256};
use time::OffsetDateTime;
use uuid::Uuid;

use super::*;

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

pub(super) async fn create_task_from_demand(
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

pub(super) fn hash_idempotency_key(provider_id: &str, value: &str) -> String {
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
