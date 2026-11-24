//! The local writer lock and Redis compare-and-set serialize sessions across processes.
use super::error;
use crate::{
    console::{ConsoleConfig, RouteConfigPersistence},
    error::GatewayError,
};
use deadpool_redis::Pool;

pub(crate) struct SessionStore {
    local: Option<RouteConfigPersistence>,
    redis: Option<Pool>,
    prefix: String,
    pub(super) slots: std::sync::Arc<tokio::sync::Semaphore>,
    pub(super) pending:
        parking_lot::Mutex<std::collections::HashMap<String, super::pending::Pending>>,
}
impl SessionStore {
    pub(crate) fn new(config: &ConsoleConfig, redis: Option<Pool>) -> Result<Self, GatewayError> {
        let local = if redis.is_none() {
            Some(
                RouteConfigPersistence::new(&config.state_dir, &config.routes_file)
                    .map_err(|_| error("lumalabs_session_store_unavailable"))?,
            )
        } else {
            None
        };
        Ok(Self {
            slots: std::sync::Arc::new(tokio::sync::Semaphore::new(32)),
            pending: Default::default(),
            local,
            redis,
            prefix: format!("gateway:{}:luma-web-session:", config.redis_namespace),
        })
    }
    pub(super) async fn read(&self, key: &str) -> Result<Option<String>, GatewayError> {
        tokio::time::timeout(std::time::Duration::from_secs(5), self.read_inner(key))
            .await
            .map_err(|_| error("lumalabs_session_store_unavailable"))?
    }
    async fn read_inner(&self, key: &str) -> Result<Option<String>, GatewayError> {
        if let Some(local) = &self.local {
            let local = local.clone();
            let key = key.to_owned();
            return tokio::task::spawn_blocking(move || local.read_luma_web_session(&key))
                .await
                .map_err(|_| error("lumalabs_session_store_unavailable"))?
                .map_err(|_| error("lumalabs_session_store_unavailable"));
        }
        let mut connection = self
            .redis
            .as_ref()
            .expect("storage backend")
            .get()
            .await
            .map_err(|_| error("lumalabs_session_store_unavailable"))?;
        // Bound bytes before transfer; corrupt external storage is never treated as an empty slot.
        let raw: Option<String> = deadpool_redis::redis::Script::new(
            "local n=redis.call('STRLEN',KEYS[1]); if n>65536 then return redis.error_reply('oversized session') end; return redis.call('GET',KEYS[1])")
            .key(format!("{}{key}", self.prefix)).invoke_async(&mut connection).await
            .map_err(|_| error("lumalabs_session_store_unavailable"))?;
        Ok(raw)
    }
    pub(super) async fn cas(
        &self,
        key: &str,
        expected: Option<&str>,
        next: &str,
    ) -> Result<(), GatewayError> {
        tokio::time::timeout(
            std::time::Duration::from_secs(5),
            self.cas_inner(key, expected, next),
        )
        .await
        .map_err(|_| error("lumalabs_session_store_unavailable"))?
    }
    async fn cas_inner(
        &self,
        key: &str,
        expected: Option<&str>,
        next: &str,
    ) -> Result<(), GatewayError> {
        super::SessionState::decode(next)?;
        if let Some(local) = &self.local {
            let (local, key, expected, next) = (
                local.clone(),
                key.to_owned(),
                expected.map(str::to_owned),
                next.to_owned(),
            );
            let saved = tokio::task::spawn_blocking(move || {
                local.cas_luma_web_session(&key, expected.as_deref(), &next)
            })
            .await
            .map_err(|_| error("lumalabs_session_store_unavailable"))?
            .map_err(|_| error("lumalabs_session_store_unavailable"))?;
            return if saved {
                Ok(())
            } else {
                Err(error("lumalabs_session_store_conflict"))
            };
        }
        let mut connection = self
            .redis
            .as_ref()
            .expect("storage backend")
            .get()
            .await
            .map_err(|_| error("lumalabs_session_store_unavailable"))?;
        let result: i32 = deadpool_redis::redis::Script::new(
            "local old=redis.call('GET',KEYS[1]); if (ARGV[1]=='0' and old) or (ARGV[1]=='1' and old~=ARGV[2]) then if old==ARGV[3] then return 1 end; return 0 end; redis.call('SET',KEYS[1],ARGV[3]); return 1")
            .key(format!("{}{key}", self.prefix)).arg(if expected.is_some() { "1" } else { "0" })
            .arg(expected.unwrap_or_default()).arg(next).invoke_async(&mut connection).await
            .map_err(|_| error("lumalabs_session_store_unavailable"))?;
        if result == 1 {
            Ok(())
        } else {
            Err(error("lumalabs_session_store_conflict"))
        }
    }
}
