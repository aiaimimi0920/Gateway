use deadpool_redis::Pool;
use redis::AsyncCommands;

use crate::error::GatewayError;
use crate::redis::keys;

use super::ProviderCredentialFolderSyncStatusView;

const MAX_UPDATE_ATTEMPTS: usize = 8;

// Compare raw bytes in Redis; keep JSON decoding and integer precision in Rust.
#[cfg(test)]
const COMMIT_STATUS: &str = r#"
local current = redis.call('GET', KEYS[1])
if ARGV[1] == '1' then
    if current ~= ARGV[2] then return 0 end
elseif current then
    return 0
end
redis.call('SET', KEYS[1], ARGV[3])
return 1
"#;

const COMMIT_ENABLED_STATUS: &str = r#"
local current_status = redis.call('GET', KEYS[1])
if ARGV[1] == '1' then
    if current_status ~= ARGV[2] then return 0 end
elseif current_status then
    return 0
end
local current_enabled = redis.call('GET', KEYS[2])
if ARGV[3] == '1' then
    if current_enabled ~= ARGV[4] then return 0 end
elseif current_enabled then
    return 0
end
redis.call('SET', KEYS[1], ARGV[5])
redis.call('SET', KEYS[2], ARGV[6])
redis.call('PUBLISH', ARGV[7], ARGV[6])
return 1
"#;

const COMMIT_STATUS_WITH_ENABLED: &str = r#"
local current_status = redis.call('GET', KEYS[1])
if ARGV[1] == '1' then
    if current_status ~= ARGV[2] then return 0 end
elseif current_status then
    return 0
end
local current_enabled = redis.call('GET', KEYS[2])
if ARGV[3] == '1' then
    if current_enabled ~= ARGV[4] then return 0 end
elseif current_enabled then
    return 0
end
redis.call('SET', KEYS[1], ARGV[5])
return 1
"#;

fn decode_status(
    raw: Option<&str>,
) -> Result<Option<ProviderCredentialFolderSyncStatusView>, GatewayError> {
    raw.map(|raw| {
        serde_json::from_str(raw).map_err(|error| {
            GatewayError::server_error(format!("decode folder sync status: {error}"))
        })
    })
    .transpose()
}

struct SnapshotValue {
    raw: Option<String>,
    primary_present: bool,
}

impl SnapshotValue {
    fn present(&self) -> bool {
        self.raw.is_some()
    }
}

struct FolderSyncSnapshot {
    status: SnapshotValue,
    enabled: SnapshotValue,
}

async fn read_folder_sync_snapshot(
    conn: &mut deadpool_redis::Connection,
) -> Result<FolderSyncSnapshot, GatewayError> {
    let values: (Option<String>, Option<String>) = redis::cmd("MGET")
        .arg(keys::provider_credential_folder_sync_status_key())
        .arg(keys::provider_credential_folder_sync_enabled_key())
        .query_async(conn)
        .await
        .map_err(|error| {
            GatewayError::server_error(format!("read folder sync status snapshot: {error}"))
        })?;
    let status_primary_present = values.0.is_some();
    let enabled_primary_present = values.1.is_some();
    let status = match values.0 {
        Some(raw) => Some(raw),
        None => conn
            .get(keys::legacy_provider_credential_folder_sync_status_key())
            .await
            .map_err(|error| {
                GatewayError::server_error(format!("read legacy folder sync status: {error}"))
            })?,
    };
    let enabled = match values.1 {
        Some(raw) => Some(raw),
        None => conn
            .get(keys::legacy_provider_credential_folder_sync_enabled_key())
            .await
            .map_err(|error| {
                GatewayError::server_error(format!("read legacy folder sync enabled: {error}"))
            })?,
    };
    Ok(FolderSyncSnapshot {
        status: SnapshotValue {
            raw: status,
            primary_present: status_primary_present,
        },
        enabled: SnapshotValue {
            raw: enabled,
            primary_present: enabled_primary_present,
        },
    })
}

