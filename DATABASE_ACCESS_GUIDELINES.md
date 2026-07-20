# Database Access Guidelines for Rust Gateway

## Purpose

This document defines how the Rust gateway should access PostgreSQL directly, including connection pool configuration, transaction isolation levels, and error handling strategies.

## Connection Pool Configuration

### Basic Setup

```rust
// gateway/src/db/pool.rs
use sqlx::postgres::{PgPool, PgPoolOptions};
use std::time::Duration;

pub async fn create_pg_pool(database_url: &str) -> Result<PgPool, sqlx::Error> {
    PgPoolOptions::new()
        .max_connections(20)              // Maximum connections
        .min_connections(5)               // Minimum idle connections
        .acquire_timeout(Duration::from_secs(5))
        .idle_timeout(Duration::from_secs(600))    // 10 minutes
        .max_lifetime(Duration::from_secs(1800))   // 30 minutes
        .test_before_acquire(true)        // Validate connections
        .connect(database_url)
        .await
}
```

### Environment Configuration

```bash
# Production
DATABASE_URL=postgresql://neuroloom:password@postgres:5432/neuroloom
DATABASE_MAX_CONNECTIONS=20
DATABASE_MIN_CONNECTIONS=5
DATABASE_ACQUIRE_TIMEOUT_SECS=5

# Development
DATABASE_URL=postgresql://neuroloom:neuroloom@localhost:5432/neuroloom
DATABASE_MAX_CONNECTIONS=10
DATABASE_MIN_CONNECTIONS=2
```

## Transaction Isolation Levels

### Read Operations (Default: READ COMMITTED)

```rust
// Simple reads - no explicit transaction needed
let credential = sqlx::query_as::<_, UserCredential>(
    "SELECT * FROM gateway_user_credentials WHERE credential_key = $1"
)
.bind(credential_key)
.fetch_optional(&pool)
.await?;
```

### Write Operations (Use REPEATABLE READ for critical operations)

```rust
// Credential issuance - prevent race conditions
let mut tx = pool.begin().await?;

// Set isolation level
sqlx::query("SET TRANSACTION ISOLATION LEVEL REPEATABLE READ")
    .execute(&mut *tx)
    .await?;

// Check if credential already exists
let existing = sqlx::query_scalar::<_, i64>(
    "SELECT COUNT(*) FROM gateway_user_credentials WHERE credential_key = $1"
)
.bind(&credential_key)
.fetch_one(&mut *tx)
.await?;

if existing > 0 {
    tx.rollback().await?;
    return Err(anyhow!("Credential already exists"));
}

// Insert new credential
sqlx::query(
    "INSERT INTO gateway_user_credentials (id, credential_key, user_id, ...) VALUES ($1, $2, $3, ...)"
)
.bind(&id)
.bind(&credential_key)
.bind(&user_id)
.execute(&mut *tx)
.await?;

tx.commit().await?;
```

### Avoid SERIALIZABLE Unless Necessary

SERIALIZABLE has significant performance impact. Only use when:
- Complex multi-table operations with strict consistency requirements
- Financial transactions or quota deductions

For most gateway operations, REPEATABLE READ is sufficient.

## Deadlock Prevention

### Rule 1: Consistent Lock Order

Always acquire locks in the same order across all transactions:

```rust
// GOOD: Always lock user_credentials before provider_accounts
let user_cred = sqlx::query("SELECT * FROM gateway_user_credentials WHERE id = $1 FOR UPDATE")
    .bind(user_id)
    .fetch_one(&mut tx)
    .await?;

let provider = sqlx::query("SELECT * FROM gateway_provider_accounts WHERE id = $1 FOR UPDATE")
    .bind(provider_id)
    .fetch_one(&mut tx)
    .await?;

// BAD: Inconsistent order can cause deadlocks
```

### Rule 2: Keep Transactions Short

```rust
// GOOD: Short transaction
let mut tx = pool.begin().await?;
sqlx::query("UPDATE gateway_user_credentials SET status = $1 WHERE id = $2")
    .bind("revoked")
    .bind(id)
    .execute(&mut *tx)
    .await?;
tx.commit().await?;

// BAD: Long transaction with external calls
let mut tx = pool.begin().await?;
// ... database operations ...
let response = http_client.get("https://external-api.com").await?; // DON'T DO THIS
// ... more database operations ...
tx.commit().await?;
```

### Rule 3: Use SKIP LOCKED for Queue-Like Patterns

```rust
// Process pending requests without blocking
let request = sqlx::query_as::<_, PendingRequest>(
    "SELECT * FROM pending_requests 
     WHERE status = 'pending' 
     ORDER BY created_at 
     LIMIT 1 
     FOR UPDATE SKIP LOCKED"
)
.fetch_optional(&mut tx)
.await?;
```

## Error Handling Strategy

### Database Connection Errors

```rust
pub async fn with_retry<F, T>(operation: F) -> Result<T>
where
    F: Fn() -> BoxFuture<'static, Result<T>>,
{
    let mut attempts = 0;
    let max_attempts = 3;
    
    loop {
        match operation().await {
            Ok(result) => return Ok(result),
            Err(e) if is_retryable(&e) && attempts < max_attempts => {
                attempts += 1;
                let delay = Duration::from_millis(100 * 2_u64.pow(attempts));
                tokio::time::sleep(delay).await;
                tracing::warn!(
                    error = %e,
                    attempt = attempts,
                    "Database operation failed, retrying"
                );
            }
            Err(e) => return Err(e),
        }
    }
}

fn is_retryable(error: &sqlx::Error) -> bool {
    matches!(
        error,
        sqlx::Error::PoolTimedOut | sqlx::Error::Io(_)
    )
}
```

