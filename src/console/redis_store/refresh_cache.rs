//! Cache repair is a derivative write fenced by the exact active revision, grant and winning tx.
use super::refresh_grants::{invalid_state, unavailable};
use super::*;
use crate::console::refresh_grants::{GrantPhase, GrantRecord, MAX_GRANT_BYTES};

pub(crate) const REPAIR: &str = r#"
if redis.call('GET', KEYS[1]) ~= ARGV[1] then return -1 end
if redis.call('GET', KEYS[2]) ~= ARGV[2] then return -1 end
if redis.call('GET', KEYS[3]) ~= ARGV[3] then return -1 end
if redis.call('GET', KEYS[4]) ~= ARGV[4] then return -1 end
if redis.call('GET', KEYS[5]) ~= ARGV[5] then return 0 end
redis.call('SET', KEYS[5], ARGV[6], 'EX', ARGV[7])
return 1
"#;

impl RouteConfigRedisStore {
    pub async fn repair_refresh_grant_cache(
        &self,
        pool: &Pool,
        revision: &RouteConfigRedisRevision,
        expected: &GrantRecord,
        transaction: &TransactionRecord,
    ) -> Result<(), RouteConfigRedisStoreError> {
        self.ensure_single_node(pool).await?;
        if expected.phase() != GrantPhase::Published {
            return Err(invalid_state());
        }
        let (grant_raw, record) = self
            .read_grant(pool, &expected.family_id(), &expected.material_id())
            .await?
            .ok_or_else(invalid_state)?;
        if record != *expected {
            return Err(invalid_state());
        }
        let revision_raw = self.grant_revision_raw(pool, revision).await?;
        let tx_key = self.keys().transaction_key(transaction.tx_id());
        let tx_raw = self
            .read_grant_value(pool, &tx_key, MAX_GRANT_BYTES)
            .await?
            .ok_or_else(invalid_state)?;
        let activated: TransactionRecord =
            serde_json::from_str(&tx_raw).map_err(|_| invalid_state())?;
        if activated.phase() != TransactionPhase::RedisActivated
            || activated.prepared_identity().map_err(|_| invalid_state())?
                != transaction
                    .prepared_identity()
                    .map_err(|_| invalid_state())?
            || activated.new_revision() != revision.metadata().id()
        {
            return Err(invalid_state());
        }
        let cache_key = crate::redis::keys::credential_key(record.credential_id());
        let store = revision.clone().into_store(ActiveConfigSource::Recovered)?;
        let snapshot = store.snapshot();
        let (_, credential) = snapshot
            .oauth_refresh_target(record.provider_id(), record.credential_id())
            .ok_or_else(invalid_state)?;
        for _ in 0..3 {
            let Some(raw) = self
                .read_grant_value(pool, &cache_key, MAX_GRANT_BYTES)
                .await?
            else {
                return Ok(()); // Never recreate a retired or expired cache entry.
            };
            let Some((updated, ttl)) = crate::token_refresh::server_cache::patch_from_grant(
                &raw,
                &record,
                &credential.payload,
            )
            .map_err(|_| invalid_state())?
            else {
                return Ok(()); // Existing cache is already updated or independently reauthorized.
            };
            let mut connection = pool.get().await.map_err(unavailable)?;
            let applied: i32 = redis::Script::new(REPAIR)
                .key(self.keys().active_revision_key())
                .key(self.keys().revision_key(revision.metadata().id()))
                .key(self.refresh_grant_key(&record.family_id(), Some(&record.material_id()))?)
                .key(&tx_key)
                .key(&cache_key)
                .arg(revision.metadata().id())
                .arg(&revision_raw)
                .arg(&grant_raw)
                .arg(&tx_raw)
                .arg(raw)
                .arg(updated)
                .arg(ttl)
                .invoke_async(&mut connection)
                .await
                .map_err(unavailable)?;
            match applied {
                1 => return Ok(()),
                0 => continue,
                _ => return Err(invalid_state()),
            }
        }
        Err(unavailable("cache CAS contention"))
    }
}
