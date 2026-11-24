use super::mutation::MutationKind::{Cancel, Delete};
use super::*;
use crate::{local_runtime::LocalRuntime, protocol::canonical::TokenUsage};
use serde_json::json;

async fn exercise(writer: Store<'_>, reader: Store<'_>, key: &str) {
    let pin = fixture_pin(key, matches!(writer, Store::Hosted(_)));
    let owner = "a".repeat(64);
    let binding = "b".repeat(64);
    let wrong_owner = "c".repeat(64);
    let upstream = UpstreamId::parse(&format!("resp_{}", uuid::Uuid::new_v4().simple())).unwrap();
    assert!(writer
        .begin("bad owner", &binding, "account", "credential", &pin)
        .await
        .is_err());
    let intent = writer
        .begin(&owner, &binding, "account", "credential", &pin)
        .await
        .unwrap();
    assert!(reader.route_pin(&intent).await.unwrap().as_ref() == Some(&pin));
    assert!(writer
        .reserve_balance(&intent, "other-key", 10)
        .await
        .is_err());
    let (a, b) = tokio::join!(
        writer.reserve_balance(&intent, key, 10),
        reader.reserve_balance(&intent, key, 10)
    );
    assert_ne!(a.unwrap(), b.unwrap());
    assert_eq!(reader.remaining_tokens(key).await, 90);
    assert!(reader
        .read(&intent.id, &wrong_owner)
        .await
        .unwrap()
        .is_none());
    assert!(reader.read(&upstream.0, &owner).await.unwrap().is_none());
    let recovered = reader.read(&intent.id, &owner).await.unwrap().unwrap();
    assert_eq!(recovered.phase, "submitting");
    assert!(recovered.upstream_id.is_none());
    assert_eq!(
        (
            recovered.provider_ref.as_str(),
            recovered.credential_ref.as_str()
        ),
        ("account", "credential")
    );
    let pending = writer
        .observe(&intent, &upstream, State::Pending)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(pending.settled, 0);
    let (a, b) = tokio::join!(
        writer.claim_mutation(&pending, Cancel),
        reader.claim_mutation(&pending, Cancel)
    );
    let (a, b) = (a.unwrap(), b.unwrap());
    assert_ne!(a.is_some(), b.is_some());
    let cancel = a.or(b).unwrap();
    let abroad = UpstreamId::parse("resp_other").unwrap();
    assert!(reader
        .observe(&pending, &abroad, State::Failed(None))
        .await
        .unwrap()
        .is_none());
    let usage = TokenUsage {
        prompt_tokens: 2,
        completion_tokens: 3,
        total_tokens: 5,
        cache_creation_input_tokens: None,
        cache_read_input_tokens: None,
    };
    let terminal = writer
        .observe(&pending, &upstream, State::Completed(None))
        .await
        .unwrap()
        .unwrap();
    assert!(reader.settle_balance(&terminal).await.unwrap().is_none());
    assert_eq!(reader.remaining_tokens(key).await, 90);
    for (row, id, state) in [
        (&terminal, &abroad, State::Completed(Some(usage.clone()))),
        (&terminal, &upstream, State::Failed(Some(usage.clone()))),
        (&pending, &upstream, State::Completed(Some(usage.clone()))),
    ] {
        assert!(reader.observe(row, id, state).await.unwrap().is_none());
    }
    let impossible = TokenUsage {
        prompt_tokens: i64::MAX as u64,
        completion_tokens: 1,
        total_tokens: i64::MAX as u64 + 1,
        ..usage.clone()
    };
    assert!(reader
        .observe(&terminal, &upstream, State::Completed(Some(impossible)))
        .await
        .is_err());
    let (a, b) = tokio::join!(
        writer.observe(&terminal, &upstream, State::Completed(Some(usage.clone()))),
        reader.observe(&terminal, &upstream, State::Completed(Some(usage)))
    );
    let (a, b) = (a.unwrap(), b.unwrap());
    assert_ne!(a.is_some(), b.is_some());
    let terminal = a.or(b).unwrap();
    assert!(terminal.terminal());
    assert_eq!(
        (terminal.input_tokens, terminal.output_tokens),
        (Some(2), Some(3))
    );
    let conflicting = TokenUsage {
        prompt_tokens: 3,
        completion_tokens: 3,
        total_tokens: 6,
        cache_creation_input_tokens: None,
        cache_read_input_tokens: None,
    };
    assert!(writer
        .observe(&terminal, &upstream, State::Completed(Some(conflicting)))
        .await
        .unwrap()
        .is_none());
    assert!(writer
        .claim_mutation(&terminal, Delete)
        .await
        .unwrap()
        .is_none());
    let (a, b) = tokio::join!(
        writer.settle_balance(&terminal),
        reader.settle_balance(&terminal)
    );
    let (a, b) = (a.unwrap(), b.unwrap());
    assert_ne!(a.is_some(), b.is_some());
    let settled = a.or(b).unwrap();
    assert_eq!(reader.remaining_tokens(key).await, 95);
    assert!(reader
        .claim_mutation(&settled, Delete)
        .await
        .unwrap()
        .is_none());
    reader
        .confirm_mutation(
            cancel,
            &json!({
                "id": upstream.0, "object": "response", "store": true,
                "output": [], "status": "cancelled"
            }),
        )
        .await
        .unwrap();
    let (a, b) = tokio::join!(
        writer.claim_mutation(&settled, Delete),
        reader.claim_mutation(&settled, Delete)
    );
    let (a, b) = (a.unwrap(), b.unwrap());
    assert_ne!(a.is_some(), b.is_some());
    let delete = a.or(b).unwrap();
    reader
        .confirm_mutation(
            delete,
            &json!({
                "id": upstream.0, "object": "response.deleted", "deleted": true
            }),
        )
        .await
        .unwrap();
    let deleted = writer.read(&settled.id, &owner).await.unwrap().unwrap();
    assert_eq!(deleted.phase, "deleted");
    assert_eq!(deleted.settled, 1);
    assert!(writer
        .observe(&pending, &upstream, State::Failed(None))
        .await
        .unwrap()
        .is_none());
    let duplicate = reader
        .begin(&wrong_owner, &binding, "account", "credential", &pin)
        .await
        .unwrap();
    assert!(reader
        .observe(&duplicate, &upstream, State::Pending)
        .await
        .is_err());

    let cancelled_id =
        UpstreamId::parse(&format!("resp_{}", uuid::Uuid::new_v4().simple())).unwrap();
    let intent = writer
        .begin(&owner, &binding, "account", "credential", &pin)
        .await
        .unwrap();
    assert!(writer.reserve_balance(&intent, key, 10).await.unwrap());
    let pending = writer
        .observe(&intent, &cancelled_id, State::Pending)
        .await
        .unwrap()
        .unwrap();
    let claim = writer
        .claim_mutation(&pending, Cancel)
        .await
        .unwrap()
        .unwrap();
    reader
        .observe(&pending, &cancelled_id, State::Failed(None))
        .await
        .unwrap()
        .unwrap();
    writer
        .confirm_mutation(
            claim,
            &json!({
                "id": cancelled_id.0, "object":"response", "store":true, "output":[],
                "status":"cancelled", "usage":{"input_tokens":0,"output_tokens":0,"total_tokens":0}
            }),
        )
        .await
        .unwrap();
    let cancelled = reader.read(&intent.id, &owner).await.unwrap().unwrap();
    assert_eq!(
        (cancelled.input_tokens, cancelled.output_tokens),
        (Some(0), Some(0))
    );
    assert!(reader.settle_balance(&cancelled).await.unwrap().is_some());
    assert_eq!(reader.remaining_tokens(key).await, 95);
}

