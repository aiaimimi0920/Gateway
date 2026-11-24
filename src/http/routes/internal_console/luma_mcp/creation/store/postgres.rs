//! PostgreSQL owns Server time, leases and task fencing across Gateway processes.
use super::{changed, invalid, GatewayError, Task};
use hmac::{Hmac, Mac};
use serde_json::{json, Value};
use sha2::Sha256;
use sqlx::PgPool;

type OwnerMac = Hmac<Sha256>;

pub(crate) struct Journal {
    pool: PgPool,
    owner: [u8; 32],
}

impl Journal {
    pub(super) fn new(
        pool: PgPool,
        secret: Option<&str>,
        fingerprint: &str,
    ) -> Result<Self, GatewayError> {
        let secret = secret.filter(|s| !s.is_empty()).ok_or_else(|| {
            GatewayError::service_unavailable(
                "Luma MCP Server tasks require GATEWAY_API_KEY_SECRET.",
            )
            .with_code("luma_mcp_journal_signing_unavailable")
        })?;
        let mut mac =
            OwnerMac::new_from_slice(secret.as_bytes()).expect("HMAC accepts any key size");
        mac.update(b"gateway:luma-mcp-management-owner:v1\0");
        mac.update(fingerprint.as_bytes());
        Ok(Self {
            pool,
            owner: mac.finalize().into_bytes().into(),
        })
    }

    pub(super) async fn insert(&self, task: &Task) -> Result<(), GatewayError> {
        sqlx::query("INSERT INTO gateway_luma_mcp_creation_tasks (id, owner_tag, payload) VALUES ($1, $2, $3)")
            .bind(&task.id).bind(self.owner.as_slice()).bind(payload(task)?)
            .execute(&self.pool).await.map_err(storage)?;
        Ok(())
    }

    pub(super) async fn read(&self, id: &str) -> Result<Task, GatewayError> {
        let (mut value, version, lease) = sqlx::query_as::<_, (Value, i64, i64)>(
            "SELECT payload, version, lease_until FROM gateway_luma_mcp_creation_tasks WHERE id = $1 AND owner_tag = $2",
        )
        .bind(id).bind(self.owner.as_slice())
        .fetch_optional(&self.pool).await.map_err(storage)?.ok_or_else(invalid)?;
        value["version"] = json!(version);
        value["leaseUntil"] = json!(lease);
        serde_json::from_value(value).map_err(|_| invalid())
    }

    pub(super) async fn expired(&self, task: &Task) -> Result<bool, GatewayError> {
        // An expired or replaced lease must not send work, even after a delayed DB response.
        let active: bool = sqlx::query_scalar(
            "SELECT EXISTS (SELECT 1 FROM gateway_luma_mcp_creation_tasks
            WHERE id = $1 AND owner_tag = $2 AND version = $3
            AND lease_until > floor(extract(epoch FROM clock_timestamp()))::bigint)",
        )
        .bind(&task.id)
        .bind(self.owner.as_slice())
        .bind(version(task)?)
        .fetch_one(&self.pool)
        .await
        .map_err(storage)?;
        Ok(!active)
    }

    pub(super) async fn acquire(&self, task: &mut Task) -> Result<(), GatewayError> {
        let (version, lease) = sqlx::query_as::<_, (i64, i64)>(
            "UPDATE gateway_luma_mcp_creation_tasks
            SET payload = $4, version = version + 1, updated_at = clock_timestamp(),
                lease_until = floor(extract(epoch FROM clock_timestamp()))::bigint + 90
            WHERE id = $1 AND owner_tag = $2 AND version = $3
              AND lease_until <= floor(extract(epoch FROM clock_timestamp()))::bigint
            RETURNING version, lease_until",
        )
        .bind(&task.id)
        .bind(self.owner.as_slice())
        .bind(version(task)?)
        .bind(payload(task)?)
        .fetch_optional(&self.pool)
        .await
        .map_err(storage)?
        .ok_or_else(changed)?;
        task.version = version as u64;
        task.lease_until = lease;
        Ok(())
    }

    pub(super) async fn save(&self, task: &mut Task, recovering: bool) -> Result<(), GatewayError> {
        let version = sqlx::query_scalar::<_, i64>("UPDATE gateway_luma_mcp_creation_tasks
            SET payload = $4, version = version + 1, lease_until = $5, updated_at = clock_timestamp()
            WHERE id = $1 AND owner_tag = $2 AND version = $3
              AND (lease_until <= floor(extract(epoch FROM clock_timestamp()))::bigint) = $6
            RETURNING version")
            .bind(&task.id).bind(self.owner.as_slice()).bind(version(task)?).bind(payload(task)?)
            .bind(task.lease_until).bind(recovering)
            .fetch_optional(&self.pool).await.map_err(storage)?.ok_or_else(changed)?;
        task.version = version as u64;
        Ok(())
    }
}

fn version(task: &Task) -> Result<i64, GatewayError> {
    i64::try_from(task.version).map_err(|_| invalid())
}

fn payload(task: &Task) -> Result<Value, GatewayError> {
    let mut value = serde_json::to_value(task).map_err(|_| invalid())?;
    let object = value.as_object_mut().ok_or_else(invalid)?;
    // These two fields have a single authority; never compare values supplied by the client.
    object.remove("version");
    object.remove("leaseUntil");
    Ok(value)
}

fn storage(error: sqlx::Error) -> GatewayError {
    let code = error.as_database_error().and_then(|error| error.code());
    if code.as_deref() == Some("42P01") {
        return GatewayError::service_unavailable(
            "Luma MCP Server task journal schema is unavailable.",
        )
        .with_code("luma_mcp_journal_schema_unavailable");
    }
    // A PG error detail can contain task payloads, including signed media links.
    tracing::error!(sqlstate = ?code, kind = ?std::mem::discriminant(&error), "Luma MCP Server journal storage failed");
    GatewayError::server_error("Luma MCP task storage failed.")
}
