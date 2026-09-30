use crate::error::GatewayError;
use crate::redis::keys;
use crate::state::AppState;
use redis::Script;
use std::collections::HashSet;
use uuid::Uuid;

use super::*;

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
    if let Some(db) = &state.local_runtime {
        local::renew(
            db,
            &task,
            &claim_token,
            lease_seconds,
            state.credential_pool_automation.refill_task_ttl_seconds(),
        )
        .await?;
        return Ok(CredentialRefillTaskView::from(&task));
    }
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
    if let Some(db) = &state.local_runtime {
        let acquired = local::claim(
            db,
            &expected_payload,
            &task,
            lease_seconds,
            state.credential_pool_automation.refill_task_ttl_seconds(),
        )
        .await?;
        return Ok(acquired.then(|| ClaimCredentialRefillTaskResult {
            task: CredentialRefillTaskView::from(&task),
            claim_token,
        }));
    }
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

pub(super) async fn require_claimed_task(
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
    if let Some(db) = &state.local_runtime {
        local::require(db, task_id, claim_token).await?;
        return Ok(task);
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

pub(super) fn claim_conflict() -> GatewayError {
    GatewayError::conflict("补号任务认领已失效或已被其他补号程序接管")
        .with_code("credential_refill_claim_conflict")
}