#[tokio::test]
async fn sap_aicore_response_receipts_sqlite_additive_reopen_and_cas() {
    let fixture_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../linshi");
    let parent = if fixture_dir.is_dir() {
        fixture_dir
    } else {
        std::env::temp_dir()
    };
    let root = parent.join(format!(
        "gateway-sap-receipts-{}",
        uuid::Uuid::new_v4().simple()
    ));
    std::fs::create_dir(&root).unwrap();
    // Simulate a previously shipped schema-1 file using only this disposable fixture.
    let old = LocalRuntime::open(&root).await.unwrap();
    sqlx::raw_sql("DROP TABLE gateway_sap_response_route_pins; CREATE TABLE legacy_marker (value TEXT); INSERT INTO legacy_marker VALUES ('kept');")
        .execute(&old.pool).await.unwrap();
    let legacy_id = format!("resp_gw_{}", uuid::Uuid::new_v4().simple());
    sqlx::query("INSERT INTO gateway_sap_response_receipts (id, owner_tag, binding_tag, provider_ref, credential_ref, phase)
                 VALUES ($1, $2, $3, 'account', 'credential', 'submitting')")
        .bind(&legacy_id).bind("a".repeat(64)).bind("b".repeat(64)).execute(&old.pool).await.unwrap();
    old.close().await;
    let writer = LocalRuntime::open(&root).await.unwrap();
    let reader = LocalRuntime::open(&root).await.unwrap();
    let key = writer
        .save_access_key_with_quota(
            None,
            crate::db::UpsertAccessKeyInput {
                owner_type: "user".into(),
                owner_id: "fixture".into(),
                resolved_project_id: "local".into(),
                resolved_tenant_id: "local".into(),
                key_kind: "user".into(),
                public_key_prefix: "sk-gw".into(),
                display_name: "SAP fixture".into(),
                expires_at: None,
                bundle_ids: vec![],
                metadata: None,
            },
            Some(&crate::access_balance::quota::KeyQuotaInput {
                mode: "token_prepaid".into(),
                limit: Some(100),
                currency: None,
            }),
        )
        .await
        .unwrap();
    exercise(
        Store::Local(&writer.pool),
        Store::Local(&reader.pool),
        &key.id,
    )
    .await;
    let store = Store::Local(&writer.pool);
    let owner = "a".repeat(64);
    let legacy = store.read(&legacy_id, &owner).await.unwrap().unwrap();
    assert!(store.route_pin(&legacy).await.unwrap().is_none());
    assert!(store.reserve_balance(&legacy, &key.id, 7).await.is_err());
    let unknown = store
        .begin(
            &owner,
            &"b".repeat(64),
            "pool",
            "key",
            &fixture_pin(&key.id, false),
        )
        .await
        .unwrap();
    assert!(store.reserve_balance(&unknown, &key.id, 7).await.unwrap());
    let pending = store
        .observe(
            &unknown,
            &UpstreamId::parse("resp_unknown").unwrap(),
            State::Pending,
        )
        .await
        .unwrap()
        .unwrap();
    let lost = store
        .claim_mutation(&pending, Cancel)
        .await
        .unwrap()
        .unwrap();
    // An invalid acknowledgment leaves a durable unknown outcome, not a retry permit.
    assert!(store
        .confirm_mutation(lost, &json!({"id":"resp_wrong"}))
        .await
        .is_err());
    writer.close().await;
    let writer = LocalRuntime::open(&root).await.unwrap();
    let store = Store::Local(&writer.pool);
    assert!(store
        .claim_mutation(&pending, Cancel)
        .await
        .unwrap()
        .is_none());
    let terminal = store
        .observe(
            &pending,
            &UpstreamId::parse("resp_unknown").unwrap(),
            State::Failed(None),
        )
        .await
        .unwrap()
        .unwrap();
    let rotated = writer.rotate_access_key(&key.id).await.unwrap();
    writer.delete_access_key(&key.id).await.unwrap();
    writer.close().await;
    let recovered = Store::Local(&reader.pool);
    assert!(recovered.settle_balance(&terminal).await.unwrap().is_none());
    assert!(!recovered
        .reserve_balance(&unknown, &key.id, 7)
        .await
        .unwrap());
    assert_eq!(
        reader
            .access_balance(&rotated.id)
            .await
            .unwrap()
            .unwrap()
            .remaining_tokens,
        Some(88)
    );
    let result = recovered.read(&unknown.id, &owner).await.unwrap().unwrap();
    assert_eq!(result.settled, 0);
    let late = recovered
        .observe(
            &result,
            &UpstreamId::parse("resp_unknown").unwrap(),
            State::Failed(Some(TokenUsage {
                prompt_tokens: 2,
                completion_tokens: 1,
                total_tokens: 3,
                cache_creation_input_tokens: None,
                cache_read_input_tokens: None,
            })),
        )
        .await
        .unwrap()
        .unwrap();
    assert!(recovered.settle_balance(&late).await.unwrap().is_some());
    assert_eq!(
        reader
            .access_balance(&rotated.id)
            .await
            .unwrap()
            .unwrap()
            .remaining_tokens,
        Some(92)
    );
    let settled = recovered.read(&unknown.id, &owner).await.unwrap().unwrap();
    assert!(recovered
        .claim_mutation(&settled, Delete)
        .await
        .unwrap()
        .is_none());
    let count: i64 = sqlx::query_scalar("SELECT count(*) FROM legacy_marker WHERE value = 'kept'")
        .fetch_one(&reader.pool)
        .await
        .unwrap();
    assert_eq!(count, 1);
    let version: i64 = sqlx::query_scalar("PRAGMA user_version")
        .fetch_one(&reader.pool)
        .await
        .unwrap();
    assert_eq!(version, 1);
    reader.close().await;
}

#[tokio::test]
#[ignore = "requires the isolated loopback SAP fixture PostgreSQL"]
async fn sap_aicore_response_receipts_postgres_independent_connections_cas() {
    let url = std::env::var("SAP_AICORE_TEST_DATABASE_URL").unwrap();
    let target = url::Url::parse(&url).unwrap();
    assert_eq!(target.host_str(), Some("127.0.0.1"));
    assert_eq!(target.path(), "/gateway_fixture");
    assert!(target.port().is_some_and(|p| p != 5432));
    let writer = sqlx::postgres::PgPoolOptions::new()
        .max_connections(1)
        .connect(&url)
        .await
        .unwrap();
    let reader = sqlx::postgres::PgPoolOptions::new()
        .max_connections(1)
        .connect(&url)
        .await
        .unwrap();
    let schema = format!("sap_receipts_{}", uuid::Uuid::new_v4().simple());
    sqlx::raw_sql(&format!(
        "CREATE SCHEMA {schema}; SET search_path TO {schema};"
    ))
    .execute(&writer)
    .await
    .unwrap();
    sqlx::raw_sql(&format!("SET search_path TO {schema};"))
        .execute(&reader)
        .await
        .unwrap();
    assert!(Store::Hosted(&writer)
        .begin(
            &"a".repeat(64),
            &"b".repeat(64),
            "pool",
            "key",
            &fixture_pin("fixture", true)
        )
        .await
        .is_err());
    sqlx::raw_sql(SCHEMA).execute(&writer).await.unwrap();
    sqlx::raw_sql(SCHEMA).execute(&writer).await.unwrap();
    sqlx::raw_sql(MUTATION_SCHEMA)
        .execute(&writer)
        .await
        .unwrap();
    let now = Store::Hosted(&writer);
    assert!(now
        .begin(
            &"a".repeat(64),
            &"b".repeat(64),
            "pool",
            "key",
            &fixture_pin("fixture", true)
        )
        .await
        .is_err());
    let count: i64 = sqlx::query_scalar("SELECT count(*) FROM gateway_sap_response_receipts")
        .fetch_one(&writer)
        .await
        .unwrap();
    assert_eq!(count, 0);
    for _ in 0..2 {
        sqlx::raw_sql(ROUTE_SCHEMA).execute(&writer).await.unwrap();
    }
    sqlx::raw_sql(MUTATION_SCHEMA)
        .execute(&writer)
        .await
        .unwrap();
    sqlx::raw_sql("CREATE TABLE gateway_access_keys (id TEXT PRIMARY KEY, status TEXT NOT NULL, expires_at TIMESTAMPTZ);
        CREATE TABLE gateway_access_key_balances (access_key_id TEXT PRIMARY KEY REFERENCES gateway_access_keys(id),
          balance_mode TEXT NOT NULL, status TEXT NOT NULL, unlimited_until TIMESTAMPTZ, period_starts_at TIMESTAMPTZ,
          period_ends_at TIMESTAMPTZ, total_tokens BIGINT, remaining_tokens BIGINT, total_messages BIGINT, remaining_messages BIGINT,
          updated_at TIMESTAMPTZ NOT NULL);
        INSERT INTO gateway_access_keys (id,status) VALUES ('fixture','active');
        INSERT INTO gateway_access_key_balances (access_key_id,balance_mode,status,total_tokens,remaining_tokens,updated_at)
          VALUES ('fixture','token_prepaid','active',100,100,now());")
        .execute(&writer).await.unwrap();
    exercise(Store::Hosted(&writer), Store::Hosted(&reader), "fixture").await;
    for _ in 0..2 {
        sqlx::raw_sql(RECOVERY_SCHEMA)
            .execute(&writer)
            .await
            .unwrap();
    }
    recovery::tests::exercise(Store::Hosted(&writer), Store::Hosted(&reader), "fixture").await;
    recovery::message_tests::hosted(&writer, &reader).await;
    // Keep the isolated schema for evidence; no production schema was selected or modified.
    writer.close().await;
    reader.close().await;
}

impl Store<'_> {
    pub(super) async fn remaining_tokens(&self, key: &str) -> i64 {
        match self {
            Store::Local(pool) => sqlx::query_scalar::<_,i64>("SELECT json_extract(payload,'$.remainingTokens') FROM local_balance_accounts WHERE id=$1").bind(key).fetch_one(*pool).await.unwrap(),
            Store::Hosted(pool) => sqlx::query_scalar::<_,i64>("SELECT remaining_tokens FROM gateway_access_key_balances WHERE access_key_id=$1").bind(key).fetch_one(*pool).await.unwrap(),
        }
    }
}

fn fixture_pin(key: &str, hosted: bool) -> RoutePin {
    RoutePin {
        public_model: "public-model".into(),
        source_key_ref: key.into(),
        platform_access_ref: hosted.then(|| "platform-access".into()),
    }
}
