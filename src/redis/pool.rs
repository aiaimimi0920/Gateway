use deadpool_redis::{Config as RedisConfig, Pool, Runtime};

pub type RedisPool = Pool;

pub fn create_pool(redis_url: &str) -> Result<RedisPool, anyhow::Error> {
    let cfg = RedisConfig::from_url(redis_url);
    let pool = cfg.create_pool(Some(Runtime::Tokio1))?;
    Ok(pool)
}