pub(super) async fn read_folder_sync_status_and_enabled_override(
    redis_pool: &Pool,
) -> Result<(Option<ProviderCredentialFolderSyncStatusView>, Option<bool>), GatewayError> {
    let mut conn = redis_pool
        .get()
        .await
        .map_err(|error| GatewayError::server_error(format!("get redis connection: {error}")))?;
    let snapshot = read_folder_sync_snapshot(&mut conn).await?;
    let status = decode_status(snapshot.status.raw.as_deref())?;
    let enabled = snapshot
        .enabled
        .raw
        .as_deref()
        .map(|raw| {
            serde_json::from_str(raw).map_err(|error| {
                GatewayError::server_error(format!("decode folder sync enabled: {error}"))
            })
        })
        .transpose()?;
    Ok((status, enabled))
}

pub(super) async fn read_folder_sync_enabled_override(
    redis_pool: &Pool,
) -> Result<Option<bool>, GatewayError> {
    let mut conn = redis_pool
        .get()
        .await
        .map_err(|error| GatewayError::server_error(format!("get redis connection: {error}")))?;
    let primary_raw: Option<String> = conn
        .get(keys::provider_credential_folder_sync_enabled_key())
        .await
        .map_err(|error| {
            GatewayError::server_error(format!("read folder sync enabled: {error}"))
        })?;
    let raw = match primary_raw {
        Some(raw) => Some(raw),
        None => conn
            .get(keys::legacy_provider_credential_folder_sync_enabled_key())
            .await
            .map_err(|error| {
                GatewayError::server_error(format!("read legacy folder sync enabled: {error}"))
            })?,
    };
    raw.map(|raw| {
        serde_json::from_str(&raw).map_err(|error| {
            GatewayError::server_error(format!("decode folder sync enabled: {error}"))
        })
    })
    .transpose()
}

#[cfg(test)]
pub(super) async fn update_folder_sync_status(
    redis_pool: &Pool,
    mut update: impl FnMut(&mut ProviderCredentialFolderSyncStatusView),
) -> Result<ProviderCredentialFolderSyncStatusView, GatewayError> {
    update_folder_sync_status_with_presence(redis_pool, |status, _| update(status)).await
}

pub(super) async fn update_folder_sync_enabled(
    redis_pool: &Pool,
    enabled: bool,
    mut update: impl FnMut(&mut ProviderCredentialFolderSyncStatusView),
) -> Result<ProviderCredentialFolderSyncStatusView, GatewayError> {
    let mut conn = redis_pool
        .get()
        .await
        .map_err(|error| GatewayError::server_error(format!("get redis connection: {error}")))?;
    let status_key = keys::provider_credential_folder_sync_status_key();
    let enabled_key = keys::provider_credential_folder_sync_enabled_key();
    let enabled_raw = serde_json::to_string(&enabled).map_err(|error| {
        GatewayError::server_error(format!("encode folder sync enabled: {error}"))
    })?;
    let script = redis::Script::new(COMMIT_ENABLED_STATUS);
    for _ in 0..MAX_UPDATE_ATTEMPTS {
        let snapshot = read_folder_sync_snapshot(&mut conn).await?;
        let mut status = decode_status(snapshot.status.raw.as_deref())?.unwrap_or_default();
        update(&mut status);
        let next = serde_json::to_string(&status).map_err(|error| {
            GatewayError::server_error(format!("encode folder sync status: {error}"))
        })?;
        let committed = script
            .key(&status_key)
            .key(&enabled_key)
            .arg(u8::from(snapshot.status.primary_present))
            .arg(if snapshot.status.primary_present {
                snapshot.status.raw.as_deref().unwrap_or_default()
            } else {
                ""
            })
            .arg(u8::from(snapshot.enabled.primary_present))
            .arg(if snapshot.enabled.primary_present {
                snapshot.enabled.raw.as_deref().unwrap_or_default()
            } else {
                ""
            })
            .arg(next)
            .arg(&enabled_raw)
            .arg(keys::provider_credential_folder_sync_events_channel())
            .invoke_async::<i64>(&mut conn)
            .await
            .map_err(|error| {
                GatewayError::server_error(format!("write folder sync enabled status: {error}"))
            })?;
        if committed == 1 {
            return Ok(status);
        }
    }
    Err(GatewayError::conflict(
        "Folder sync enabled status changed concurrently; retry the status update",
    ))
}

