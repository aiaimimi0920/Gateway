//! Read/validate the exact gate, then atomically settle only those observed bytes.
//! A lost response can be retried: tombstones have no expiry and take precedence
//! over a different transaction activating the same deterministic revision ID.
use super::{serialize_transaction_record, RouteConfigRedisSettlement as Settlement, *};

#[cfg(test)]
#[path = "transaction_fence_tests.rs"]
mod tests;

const SETTLE_LUA: &str = r#"
local gate = redis.call('GET', KEYS[2])
if not gate then gate = '' end
if gate ~= ARGV[1] then return 'changed' end
if ARGV[2] == 'revoked' then return 'revoked' end
local active = redis.call('GET', KEYS[1])
if not active then active = '' end
if ARGV[2] == 'activated' then
  if active == ARGV[4] then return 'activated' end
  return 'conflict'
end
if active ~= '' and active ~= ARGV[3] and active ~= ARGV[4] then
  return 'conflict'
end
redis.call('SET', KEYS[2], ARGV[5])
return 'revoked'
"#;

impl RouteConfigRedisStore {
    pub async fn settle_transaction(
        &self,
        pool: &Pool,
        record: &TransactionRecord,
    ) -> Result<Settlement, RouteConfigRedisStoreError> {
        let prepared = record
            .prepared_identity()
            .map_err(RouteConfigRedisStoreError::from_persistence)?;
        let tombstone = record
            .revocation_tombstone()
            .map_err(RouteConfigRedisStoreError::from_persistence)?;
        self.ensure_single_node(pool).await?;
        let mut connection = pool
            .get()
            .await
            .map_err(|_| fence_error("connection unavailable"))?;
        let key = self.keys.transaction_key(record.tx_id());
        let observed: Option<String> = connection
            .get(&key)
            .await
            .map_err(|_| fence_error("gate read failed"))?;
        let state = classify_gate(observed.as_deref(), &prepared, &tombstone)?;
        let result: String = redis::Script::new(SETTLE_LUA)
            .key(self.keys.active_revision_key())
            .key(key)
            .arg(observed.as_deref().unwrap_or(""))
            .arg(state)
            .arg(record.old_revision().unwrap_or(""))
            .arg(record.new_revision())
            .arg(serialize_transaction_record(&tombstone)?)
            .invoke_async(&mut connection)
            .await
            .map_err(|_| fence_error("settlement response unavailable"))?;
        match result.as_str() {
            "activated" => Ok(Settlement::Activated),
            "revoked" => Ok(Settlement::Revoked),
            // A concurrent activation changed the validated gate: retry recovery,
            // never infer that a single old-active read permits local rollback.
            "changed" | "conflict" => Err(fence_error("transaction or active revision changed")),
            _ => Err(fence_error("invalid settlement response")),
        }
    }
}

fn classify_gate(
    raw: Option<&str>,
    prepared: &TransactionRecord,
    tombstone: &TransactionRecord,
) -> Result<&'static str, RouteConfigRedisStoreError> {
    let Some(raw) = raw else {
        return Ok("pending");
    };
    let stored: TransactionRecord =
        serde_json::from_str(raw).map_err(|_| fence_error("invalid transaction JSON"))?;
    let identity = stored
        .prepared_identity()
        .map_err(|_| fence_error("invalid transaction record"))?;
    if identity != *prepared {
        return Err(fence_error("transaction identity mismatch"));
    }
    match stored.phase() {
        TransactionPhase::Prepared if stored == *prepared => Ok("pending"),
        TransactionPhase::RedisActivated => Ok("activated"),
        TransactionPhase::Aborted if stored == *tombstone => Ok("revoked"),
        _ => Err(fence_error("unexpected transaction phase")),
    }
}

fn fence_error(reason: &str) -> RouteConfigRedisStoreError {
    RouteConfigRedisStoreError::from_persistence(
        crate::console::PersistenceError::recovery_required(format!(
            "Gateway console transaction settlement requires recovery: {reason}"
        )),
    )
}
