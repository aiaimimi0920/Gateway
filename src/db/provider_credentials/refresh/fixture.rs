//! Opt-in tests reuse the existing row schema in a run-guarded, loopback-only database.
#[path = "../../../../tests/provider_credential_folder_sync_database/schema.rs"]
mod schema;
use super::{journal, snapshot::Snapshot, snapshot_tests::rows};
use crate::console::refresh_grants::{GrantRecord, GrantReservation, GrantResult};
use serde_json::Value;
use sqlx::{
    postgres::{PgConnectOptions, PgPoolOptions},
    types::Json,
    PgPool,
};
use std::str::FromStr;

pub(super) struct Fixture {
    pub pool: PgPool,
    pub credential: String,
    pub account: String,
    schema: String,
    admin: PgPool,
}
impl Fixture {
    pub async fn new() -> Self {
        let run = std::env::var("GATEWAY_DELETE_TEST_RUN_ID").expect("guarded fixture run id");
        assert!(run.len() == 32 && run.bytes().all(|b| b.is_ascii_hexdigit()));
        let database = std::env::var("GATEWAY_DELETE_TEST_DATABASE_URL").unwrap();
        let url = url::Url::parse(&database).unwrap();
        assert_eq!(url.host_str(), Some("127.0.0.1"));
        assert_eq!(url.path(), "/gateway_delete_fixture");
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
        let schema = format!("accio_refresh_{}", uuid::Uuid::new_v4().simple());
        sqlx::query(&format!("create schema {schema}"))
            .execute(&admin)
            .await
            .unwrap();
        let options = PgConnectOptions::from_str(&database)
            .unwrap()
            .options([("search_path", schema.as_str())]);
        let pool = PgPoolOptions::new()
            .max_connections(4)
            .connect_with(options)
            .await
            .unwrap();
        schema::initialize(&pool).await;
        let mut tx = pool.begin().await.unwrap();
        sqlx::raw_sql(include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/deploy/postgres/initdb/005-gateway-refresh-authority.sql"
        )))
        .execute(&mut *tx)
        .await
        .unwrap();
        tx.commit().await.unwrap();
        let account = format!("account-{}", uuid::Uuid::new_v4().simple());
        let credential = format!("credential-{}", uuid::Uuid::new_v4().simple());
        let (_, _, ap, cp) = rows();
        sqlx::query("insert into gateway_provider_accounts(id,label,adapter,payload_inline) values($1,'retained account','accio_compatible',$2)")
            .bind(&account).bind(Json(ap)).execute(&pool).await.unwrap();
        sqlx::query("insert into gateway_provider_credentials(id,provider_account_id,label,payload_inline,source_kind,sync_mode,source_hash,sync_error,last_error,failure_count) values($1,$2,'retained credential',$3,'manual','manual','retained-hash','retained sync','retained health',7)")
            .bind(&credential).bind(&account).bind(Json(cp)).execute(&pool).await.unwrap();
        Self {
            pool,
            credential,
            account,
            schema,
            admin,
        }
    }
    pub async fn snapshot(&self) -> Snapshot {
        journal::read_snapshot(&self.pool, &self.credential)
            .await
            .unwrap()
            .unwrap()
    }
    pub async fn sending(&self) -> (Snapshot, GrantRecord) {
        let snapshot = self.snapshot().await;
        let GrantReservation::Reserved(record) =
            journal::reserve(&self.pool, &snapshot).await.unwrap()
        else {
            panic!("first admission must reserve")
        };
        (snapshot, record)
    }
    pub fn result(snapshot: &Snapshot) -> GrantResult {
        snapshot
            .request
            .result_with_accio(
                "synthetic-successor",
                None,
                3600,
                "2027-01-01T00:00:00.123456789Z",
                None,
                None,
            )
            .unwrap()
    }
    pub async fn row(&self) -> Value {
        sqlx::query_scalar::<_, Json<Value>>(
            "select to_jsonb(c) from gateway_provider_credentials c where id=$1",
        )
        .bind(&self.credential)
        .fetch_one(&self.pool)
        .await
        .unwrap()
        .0
    }
    pub async fn record(&self, original: &GrantRecord) -> GrantRecord {
        let raw: String = sqlx::query_scalar(
            "select record from gateway_provider_refresh_grants where family=$1 and material=$2",
        )
        .bind(original.family_id())
        .bind(original.material_id())
        .fetch_one(&self.pool)
        .await
        .unwrap();
        GrantRecord::from_json(&raw).unwrap()
    }
    pub async fn finish(self) {
        self.pool.close().await;
        let tail = self.schema.strip_prefix("accio_refresh_").unwrap();
        assert!(tail.len() == 32 && tail.bytes().all(|b| b.is_ascii_hexdigit()));
        sqlx::query(&format!("drop schema {} cascade", self.schema))
            .execute(&self.admin)
            .await
            .unwrap();
        self.admin.close().await;
    }
}