pub(super) async fn update_folder_sync_status_with_shared_enabled(
    redis_pool: &Pool,
    mut update: impl FnMut(&mut ProviderCredentialFolderSyncStatusView, bool, Option<bool>),
) -> Result<ProviderCredentialFolderSyncStatusView, GatewayError> {
    let mut conn = redis_pool
        .get()
        .await
        .map_err(|error| GatewayError::server_error(format!("get redis connection: {error}")))?;
    let status_key = keys::provider_credential_folder_sync_status_key();
    let enabled_key = keys::provider_credential_folder_sync_enabled_key();
    let script = redis::Script::new(COMMIT_STATUS_WITH_ENABLED);
    for _ in 0..MAX_UPDATE_ATTEMPTS {
        let snapshot = read_folder_sync_snapshot(&mut conn).await?;
        let mut status = decode_status(snapshot.status.raw.as_deref())?.unwrap_or_default();
        let enabled = snapshot
            .enabled
            .raw
            .as_deref()
            .map(|raw| {
                serde_json::from_str(raw).map_err(|error| {
                    GatewayError::server_error(format!("decode folder sync enabled: {error}"))
                })
            })
            .transpose()?;
        update(&mut status, snapshot.status.present(), enabled);
        let next = serde_json::to_string(&status).map_err(|error| {
            GatewayError::server_error(format!("encode folder sync status: {error}"))
        })?;
        let committed = script
            .key(&status_key)
            .key(&enabled_key)
            .arg(u8::from(snapshot.status.primary_present))
            .arg(if snapshot.status.primary_present {
                snapshot.status.raw.as_deref().unwrap_or_default()
            } else {
                ""
            })
            .arg(u8::from(snapshot.enabled.primary_present))
            .arg(if snapshot.enabled.primary_present {
                snapshot.enabled.raw.as_deref().unwrap_or_default()
            } else {
                ""
            })
            .arg(next)
            .invoke_async::<i64>(&mut conn)
            .await
            .map_err(|error| {
                GatewayError::server_error(format!("write folder sync status: {error}"))
            })?;
        if committed == 1 {
            return Ok(status);
        }
    }
    Err(GatewayError::conflict(
        "Folder sync status changed concurrently; retry the status update",
    ))
}

#[cfg(test)]
pub(super) async fn update_folder_sync_status_with_presence(
    redis_pool: &Pool,
    mut update: impl FnMut(&mut ProviderCredentialFolderSyncStatusView, bool),
) -> Result<ProviderCredentialFolderSyncStatusView, GatewayError> {
    let mut conn = redis_pool
        .get()
        .await
        .map_err(|error| GatewayError::server_error(format!("get redis connection: {error}")))?;
    let key = keys::provider_credential_folder_sync_status_key();
    let script = redis::Script::new(COMMIT_STATUS);
    for _ in 0..MAX_UPDATE_ATTEMPTS {
        let primary_raw: Option<String> = conn.get(&key).await.map_err(|error| {
            GatewayError::server_error(format!("read folder sync status: {error}"))
        })?;
        let primary_present = primary_raw.is_some();
        let raw = match primary_raw {
            Some(raw) => Some(raw),
            None => conn
                .get(keys::legacy_provider_credential_folder_sync_status_key())
                .await
                .map_err(|error| {
                    GatewayError::server_error(format!("read legacy folder sync status: {error}"))
                })?,
        };
        let present = raw.is_some();
        let mut status = decode_status(raw.as_deref())?.unwrap_or_default();
        // Reapply only metadata to each fresh snapshot, never database operations.
        update(&mut status, present);
        let next = serde_json::to_string(&status).map_err(|error| {
            GatewayError::server_error(format!("encode folder sync status: {error}"))
        })?;
        let committed = script
            .key(&key)
            .arg(u8::from(primary_present))
            .arg(if primary_present {
                raw.as_deref().unwrap_or_default()
            } else {
                ""
            })
            .arg(next)
            .invoke_async::<i64>(&mut conn)
            .await
            .map_err(|error| {
                GatewayError::server_error(format!("write folder sync status: {error}"))
            })?;
        if committed == 1 {
            return Ok(status);
        }
    }
    Err(GatewayError::conflict(
        "Folder sync status changed concurrently; retry the status update",
    ))
}

#[cfg(test)]
mod tests;
