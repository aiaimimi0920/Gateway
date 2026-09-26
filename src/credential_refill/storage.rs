use crate::error::GatewayError;
use crate::redis::keys;
use crate::state::AppState;
use time::OffsetDateTime;

use super::*;

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

pub(super) async fn pending_task_ids(
    state: &AppState,
    limit: usize,
) -> Result<Vec<String>, GatewayError> {
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

pub(super) async fn load_outstanding_task(
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

pub(super) async fn load_task(
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

pub(super) async fn redis_connection(
    state: &AppState,
) -> Result<deadpool_redis::Connection, GatewayError> {
    state
        .redis_pool
        .get()
        .await
        .map_err(|_| redis_operation_error())
}

pub(super) fn serialize_task(task: &CredentialRefillTaskRecord) -> Result<String, GatewayError> {
    serde_json::to_string(task).map_err(|_| {
        GatewayError::server_error("补号任务状态无法序列化")
            .with_code("credential_refill_task_serialize_failed")
    })
}

pub(super) fn redis_operation_error() -> GatewayError {
    GatewayError::server_error("补号任务 Redis 操作失败").with_code("credential_refill_redis_error")
}