### Query Timeout Handling

```rust
use tokio::time::timeout;

pub async fn query_with_timeout<T>(
    query: sqlx::query::Query<'_, sqlx::Postgres, T>,
    pool: &PgPool,
) -> Result<T>
where
    T: for<'r> sqlx::FromRow<'r, sqlx::postgres::PgRow>,
{
    match timeout(Duration::from_secs(5), query.fetch_one(pool)).await {
        Ok(Ok(result)) => Ok(result),
        Ok(Err(e)) => Err(e.into()),
        Err(_) => Err(anyhow!("Query timeout after 5 seconds")),
    }
}
```

### Constraint Violation Handling

```rust
pub fn handle_db_error(error: sqlx::Error) -> GatewayError {
    match error {
        sqlx::Error::Database(db_err) => {
            if let Some(code) = db_err.code() {
                match code.as_ref() {
                    "23505" => {
                        // Unique violation
                        GatewayError::conflict("Resource already exists")
                    }
                    "23503" => {
                        // Foreign key violation
                        GatewayError::bad_request("Referenced resource not found")
                    }
                    "23514" => {
                        // Check constraint violation
                        GatewayError::bad_request("Invalid data")
                    }
                    _ => GatewayError::internal(format!("Database error: {}", db_err)),
                }
            } else {
                GatewayError::internal(format!("Database error: {}", db_err))
            }
        }
        sqlx::Error::RowNotFound => GatewayError::not_found("Resource not found"),
        sqlx::Error::PoolTimedOut => {
            GatewayError::service_unavailable("Database connection pool exhausted")
        }
        _ => GatewayError::internal(format!("Database error: {}", error)),
    }
}
```

## Redis Fallback Strategy

When Redis is unavailable, the gateway should degrade gracefully:

```rust
pub async fn get_user_credential(
    pool: &PgPool,
    redis_pool: &deadpool_redis::Pool,
    credential_key: &str,
) -> Result<Option<UserCredential>> {
    // Try Redis cache first
    match get_from_redis(redis_pool, credential_key).await {
        Ok(Some(credential)) => {
            tracing::debug!("Cache hit for credential");
            return Ok(Some(credential));
        }
        Ok(None) => {
            tracing::debug!("Cache miss for credential");
        }
        Err(e) => {
            tracing::warn!(error = %e, "Redis error, falling back to database");
        }
    }
    
    // Fallback to PostgreSQL
    let credential = sqlx::query_as::<_, UserCredential>(
        "SELECT * FROM gateway_user_credentials WHERE credential_key = $1"
    )
    .bind(credential_key)
    .fetch_optional(pool)
    .await?;
    
    // Try to populate cache (best effort)
    if let Some(ref cred) = credential {
        if let Err(e) = set_in_redis(redis_pool, credential_key, cred).await {
            tracing::warn!(error = %e, "Failed to populate Redis cache");
        }
    }
    
    Ok(credential)
}
```

## Monitoring and Observability

### Connection Pool Metrics

```rust
pub async fn report_pool_metrics(pool: &PgPool) {
    let size = pool.size();
    let idle = pool.num_idle();
    
    tracing::info!(
        pool_size = size,
        pool_idle = idle,
        pool_active = size - idle,
        "Database connection pool status"
    );
}
```

### Slow Query Logging

```rust
use std::time::Instant;

pub async fn query_with_logging<T>(
    query: sqlx::query::Query<'_, sqlx::Postgres, T>,
    pool: &PgPool,
    query_name: &str,
) -> Result<T>
where
    T: for<'r> sqlx::FromRow<'r, sqlx::postgres::PgRow>,
{
    let start = Instant::now();
    let result = query.fetch_one(pool).await;
    let duration = start.elapsed();
    
    if duration > Duration::from_millis(100) {
        tracing::warn!(
            query = query_name,
            duration_ms = duration.as_millis(),
            "Slow query detected"
        );
    }
    
    result.map_err(Into::into)
}
```

## Best Practices Summary

1. **Connection Pool**: Use 20 max connections in production, 10 in development
2. **Isolation Level**: READ COMMITTED for reads, REPEATABLE READ for critical writes
3. **Transactions**: Keep them short, avoid external calls inside transactions
4. **Deadlocks**: Consistent lock order, use SKIP LOCKED for queues
5. **Retries**: Retry connection errors with exponential backoff (max 3 attempts)
6. **Timeouts**: 5 seconds for queries, fail fast
7. **Redis Fallback**: Always degrade gracefully when Redis is unavailable
8. **Monitoring**: Log slow queries (>100ms), track pool metrics

## Migration from TypeScript

When migrating from TypeScript (Drizzle ORM) to Rust (sqlx):

1. **No ORM**: sqlx is a query builder, not an ORM. Write raw SQL.
2. **Compile-time Checking**: Use `sqlx::query_as!` macro for compile-time SQL validation
3. **Async All The Way**: No blocking database calls, always use `.await`
4. **Error Handling**: Use `?` operator, convert sqlx errors to GatewayError
5. **Migrations**: Keep using existing migration system, sqlx can run them

---

**Last Updated**: 2026-04-13
**Owner**: Rust Gateway Team
