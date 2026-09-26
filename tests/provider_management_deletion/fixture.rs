use std::path::PathBuf;
use std::str::FromStr;
use std::sync::Arc;
use std::time::Duration;

use axum::body::{to_bytes, Body};
use axum::http::Request;
use neuro_gateway::console::{ConsoleConfig, ConsoleConfigValues};
use neuro_gateway::redis::keys;
use neuro_gateway::routing::config::RouteConfigStore;
use neuro_gateway::state::AppState;
use redis::AsyncCommands;
use serde_json::Value;
use sqlx::postgres::{PgConnectOptions, PgPoolOptions};
use sqlx::PgPool;
use tower::ServiceExt;

const TOKEN: &str = "provider-deletion-contract-token";

pub struct Fixture {
    pub state: Arc<AppState>,
    pub pg: PgPool,
    pub account: String,
    pub root: PathBuf,
    base: PathBuf,
    schema: String,
    admin: PgPool,
    runtime_keys: Vec<String>,
}

impl Fixture {
    pub async fn new() -> Self {
        let run = std::env::var("GATEWAY_DELETE_TEST_RUN_ID").expect("guarded fixture run id");
        assert!(run.len() == 32 && run.bytes().all(|b| b.is_ascii_hexdigit()));
        let database = std::env::var("GATEWAY_DELETE_TEST_DATABASE_URL").unwrap();
        let redis_url = std::env::var("GATEWAY_DELETE_TEST_REDIS_URL").unwrap();
        let url = url::Url::parse(&database).unwrap();
        assert_eq!(url.host_str(), Some("127.0.0.1"));
        assert_eq!(url.path(), "/gateway_delete_fixture");
        assert_eq!(
            url::Url::parse(&redis_url).unwrap().host_str(),
            Some("127.0.0.1")
        );
        let admin = PgPoolOptions::new()
            .max_connections(2)
            .connect(&database)
            .await
            .unwrap();
        let guard: String = sqlx::query_scalar("select run_id from public.deletion_fixture_guard")
            .fetch_one(&admin)
            .await
            .unwrap();
        assert_eq!(guard, run);
        let id = uuid::Uuid::new_v4().simple().to_string();
        let schema = format!("delete_{id}");
        sqlx::query(&format!("create schema {schema}"))
            .execute(&admin)
            .await
            .unwrap();
        let options = PgConnectOptions::from_str(&database)
            .unwrap()
            .options([("search_path", schema.as_str())]);
        let pg = PgPoolOptions::new()
            .max_connections(4)
            .connect_with(options)
            .await
            .unwrap();
        super::schema::initialize(&pg).await;
        let base = std::env::temp_dir().join(format!("gateway-management-delete-{run}-{id}"));
        let root = base.join("root");
        std::fs::create_dir_all(&root).unwrap();
        let console = ConsoleConfig::from_values(ConsoleConfigValues {
            state_dir: Some(base.join("console")),
            routes_file: Some(base.join("console/routes.yaml")),
            ..Default::default()
        })
        .unwrap();
        let mut config = super::config::test_config(console);
        config.gateway_management_token = Some(TOKEN.into());
        config.redis_url = redis_url;
        config.provider_credential_folder_sync_enabled = true;
        config.provider_credential_folder_sync_root_dir = Some(root.to_str().unwrap().into());
        let mut state = super::support::build_test_app_state(config, RouteConfigStore::new(), None);
        Arc::get_mut(&mut state).unwrap().pg_pool = Some(pg.clone());
        let mut connection = state.redis_pool.get().await.unwrap();
        let guard: String = connection
            .get("gw:management-delete:contract_run")
            .await
            .unwrap();
        assert_eq!(guard, run);
        drop(connection);
        let account = format!("account-{id}");
        sqlx::query(
            "insert into gateway_provider_accounts (id,label) values ($1,'Fixture account')",
        )
        .bind(&account)
        .execute(&pg)
        .await
        .unwrap();
        let mut fixture = Self {
            state,
            pg,
            account,
            root,
            base,
            schema,
            admin,
            runtime_keys: Vec::new(),
        };
        fixture.seed_keys(fixture.account_keys()).await;
        fixture
    }

