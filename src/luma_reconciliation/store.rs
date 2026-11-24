//! Reuses the request-audit row in SQLite/PostgreSQL; no second ledger or schema migration.
use super::*;
use crate::local_runtime::LocalRuntime;
use sqlx::types::Json;

#[derive(Clone)]
pub(super) enum Store {
    Local(LocalRuntime),
    Server(sqlx::PgPool),
}
impl Store {
    pub(super) fn from_state(state: &AppState) -> Option<Self> {
        state
            .local_runtime
            .clone()
            .map(Self::Local)
            .or_else(|| state.pg_pool.clone().map(Self::Server))
    }

    pub(super) async fn checkpoint(
        &self,
        id: &str,
        facts: &Value,
        journal: &Value,
    ) -> Result<(), GatewayError> {
        let affected = match self {
            Self::Local(local) => sqlx::query("UPDATE request_audits SET payload = json_set(payload, '$.routeTrace',
                json_set(coalesce(json_extract(payload, '$.routeTrace'), '{}'),
                '$.lumaAgentsGeneration', json(?), '$.lumaReconciliation', json(?)))
                WHERE id = ? AND status = 'running'")
                .bind(facts.to_string()).bind(journal.to_string()).bind(id).execute(&local.pool).await
                .map_err(storage_error)?.rows_affected(),
            Self::Server(pool) => sqlx::query("UPDATE gateway_request_audits SET route_trace =
                coalesce(route_trace, '{}'::jsonb) || jsonb_build_object('lumaAgentsGeneration', $2::jsonb, 'lumaReconciliation', $3::jsonb)
                WHERE id = $1 AND status = 'running'")
                .bind(id).bind(Json(facts)).bind(Json(journal)).execute(pool).await
                .map_err(storage_error)?.rows_affected(),
        };
        if affected != 1 {
            return Err(GatewayError::conflict(
                "Luma request audit is no longer active.",
            ));
        }
        Ok(())
    }

    pub(super) async fn due(&self, time: i64) -> Result<Vec<(String, Value)>, GatewayError> {
        match self {
            Self::Local(local) => {
                let rows: Vec<(String,String)> = sqlx::query_as("SELECT id, json_extract(payload, '$.routeTrace') FROM request_audits
                    WHERE (status != 'running' OR json_extract(payload, '$.routeTrace.lumaReconciliation.activeUntil') <= ?)
                    AND json_extract(payload, '$.routeTrace.lumaReconciliation.state') = 'pending'
                    AND json_extract(payload, '$.routeTrace.lumaReconciliation.nextAt') <= ? ORDER BY created LIMIT 8")
                    .bind(time).bind(time).fetch_all(&local.pool).await.map_err(storage_error)?;
                rows.into_iter()
                    .map(|(id, raw)| {
                        serde_json::from_str(&raw)
                            .map(|value| (id, value))
                            .map_err(|_| GatewayError::server_error("Luma audit is corrupt."))
                    })
                    .collect()
            }
            Self::Server(pool) => {
                let rows: Vec<(String, Json<Value>)> = sqlx::query_as("SELECT id, route_trace FROM gateway_request_audits
                    WHERE (status != 'running' OR (route_trace->'lumaReconciliation'->>'activeUntil')::bigint <= $1)
                    AND route_trace->'lumaReconciliation'->>'state' = 'pending'
                    AND (route_trace->'lumaReconciliation'->>'nextAt')::bigint <= $1 ORDER BY created_at LIMIT 8")
                    .bind(time).fetch_all(pool).await.map_err(storage_error)?;
                Ok(rows
                    .into_iter()
                    .map(|(id, Json(trace))| (id, trace))
                    .collect())
            }
        }
    }

    /// CAS claims also fence stale workers after a crash or a second Gateway process.
    pub(super) async fn update(
        &self,
        id: &str,
        previous: &Value,
        trace: &Value,
    ) -> Result<bool, GatewayError> {
        let affected = match self {
            Self::Local(local) => sqlx::query("UPDATE request_audits SET payload = json_set(payload,
                '$.routeTrace.lumaAgentsGeneration', json(?), '$.routeTrace.lumaReconciliation', json(?))
                WHERE id = ? AND json_extract(payload, '$.routeTrace.lumaReconciliation') = json(?)
                AND json_extract(payload, '$.routeTrace.lumaAgentsGeneration') = json(?)")
                .bind(trace["lumaAgentsGeneration"].to_string()).bind(trace["lumaReconciliation"].to_string())
                .bind(id).bind(previous["lumaReconciliation"].to_string())
                .bind(previous["lumaAgentsGeneration"].to_string()).execute(&local.pool).await
                .map_err(storage_error)?.rows_affected(),
            Self::Server(pool) => sqlx::query("UPDATE gateway_request_audits SET route_trace = route_trace || $3::jsonb, updated_at = now()
                WHERE id = $1 AND route_trace->'lumaReconciliation' = $2
                AND route_trace->'lumaAgentsGeneration' = $4")
                .bind(id).bind(Json(&previous["lumaReconciliation"]))
                .bind(Json(json!({"lumaAgentsGeneration":trace["lumaAgentsGeneration"],"lumaReconciliation":trace["lumaReconciliation"]})))
                .bind(Json(&previous["lumaAgentsGeneration"])).execute(pool).await
                .map_err(storage_error)?.rows_affected(),
        };
        Ok(affected == 1)
    }
}
