use std::collections::HashMap;
use std::path::PathBuf;
use std::str::FromStr;

use ::redis::AsyncCommands;
use deadpool_redis::{Config, Pool, Runtime};
use sqlx::postgres::{PgConnectOptions, PgPoolOptions};
use sqlx::PgPool;

use super::{import_folder_credentials, FolderSyncCounters, HashSet};
use crate::error::GatewayError;
use crate::redis::keys;

#[path = "../../../../tests/provider_management_deletion/schema.rs"]
mod schema;

pub struct Fixture {
    pub pg: PgPool,
    pub root: PathBuf,
    redis: Pool,
    admin: PgPool,
    schema: String,
    ids: Vec<String>,
}

pub struct Outcome {
    pub result: Result<(), GatewayError>,
    pub counters: FolderSyncCounters,
    pub remaining: Vec<String>,
    pub deleted: Vec<String>,
    pub key_counts: HashMap<String, usize>,
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
            .max_connections(1)
            .connect(&database)
            .await
            .unwrap();
        let guard: String = sqlx::query_scalar("select run_id from public.deletion_fixture_guard")
            .fetch_one(&admin)
            .await
            .unwrap();
        assert_eq!(guard, run);
        let redis = Config::from_url(redis_url)
            .create_pool(Some(Runtime::Tokio1))
            .unwrap();
        let mut connection = redis.get().await.unwrap();
        let guard: String = connection
            .get("gw:management-delete:contract_run")
            .await
            .unwrap();
        assert_eq!(guard, run);
        drop(connection);
        let id = uuid::Uuid::new_v4().simple().to_string();
        let schema_name = format!("overlap_{id}");
        sqlx::query(&format!("create schema {schema_name}"))
            .execute(&admin)
            .await
            .unwrap();
        let options = PgConnectOptions::from_str(&database)
            .unwrap()
            .options([("search_path", schema_name.as_str())]);
        let pg = PgPoolOptions::new()
            .max_connections(2)
            .connect_with(options)
            .await
            .unwrap();
        schema::initialize(&pg).await;
        sqlx::raw_sql(
            "insert into gateway_provider_accounts (id,label) values ('fixture-account','Fixture');
             create table deletion_log (sequence bigserial, id text);
             create table blocked_deletions (id text primary key);
             create function trace_deletion() returns trigger language plpgsql as $$
             begin insert into deletion_log(id) values (old.id); return old; end; $$;
             create trigger trace_deletion after delete on gateway_provider_credentials
             for each row execute function trace_deletion();
             create function block_deletion() returns trigger language plpgsql as $$
             begin if exists(select 1 from blocked_deletions where id=old.id) then
             return null; end if; return old; end; $$;
             create trigger block_deletion before delete on gateway_provider_credentials
             for each row execute function block_deletion();",
        )
        .execute(&pg)
        .await
        .unwrap();
        let root = std::env::temp_dir().join(format!("gateway-folder-overlap-{run}-{id}"));
        std::fs::create_dir(&root).unwrap();
        Self {
            pg,
            root,
            redis,
            admin,
            schema: schema_name,
            ids: Vec::new(),
        }
    }

    pub async fn add(&mut self, id: &str, path: &str) {
        sqlx::query(
            "insert into gateway_provider_credentials
             (id,provider_account_id,label,source_path,source_kind,created_at)
             values ($1,'fixture-account',$1,$2,'folder_sync_import',$3)",
        )
        .bind(id)
        .bind(path)
        .bind(time::OffsetDateTime::UNIX_EPOCH + time::Duration::seconds(self.ids.len() as i64))
        .execute(&self.pg)
        .await
        .unwrap();
        let mut connection = self.redis.get().await.unwrap();
        for key in runtime_keys(id) {
            let _: () = connection.set(key, "fixture").await.unwrap();
        }
        self.ids.push(id.to_string());
    }

    pub async fn block(&self, id: &str) {
        sqlx::query("insert into blocked_deletions values ($1)")
            .bind(id)
            .execute(&self.pg)
            .await
            .unwrap();
    }

    pub async fn run(self, delete_missing: bool, explicit: &[&str]) -> Outcome {
        let mut counters = FolderSyncCounters::default();
        let explicit = explicit
            .iter()
            .map(|path| path.to_string())
            .collect::<HashSet<_>>();
        let result = import_folder_credentials(
            &self.pg,
            &self.redis,
            &self.root,
            delete_missing,
            &explicit,
            &mut counters,
        )
        .await;
        let remaining =
            sqlx::query_scalar("select id from gateway_provider_credentials order by id")
                .fetch_all(&self.pg)
                .await
                .unwrap();
        let deleted = sqlx::query_scalar("select id from deletion_log order by sequence")
            .fetch_all(&self.pg)
            .await
            .unwrap();
        let mut connection = self.redis.get().await.unwrap();
        let mut key_counts = HashMap::new();
        for id in &self.ids {
            key_counts.insert(
                id.clone(),
                connection.exists(runtime_keys(id)).await.unwrap(),
            );
            let _: usize = connection.del(runtime_keys(id)).await.unwrap();
        }
        drop(connection);
        self.pg.close().await;
        assert!(
            self.schema.starts_with("overlap_")
                && self.schema[8..].bytes().all(|b| b.is_ascii_hexdigit())
        );
        sqlx::query(&format!("drop schema {} cascade", self.schema))
            .execute(&self.admin)
            .await
            .unwrap();
        self.admin.close().await;
        let root = self.root.clone();
        drop(self);
        assert!(!root.exists());
        Outcome {
            result,
            counters,
            remaining,
            deleted,
            key_counts,
        }
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        if self.root.parent() == Some(std::env::temp_dir().as_path())
            && self
                .root
                .file_name()
                .unwrap()
                .to_string_lossy()
                .starts_with("gateway-folder-overlap-")
        {
            let _ = std::fs::remove_dir_all(&self.root);
        }
    }
}

fn runtime_keys(id: &str) -> Vec<String> {
    vec![
        keys::provider_credential_failure_count_key(id),
        keys::provider_credential_breaker_open_key(id),
        keys::provider_credential_quota_snapshot_key(id),
        keys::provider_credential_quota_lock_key(id),
    ]
}