    pub fn account_keys(&self) -> Vec<String> {
        vec![
            keys::provider_payload_key(&self.account),
            keys::provider_failure_count_key(&self.account),
            keys::provider_breaker_open_key(&self.account),
            keys::provider_quota_snapshot_key(&self.account),
            keys::provider_quota_lock_key(&self.account),
        ]
    }

    pub fn credential_keys(id: &str) -> Vec<String> {
        vec![
            keys::provider_credential_failure_count_key(id),
            keys::provider_credential_breaker_open_key(id),
            keys::provider_credential_quota_snapshot_key(id),
            keys::provider_credential_quota_lock_key(id),
        ]
    }

    async fn seed_keys(&mut self, keys: Vec<String>) {
        let mut connection = self.state.redis_pool.get().await.unwrap();
        for key in &keys {
            let _: () = connection.set(key, "fixture").await.unwrap();
        }
        self.runtime_keys.extend(keys);
    }

    pub async fn credential(&mut self, path: &str, mode: &str) -> String {
        let id = format!("credential-{}", uuid::Uuid::new_v4().simple());
        sqlx::query("insert into gateway_provider_credentials (id,provider_account_id,label,source_path,sync_mode) values ($1,$2,'Fixture credential',$3,$4)")
            .bind(&id).bind(&self.account).bind(path).bind(mode).execute(&self.pg).await.unwrap();
        self.seed_keys(Self::credential_keys(&id)).await;
        id
    }

    pub async fn key_count(&self, keys: &[String]) -> usize {
        let mut connection = self.state.redis_pool.get().await.unwrap();
        connection.exists(keys).await.unwrap()
    }

    pub async fn credential_exists(&self, id: &str) -> bool {
        sqlx::query_scalar("select exists(select 1 from gateway_provider_credentials where id=$1)")
            .bind(id)
            .fetch_one(&self.pg)
            .await
            .unwrap()
    }

    pub async fn account_exists(&self) -> bool {
        sqlx::query_scalar("select exists(select 1 from gateway_provider_accounts where id=$1)")
            .bind(&self.account)
            .fetch_one(&self.pg)
            .await
            .unwrap()
    }

    pub async fn wait_idle(&self) {
        // Admitted workers can outlive requests; never delete their directory early.
        for _ in 0..300 {
            if Arc::strong_count(&self.state) == 1 {
                break;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
        assert_eq!(
            Arc::strong_count(&self.state),
            1,
            "deletion worker must drain"
        );
    }

    pub async fn finish(self) {
        self.wait_idle().await;
        let mut connection = self.state.redis_pool.get().await.unwrap();
        let _: usize = connection.del(&self.runtime_keys).await.unwrap();
        drop(connection);
        self.pg.close().await;
        assert!(
            self.schema.starts_with("delete_")
                && self.schema[7..].bytes().all(|b| b.is_ascii_hexdigit())
        );
        sqlx::query(&format!("drop schema {} cascade", self.schema))
            .execute(&self.admin)
            .await
            .unwrap();
        self.admin.close().await;
        let base = self.base.clone();
        drop(self);
        assert!(!base.exists());
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        if Arc::strong_count(&self.state) == 1
            && self.base.parent() == Some(std::env::temp_dir().as_path())
            && self
                .base
                .file_name()
                .unwrap()
                .to_string_lossy()
                .starts_with("gateway-management-delete-")
        {
            let _ = std::fs::remove_dir_all(&self.base);
        }
    }
}

pub async fn request(
    state: Arc<AppState>,
    account: bool,
    id: &str,
    authorized: bool,
) -> (u16, Value) {
    let segment = if account {
        "provider-accounts"
    } else {
        "provider-credentials"
    };
    let mut request = Request::builder()
        .method("DELETE")
        .uri(format!("/v1/internal/gateway/{segment}/{id}"));
    if authorized {
        request = request.header("x-management-token", TOKEN);
    }
    let response = neuro_gateway::http::router::build_router(state)
        .oneshot(request.body(Body::empty()).unwrap())
        .await
        .unwrap();
    let status = response.status().as_u16();
    let body = to_bytes(response.into_body(), 64 * 1024).await.unwrap();
    (status, serde_json::from_slice(&body).unwrap())
}
