//! Fresh guarded schema uses the real repository DDL, not a simplified mock DB contract.
#[path = "../../../tests/pipeline_send_runtime/config.rs"]
mod config;
use crate::{
    config::Config,
    console::refresh_grants::{GrantPhase, GrantRecord},
    console::{ConsoleConfig, ConsoleConfigValues},
};
use redis::AsyncCommands;
use sqlx::{
    postgres::{PgConnectOptions, PgPoolOptions},
    PgPool,
};
use std::{path::PathBuf, str::FromStr, time::Duration};
pub(super) struct Fixture {
    pub pg: PgPool,
    pub config: Config,
    admin: PgPool,
    schema: String,
    pub root: PathBuf,
}

async fn apply_script(pg: &PgPool, script: &str, schema: &str) {
    let script = script
        .replace("\r\n", "\n")
        .replace("public.", &format!("{schema}."))
        .replace(
            "SELECT pg_catalog.set_config('search_path', '', false);",
            "",
        );
    let copy =
        format!("COPY {schema}.gateway_schema_migrations (file_name, applied_at) FROM stdin;");
    let Some((before, copied)) = script.split_once(&copy) else {
        sqlx::raw_sql(&script).execute(pg).await.unwrap();
        return;
    };
    let (data, after) = copied
        .strip_prefix('\n')
        .unwrap()
        .split_once("\\.\n")
        .unwrap();
    assert!(
        !after.contains("COPY "),
        "only the known migration-ledger dump is supported"
    );
    // raw_sql cannot consume psql's inline COPY data. Keep the original rows via SQLx's protocol.
    let mut conn = pg.acquire().await.unwrap();
    sqlx::raw_sql(before).execute(&mut *conn).await.unwrap();
    let mut input = conn.copy_in_raw(&copy).await.unwrap();
    input.send(data.as_bytes()).await.unwrap();
    assert_eq!(input.finish().await.unwrap(), data.lines().count() as u64);
    sqlx::raw_sql(after).execute(&mut *conn).await.unwrap();
}

impl Fixture {
    pub async fn new() -> Self {
        let run = std::env::var("GATEWAY_DELETE_TEST_RUN_ID").unwrap();
        assert!(run.len() == 32 && run.bytes().all(|b| b.is_ascii_hexdigit()));
        let database = std::env::var("GATEWAY_DELETE_TEST_DATABASE_URL").unwrap();
        let redis = std::env::var("GATEWAY_DELETE_TEST_REDIS_URL").unwrap();
        let mut url = url::Url::parse(&database).unwrap();
        assert_eq!(url.host_str(), Some("127.0.0.1"));
        assert_eq!(url.path(), "/gateway_delete_fixture");
        assert_ne!(url.port(), Some(5432));
        assert_eq!(
            url::Url::parse(&redis).unwrap().host_str(),
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
        let pool = crate::redis::pool::create_pool(&redis).unwrap();
        let mut conn = pool.get().await.unwrap();
        let guard: String = conn.get("gw:management-delete:contract_run").await.unwrap();
        assert_eq!(guard, run);
        drop(conn);
        let schema = format!("accio_server_{}", uuid::Uuid::new_v4().simple());
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
        for script in [
            include_str!("../../../deploy/postgres/initdb/001-gateway-schema.sql"),
            include_str!("../../../deploy/postgres/initdb/002-gateway-bootstrap.sql"),
            include_str!("../../../deploy/postgres/initdb/004-gateway-key-cash.sql"),
            include_str!("../../../deploy/postgres/initdb/005-gateway-refresh-authority.sql"),
        ] {
            // Namespace-only transform; retain production columns, constraints and defaults.
            // Keep the DB provider audit FK: standalone route-document relaxation is not needed here.
            Box::pin(apply_script(&pg, script, &schema)).await;
        }
        let root = std::env::temp_dir().join(&schema);
        std::fs::create_dir_all(&root).unwrap();
        let console = ConsoleConfig::from_values(ConsoleConfigValues {
            state_dir: Some(root.join("state")),
            routes_file: Some(root.join("routes.yaml")),
            redis_namespace: Some(schema.clone()),
            ..Default::default()
        })
        .unwrap();
        std::fs::write(
            &console.routes_file,
            "providers: []\nmodel_routes: []\naliases: {}\n",
        )
        .unwrap();
        let mut config = config::test_config(console);
        url.query_pairs_mut()
            .append_pair("options", &format!("-c search_path={schema}"));
        config.database_url = Some(url.to_string());
        config.redis_url = redis;
        config.default_project_id = "platform-default-project".into();
        config.gateway_api_key = Some("server-public-synthetic".into());
        config.gateway_management_token = Some("server-management-synthetic".into());
        config.provider_credential_refresh_before_secs = 300;
        Self {
            pg,
            config,
            admin,
            schema,
            root,
        }
    }
    pub async fn phase(&self) -> Option<GrantPhase> {
        let raw: Option<String> =
            sqlx::query_scalar("select record from gateway_provider_refresh_grants")
                .fetch_optional(&self.pg)
                .await
                .unwrap();
        raw.map(|raw| GrantRecord::from_json(&raw).unwrap().phase())
    }
    pub async fn wait_phase(&self, expected: GrantPhase) {
        tokio::time::timeout(Duration::from_secs(10), async {
            while self.phase().await != Some(expected) {
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
        })
        .await
        .unwrap();
    }
    pub async fn finish(self) {
        self.pg.close().await;
        let suffix = self.schema.strip_prefix("accio_server_").unwrap();
        assert!(suffix.len() == 32 && suffix.bytes().all(|b| b.is_ascii_hexdigit()));
        sqlx::query(&format!("drop schema {} cascade", self.schema))
            .execute(&self.admin)
            .await
            .unwrap();
        self.admin.close().await;
        // Keep TEMP evidence; only the caller-authorized container pair is removed.
        assert!(self.root.starts_with(std::env::temp_dir()));
    }
}
