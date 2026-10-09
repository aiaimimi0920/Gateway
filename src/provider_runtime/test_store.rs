//! Latest assessment and atomic schedule slots, isolated from credential health and secrets.
use super::test_results::TestRunResult;
use crate::{error::GatewayError, state::AppState};
use sha2::{Digest, Sha256};

pub(crate) const LOCAL_SCHEMA: &str = "CREATE TABLE IF NOT EXISTS credential_test_results (
    id TEXT PRIMARY KEY, payload TEXT NOT NULL, updated INTEGER NOT NULL);
    CREATE TABLE IF NOT EXISTS credential_test_slots (
    id TEXT PRIMARY KEY, next INTEGER NOT NULL);";
const RETENTION: i64 = 30 * 24 * 3600;

pub(crate) fn identity(namespace: &str, provider: &str, credential: &str) -> String {
    hex::encode(Sha256::digest(
        serde_json::to_vec(&[namespace, provider, credential]).expect("identity"),
    ))
}

pub(crate) fn plan_identity(
    namespace: &str,
    provider: &str,
    credential: &str,
    plan: Option<&str>,
) -> String {
    if plan.is_none() {
        return identity(namespace, provider, credential);
    }
    let mut parts = vec![namespace, provider, credential];
    if let Some(plan) = plan {
        parts.push(plan);
    }
    hex::encode(Sha256::digest(
        serde_json::to_vec(&parts).expect("identity"),
    ))
}

pub(crate) async fn save(state: &AppState, result: &TestRunResult) -> Result<(), GatewayError> {
    let id = plan_identity(
        &state.config.console.redis_namespace,
        &result.provider_id,
        &result.credential_id,
        result
            .assessment
            .as_ref()
            .and_then(|assessment| assessment.plan_id.as_deref()),
    );
    let payload = serde_json::to_string(result)
        .map_err(|_| GatewayError::server_error("Cannot encode test results"))?;
    if let Some(local) = &state.local_runtime {
        let now = time::OffsetDateTime::now_utc().unix_timestamp();
        sqlx::query("INSERT INTO credential_test_results(id,payload,updated) VALUES (?,?,?) ON CONFLICT(id) DO UPDATE SET payload=excluded.payload, updated=excluded.updated")
            .bind(id).bind(payload).bind(now).execute(&local.pool).await.map_err(crate::local_runtime::storage_error)?;
        // Only this new owner's expired assessments/slots are retained for 30 days.
        sqlx::query("DELETE FROM credential_test_results WHERE updated < ?")
            .bind(now - RETENTION)
            .execute(&local.pool)
            .await
            .map_err(crate::local_runtime::storage_error)?;
    } else {
        let mut connection = state
            .redis_pool
            .get()
            .await
            .map_err(|_| GatewayError::server_error("Test result store unavailable"))?;
        redis::cmd("SET")
            .arg(format!("gw:credential-test-result:{id}"))
            .arg(payload)
            .arg("EX")
            .arg(RETENTION)
            .query_async::<()>(&mut connection)
            .await
            .map_err(|_| GatewayError::server_error("Cannot persist test result"))?;
    }
    Ok(())
}

pub(crate) async fn read_many(
    state: &AppState,
    provider: &str,
    credentials: &[&str],
    plan: Option<&str>,
) -> Result<Vec<TestRunResult>, GatewayError> {
    if credentials.is_empty() {
        return Ok(vec![]);
    }
    if credentials.len() > 128 {
        return Err(GatewayError::bad_request(
            "At most 128 test result identities per read.",
        ));
    }
    let ids: Vec<_> = credentials
        .iter()
        .map(|credential| {
            plan_identity(
                &state.config.console.redis_namespace,
                provider,
                credential,
                plan,
            )
        })
        .collect();
    // One bounded query per plan rather than up to 4096 serial round trips for a collection.
    let payloads: Vec<Option<String>> = if let Some(local) = &state.local_runtime {
        let mut query = sqlx::QueryBuilder::<sqlx::Sqlite>::new(
            "SELECT id,payload FROM credential_test_results WHERE updated>=",
        );
        query
            .push_bind(time::OffsetDateTime::now_utc().unix_timestamp() - RETENTION)
            .push(" AND id IN (");
        let mut list = query.separated(",");
        for id in &ids {
            list.push_bind(id);
        }
        list.push_unseparated(")");
        let mut rows: std::collections::HashMap<String, String> = query
            .build_query_as::<(String, String)>()
            .fetch_all(&local.pool)
            .await
            .map_err(crate::local_runtime::storage_error)?
            .into_iter()
            .collect();
        ids.iter().map(|id| rows.remove(id)).collect()
    } else {
        let mut connection = state
            .redis_pool
            .get()
            .await
            .map_err(|_| GatewayError::server_error("Test result store unavailable"))?;
        redis::cmd("MGET")
            .arg(
                ids.iter()
                    .map(|id| format!("gw:credential-test-result:{id}"))
                    .collect::<Vec<_>>(),
            )
            .query_async(&mut connection)
            .await
            .map_err(|_| GatewayError::server_error("Cannot read test results"))?
    };
    payloads
        .into_iter()
        .zip(credentials)
        .filter_map(|(payload, credential)| {
            payload.map(|payload| {
                let result: TestRunResult = serde_json::from_str(&payload)
                    .map_err(|_| GatewayError::server_error("Stored test result is invalid"))?;
                if result.provider_id != provider
                    || result.credential_id != *credential
                    || result
                        .assessment
                        .as_ref()
                        .and_then(|assessment| assessment.plan_id.as_deref())
                        != plan
                {
                    return Err(GatewayError::server_error(
                        "Stored test result identity is invalid",
                    ));
                }
                Ok(result)
            })
        })
        .collect()
}

pub(crate) async fn claim(
    state: &AppState,
    provider: &str,
    credential: &str,
    interval: u64,
    plan: Option<&str>,
) -> Result<bool, GatewayError> {
    let id = plan_identity(
        &state.config.console.redis_namespace,
        provider,
        credential,
        plan,
    );
    let now = time::OffsetDateTime::now_utc().unix_timestamp();
    let ttl = interval.clamp(1, 10_080) as i64 * 60;
    if let Some(local) = &state.local_runtime {
        let result = sqlx::query("INSERT INTO credential_test_slots(id,next) VALUES (?,?) ON CONFLICT(id) DO UPDATE SET next=excluded.next WHERE credential_test_slots.next<=?")
            .bind(id).bind(now + ttl).bind(now).execute(&local.pool).await.map_err(crate::local_runtime::storage_error)?;
        sqlx::query("DELETE FROM credential_test_slots WHERE next < ?")
            .bind(now - RETENTION)
            .execute(&local.pool)
            .await
            .map_err(crate::local_runtime::storage_error)?;
        Ok(result.rows_affected() == 1)
    } else {
        let mut connection = state
            .redis_pool
            .get()
            .await
            .map_err(|_| GatewayError::server_error("Test schedule store unavailable"))?;
        let value: Option<String> = redis::cmd("SET")
            .arg(format!("gw:credential-test-slot:{id}"))
            .arg("1")
            .arg("NX")
            .arg("EX")
            .arg(ttl)
            .query_async(&mut connection)
            .await
            .map_err(|_| GatewayError::server_error("Cannot claim test schedule"))?;
        Ok(value.is_some())
    }
}
