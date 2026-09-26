use crate::error::GatewayError;
use crate::redis::keys;
use crate::state::AppState;
use redis::Script;
use std::sync::Arc;

use super::*;

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
