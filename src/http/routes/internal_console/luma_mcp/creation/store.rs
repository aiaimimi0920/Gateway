//! Local audit and Server management journals share the task model and account checks.
mod postgres;
use super::*;
use crate::{
    console::RouteConfigRuntime, db::CreateRequestAuditInput, local_runtime::LocalRuntime,
};
use sha2::{Digest, Sha256};
use sqlx::PgPool;

pub(super) enum Store {
    Local(LocalRuntime),
    Server(postgres::Journal),
}

impl Store {
    pub(super) fn server(
        pool: PgPool,
        secret: Option<&str>,
        fingerprint: &str,
    ) -> Result<Self, GatewayError> {
        Ok(Self::Server(postgres::Journal::new(
            pool,
            secret,
            fingerprint,
        )?))
    }

    pub(super) async fn expired(&self, task: &Task) -> Result<bool, GatewayError> {
        if task.lease_until == 0 {
            return Ok(true);
        }
        match self {
            Self::Local(_) => Ok(task.lease_until <= now()),
            Self::Server(pg) => pg.expired(task).await,
        }
    }

    pub(super) async fn acquire(&self, task: &mut Task) -> Result<(), GatewayError> {
        match self {
            Self::Local(local) => {
                task.lease_until = now() + 90;
                save_local(local, task).await
            }
            Self::Server(pg) => pg.acquire(task).await,
        }
    }

    pub(super) async fn save(&self, task: &mut Task, recovering: bool) -> Result<(), GatewayError> {
        match self {
            Self::Local(local) => save_local(local, task).await,
            Self::Server(pg) => pg.save(task, recovering).await,
        }
    }
}

#[derive(Clone, Debug, serde::Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct Task {
    pub id: String,
    pub provider_id: String,
    pub credential_id: String,
    pub revision: String,
    pub request_hash: String,
    pub kind: flow::Kind,
    #[serde(default)]
    pub requested_model: Option<String>,
    #[serde(default)]
    pub operation: Option<String>,
    pub budget_credits: u32,
    pub state: String,
    pub session_id: Option<String>,
    pub workspace_id: Option<String>,
    #[serde(default)]
    pub workspace_binding: Option<super::workspace::Binding>,
    pub run_id: Option<String>,
    pub assets: Vec<Value>,
    pub usage: Option<Value>,
    pub error: Option<String>,
    pub lease_until: i64,
    pub version: u64,
    pub ended: bool,
    #[serde(default)]
    pub review: Option<flow::Review>,
    #[serde(default)]
    pub responses: Vec<super::review::Receipt>,
    #[serde(default)]
    pub after_seq: Option<u64>,
    #[serde(default)]
    pub session_update: Option<Value>,
}
pub(super) fn hash(input: &flow::Request) -> Result<String, GatewayError> {
    let bytes = serde_json::to_vec(input).map_err(|_| invalid())?;
    Ok(format!("{:x}", Sha256::digest(bytes)))
}
pub(super) async fn prepare(
    store: &Store,
    target: &super::super::Input,
    request: &flow::Request,
    runtime: Option<&RouteConfigRuntime>,
) -> Result<Task, GatewayError> {
    let workspace_binding = super::workspace::bind(store, target, request, runtime).await?;
    let task = Task {
        id: format!("luma-mcp-{}", uuid::Uuid::new_v4()),
        provider_id: target.provider_id.clone(),
        credential_id: target.credential_id.clone(),
        revision: target.expected_revision.clone(),
        request_hash: hash(request)?,
        kind: request.kind,
        requested_model: Some(request.model().into()),
        operation: Some(request.operation().map_err(|_| invalid())?.into()),
        budget_credits: request.budget_credits,
        state: "prepared".into(),
        session_id: None,
        workspace_id: None,
        workspace_binding,
        run_id: None,
        assets: vec![],
        usage: None,
        error: None,
        lease_until: 0,
        version: 0,
        ended: false,
        review: None,
        responses: vec![],
        after_seq: None,
        session_update: None,
    };
    match store {
        Store::Local(local) => insert_local(local, &task).await?,
        Store::Server(pg) => pg.insert(&task).await?,
    }
    Ok(task)
}

