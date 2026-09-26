use super::storage::{deserialize_json, redis_connection_error, serialize_json};
use super::{
    BrowserCapabilityLeaseView, BrowserCapabilitySlotStatus, BrowserCapabilitySlotView,
    DEFAULT_SLOT_TTL_SECONDS, RELEASED_LEASE_TTL_SECONDS,
};
use crate::error::GatewayError;
use crate::redis::keys;
use crate::state::AppState;

const MAX_RELEASE_ATTEMPTS: usize = 4;

// Validate the raw slot version before any writes, and compare/delete the lock
// in the same script. Rust owns JSON decoding so heartbeat metadata stays typed.
const RELEASE_SCRIPT: &str = r#"
local owned = redis.call('GET', KEYS[1]) == ARGV[1]
if owned then
    if ARGV[2] ~= '1' then return 0 end
    local slot = redis.call('GET', KEYS[2])
    if ARGV[3] == '1' then
        if slot ~= ARGV[4] then return 0 end
    elseif slot then
        return 0
    end
end
if owned and ARGV[3] == '1' then
    redis.call('SET', KEYS[2], ARGV[5], 'EX', ARGV[7])
end
redis.call('SET', KEYS[3], ARGV[6], 'EX', ARGV[8])
if owned then redis.call('DEL', KEYS[1]) end
return 1
"#;

pub(super) async fn commit_lease_release(
    state: &AppState,
    lease: &BrowserCapabilityLeaseView,
    released_at: &str,
    release_status: BrowserCapabilitySlotStatus,
) -> Result<(), GatewayError> {
    let lock_key = keys::browser_executor_slot_lease_lock_key(&lease.slot_id);
    let slot_key = keys::browser_executor_slot_key(&lease.slot_id);
    let lease_key = keys::browser_executor_lease_key(&lease.lease_id);
    let lease_payload = serialize_json(lease, "browser capability lease view")?;
    let script = redis::Script::new(RELEASE_SCRIPT);
    let mut conn = state
        .redis_pool
        .get()
        .await
        .map_err(redis_connection_error)?;
    for _ in 0..MAX_RELEASE_ATTEMPTS {
        let (lock, raw_slot): (Option<String>, Option<String>) = redis::pipe()
            .cmd("GET")
            .arg(&lock_key)
            .cmd("GET")
            .arg(&slot_key)
            .query_async(&mut conn)
            .await
            .map_err(|error| {
                GatewayError::server_error(format!("read browser lease release snapshot: {error}"))
            })?;
        let owns_lock = lock.as_deref() == Some(lease.lease_id.as_str());
        let slot_payload = if owns_lock {
            raw_slot
                .as_deref()
                .map(|raw| release_slot_payload(raw, &slot_key, released_at, release_status))
                .transpose()?
        } else {
            None
        };
        let committed = script
            .key(&lock_key)
            .key(&slot_key)
            .key(&lease_key)
            .arg(&lease.lease_id)
            .arg(u8::from(owns_lock))
            .arg(u8::from(raw_slot.is_some()))
            .arg(raw_slot.as_deref().unwrap_or_default())
            .arg(slot_payload.as_deref().unwrap_or_default())
            .arg(&lease_payload)
            .arg(DEFAULT_SLOT_TTL_SECONDS)
            .arg(RELEASED_LEASE_TTL_SECONDS)
            .invoke_async::<i64>(&mut conn)
            .await
            .map_err(|error| {
                GatewayError::server_error(format!(
                    "commit browser capability lease release: {error}"
                ))
            })?;
        if committed == 1 {
            return Ok(());
        }
    }
    Err(GatewayError::conflict(
        "Browser capability slot changed during lease release; retry the release",
    ))
}

fn release_slot_payload(
    raw: &str,
    slot_key: &str,
    released_at: &str,
    status: BrowserCapabilitySlotStatus,
) -> Result<String, GatewayError> {
    let mut slot: BrowserCapabilitySlotView = deserialize_json(raw, slot_key)?;
    slot.status = status;
    slot.updated_at = released_at.to_owned();
    if status == BrowserCapabilitySlotStatus::Cooling {
        slot.last_failure_at = Some(released_at.to_owned());
    }
    serialize_json(&slot, "browser capability slot view")
}

#[cfg(test)]
mod tests;
