use sqlx::{
    postgres::{PgConnectOptions, PgPoolOptions},
    Connection, Executor,
};

use super::fixture::{candidate, Upstream};
use super::health_store::{contract, HealthStore};

#[tokio::test]
#[ignore = "requires guarded loopback PostgreSQL fixture"]
async fn postgres_cooldown_writer_reader_and_candidate_contract() {
    let url = std::env::var("GATEWAY_REFERENCE_TEST_DATABASE_URL").expect("owned fixture URL");
    let run = std::env::var("GATEWAY_REFERENCE_TEST_RUN_ID").expect("owned fixture guard");
    assert_eq!(run.len(), 32);
    assert!(run.bytes().all(|c| c.is_ascii_hexdigit()));
    let options: PgConnectOptions = url.parse().unwrap();
    assert_eq!(options.get_host(), "127.0.0.1");
    assert_eq!(options.get_database(), Some("gateway_reference_fixture"));
    assert_ne!(
        options.get_port(),
        5432,
        "never borrow the host application DB"
    );
    let mut admin = sqlx::PgConnection::connect_with(&options).await.unwrap();
    let guard: String = sqlx::query_scalar("SELECT run_id FROM public.reference_fixture_guard")
        .fetch_one(&mut admin)
        .await
        .unwrap();
    assert_eq!(guard, run);
    let schema = format!("reference_{}", uuid::Uuid::new_v4().simple());
    admin
        .execute(format!("CREATE SCHEMA {schema}").as_str())
        .await
        .unwrap();
    let selected = schema.clone();
    let pool = PgPoolOptions::new()
        .max_connections(2)
        .after_connect(move |connection, _| {
            let query = format!("SET search_path TO {selected}");
            Box::pin(async move {
                connection.execute(query.as_str()).await?;
                Ok(())
            })
        })
        .connect_with(options)
        .await
        .unwrap();
    // Same columns and SQL types as the production table, confined to a new
    // schema inside a verified disposable database. No live migrations run.
    pool.execute(
        "CREATE TABLE gateway_provider_credential_model_states (
        id text PRIMARY KEY, provider_account_id text NOT NULL,
        provider_credential_id text, provider_credential_ref text, protocol_profile text,
        model text NOT NULL, status text NOT NULL DEFAULT 'active',
        failure_class text, failure_scope text, failure_count integer NOT NULL DEFAULT 0,
        last_error text, last_upstream_status integer, cooldown_until timestamptz,
        last_success_at timestamptz, last_failure_at timestamptz,
        created_at timestamptz NOT NULL DEFAULT now(), updated_at timestamptz NOT NULL DEFAULT now()
    )",
    )
    .await
    .unwrap();
    let upstream = Upstream::start(axum::http::StatusCode::OK, false, "{}").await;
    contract(HealthStore::Pg(&pool), candidate(&upstream, "pg-model")).await;
    upstream.finish().await;
    pool.close().await;
    admin.close().await.unwrap();
    // Leave only fixture-owned rows until the coordinator stops this tmpfs DB;
    // do not issue broad cleanup SQL against any inherited database.
}