async fn insert_local(local: &LocalRuntime, task: &Task) -> Result<(), GatewayError> {
    let trace = json!({"realCredentialRef":task.credential_id,"lumaMcpCreation":task});
    local
        .create_audit(CreateRequestAuditInput {
            project_id: "console-luma-mcp".into(),
            api_key_id: None,
            user_credential_id: None,
            access_key_id: None,
            source_access_key_id: None,
            session_id: None,
            route_policy_id: None,
            provider_account_id: Some(task.provider_id.clone()),
            protocol_family: "lumalabs_mcp".into(),
            endpoint_kind: "media_creation".into(),
            requested_model: task.requested_model.clone(),
            resolved_model: None,
            model_alias: None,
            stream: false,
            route_attempt_count: 1,
            response_id: task.id.clone(),
            previous_response_id: None,
            route_trace: Some(trace),
        })
        .await?;
    // Prepared is not an active network request. Keep this journal outside normal crash finalization.
    sqlx::query("UPDATE request_audits SET status = 'pending', payload = json_set(payload, '$.status', 'pending') WHERE id = ?")
        .bind(&task.id).execute(&local.pool).await.map_err(storage)?;
    Ok(())
}
pub(super) async fn load(
    store: &Store,
    id: &str,
    target: &super::super::Input,
    runtime: Option<&RouteConfigRuntime>,
) -> Result<Task, GatewayError> {
    if !id.starts_with("luma-mcp-") || uuid::Uuid::parse_str(&id[9..]).is_err() {
        return Err(invalid());
    }
    let task = match store {
        Store::Local(local) => read_local(local, id).await?,
        Store::Server(pg) => pg.read(id).await?,
    };
    if task.id != id
        || task.provider_id != target.provider_id
        || task.credential_id != target.credential_id
    {
        return Err(invalid());
    }
    super::identity::verify(runtime, &task.revision, target).await?;
    Ok(task)
}
async fn read_local(local: &LocalRuntime, id: &str) -> Result<Task, GatewayError> {
    let raw: Option<String> = sqlx::query_scalar("SELECT json_extract(payload, '$.routeTrace.lumaMcpCreation') FROM request_audits WHERE id = ?")
        .bind(id).fetch_optional(&local.pool).await.map_err(storage)?;
    serde_json::from_str(&raw.ok_or_else(invalid)?).map_err(|_| invalid())
}

async fn save_local(local: &LocalRuntime, task: &mut Task) -> Result<(), GatewayError> {
    let previous = task.version;
    // Supplier completion alone does not prove a consumable result or the actual model identity.
    let status = match task.state.as_str() {
        "failed" | "cancelled" => task.state.as_str(),
        "completed" if task.error.is_some() => "failed",
        "completed"
            if !task.assets.is_empty()
                && task
                    .assets
                    .iter()
                    .all(|asset| asset["state"] == "ready" && asset["url"].is_string()) =>
        {
            "completed"
        }
        _ => "pending",
    };
    let mut value = serde_json::to_value(&*task).map_err(|_| invalid())?;
    value["version"] = json!(previous + 1);
    let raw = value.to_string();
    let affected = sqlx::query("UPDATE request_audits SET status = ?, payload = json_set(payload, '$.status', ?, '$.routeTrace.lumaMcpCreation', json(?))
        WHERE id = ? AND json_extract(payload, '$.routeTrace.lumaMcpCreation.version') = ?")
        .bind(status).bind(status).bind(raw).bind(&task.id).bind(previous as i64)
        .execute(&local.pool).await.map_err(storage)?.rows_affected();
    if affected != 1 {
        return Err(changed());
    }
    task.version = previous + 1;
    Ok(())
}
fn changed() -> GatewayError {
    GatewayError::conflict("Luma task changed or its lease expired; read it again.")
}
fn now() -> i64 {
    time::OffsetDateTime::now_utc().unix_timestamp()
}
fn storage(error: sqlx::Error) -> GatewayError {
    tracing::error!(error = %error, "Luma MCP journal storage failed");
    GatewayError::server_error("Luma MCP task storage failed.")
}

#[cfg(test)]
mod tests {
    use super::*;
    #[tokio::test]
    async fn luma_mcp_stale_writer_and_missing_assets_cannot_be_successful() {
        let root = std::env::temp_dir().join(format!(
            "gateway-local-luma-mcp-cas-{}",
            uuid::Uuid::new_v4()
        ));
        let local = LocalRuntime::open(&root).await.unwrap();
        let target = super::super::Input {
            provider_id: "provider".into(),
            credential_id: "account".into(),
            expected_revision: "r1".into(),
        };
        let request = flow::Request {
            kind: flow::Kind::Image,
            prompt: "circle".into(),
            budget_credits: 20,
            options: None,
            reuse: None,
        };
        let store = Store::Local(local.clone());
        let mut stale = prepare(&store, &target, &request, None).await.unwrap();
        let mut winner = stale.clone();
        winner.state = "completed".into();
        store.save(&mut winner, false).await.unwrap();
        assert!(store.save(&mut stale, false).await.is_err());
        assert_eq!(stale.version, 0);
        assert!(store.save(&mut stale, false).await.is_err());
        assert_eq!(
            load(&store, &winner.id, &target, None).await.unwrap().state,
            "completed"
        );
        let (status, payload): (String, String) =
            sqlx::query_as("SELECT status, payload FROM request_audits WHERE id = ?")
                .bind(&winner.id)
                .fetch_one(&local.pool)
                .await
                .unwrap();
        assert_eq!(status, "pending");
        let payload: Value = serde_json::from_str(&payload).unwrap();
        assert!(payload["resolvedModel"].is_null());
        local.close().await;
        crate::local_runtime::test_support::remove_test_root(&root).await;
    }
}
