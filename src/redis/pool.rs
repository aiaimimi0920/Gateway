use deadpool_redis::{Config as RedisConfig, Pool, Runtime};

pub type RedisPool = Pool;

/// Keeps server-only call sites fail-closed without a local network connection.
pub fn disabled_pool() -> Result<RedisPool, anyhow::Error> {
    let pool = create_pool("redis://127.0.0.1")?;
    pool.close();
    Ok(pool)
}

pub fn create_pool(redis_url: &str) -> Result<RedisPool, anyhow::Error> {
    let cfg = RedisConfig::from_url(redis_url);
    let pool = cfg.create_pool(Some(Runtime::Tokio1))?;
    Ok(pool)
}
