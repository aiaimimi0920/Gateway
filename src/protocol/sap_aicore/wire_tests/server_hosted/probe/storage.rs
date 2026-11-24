//! Missing additive schema fails closed; a lost finalization remains unknown rather than unsent.
use super::*;

#[test]
#[ignore = "Requires dedicated PostgreSQL/Redis; retains a unique isolated schema"]
fn sap_aicore_server_probe_missing_extension_and_legacy_migration_are_safe() {
    run_hosted(migration);
}

async fn migration() {
    let peer = peer::Peer::new("chat", false, false, 200).await;
    let scope = format!("sap-probe-migration-{}", uuid::Uuid::new_v4().simple());
    let reader = app(&scope, "reader", &peer).await;
    // Generated identifier, dedicated database, additive schema only; never drop a shared table.
    let schema = format!("sap_probe_{}", uuid::Uuid::new_v4().simple());
    sqlx::raw_sql(&format!("CREATE SCHEMA {schema}"))
        .execute(reader.pg_pool.as_ref().unwrap())
        .await
        .unwrap();
    let options = std::env::var("SAP_AICORE_TEST_DATABASE_URL")
        .unwrap()
        .parse::<sqlx::postgres::PgConnectOptions>()
        .unwrap()
        .options([("search_path", schema.as_str())]);
    // Bind every connection (including reconnects) to this test's isolated schema.
    let pool = sqlx::postgres::PgPoolOptions::new()
        .max_connections(1)
        .connect_with(options)
        .await
        .unwrap();
    sqlx::raw_sql(include_str!(
        "../../../../../../deploy/postgres/initdb/007-gateway-internal-model-probe-audits.sql"
    ))
    .execute(&pool)
    .await
    .unwrap();
    // Inject the pool before management requests can retain another AppState owner.
    let mut writer = build(&scope, "writer", &peer).await;
    let original = Arc::get_mut(&mut writer)
        .unwrap()
        .pg_pool
        .replace(pool.clone())
        .unwrap();
    writer
        .route_config
        .replace_document(reader.route_config.snapshot().document().clone())
        .unwrap();
    let report = probe(&peer, &writer).await;
    assert_eq!(report.status, ProviderPayloadProbeStatus::Failed);
    assert!(report.message.contains("no request was sent"), "{report:?}");
    assert!(peer.oauth.lock().unwrap().is_empty() && peer.calls.lock().unwrap().is_empty());
    sqlx::query(
        "INSERT INTO gateway_internal_model_probe_audits
        (id,namespace,product,provider_id,credential_id,model) VALUES
        ('legacy','legacy','commandcode','provider','credential','model')",
    )
    .execute(&pool)
    .await
    .unwrap();
    for _ in 0..2 {
        sqlx::raw_sql(SCHEMA).execute(&pool).await.unwrap();
    }
    let legacy: Option<Value> = sqlx::query_scalar(
        "SELECT sap_evidence FROM gateway_internal_model_probe_audits WHERE id='legacy'",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert!(
        legacy.is_none(),
        "migration must not invent attempts for old records"
    );
    assert_eq!(
        probe(&peer, &writer).await.status,
        ProviderPayloadProbeStatus::Passed
    );
    let (status, evidence): (String, Value) = sqlx::query_as(
        "SELECT status,sap_evidence FROM gateway_internal_model_probe_audits WHERE namespace=$1",
    )
    .bind(&scope)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(status, "completed");
    assert_eq!(evidence["attempts"], 1);
    original.close().await;
    finish(writer, reader, peer).await;
}

#[test]
#[ignore = "Requires dedicated PostgreSQL/Redis; closes only this test writer pool"]
fn sap_aicore_server_probe_finalization_failure_is_not_success_or_unsent_proof() {
    run_hosted(finalization);
}

async fn finalization() {
    let peer = peer::Peer::new("chat", false, false, 200).await;
    let scope = format!("sap-probe-finalize-{}", uuid::Uuid::new_v4().simple());
    let writer = app(&scope, "writer", &peer).await;
    let reader = build(&scope, "reader", &peer).await;
    sqlx::raw_sql(SCHEMA)
        .execute(writer.pg_pool.as_ref().unwrap())
        .await
        .unwrap();
    let pause = Arc::new(Pause::default());
    let hooks = Hooks {
        finish: Some(pause.clone()),
        ..Hooks::default()
    };
    let mut pending = Box::pin(hooks.within(probe(&peer, &writer)));
    tokio::time::timeout(Duration::from_secs(5), async {
        tokio::select! { _ = pause.entered() => {}, _ = &mut pending => panic!("finalization barrier missed") }
    }).await.unwrap();
    let before = rows(&reader, &scope).await;
    assert_eq!(before.len(), 1);
    assert_eq!(before[0].status, "running");
    assert!(before[0].sap_evidence.is_none());
    assert_eq!(peer.calls.lock().unwrap().len(), 1);
    writer.pg_pool.as_ref().unwrap().close().await;
    pause.release();
    let report = tokio::time::timeout(Duration::from_secs(5), pending)
        .await
        .unwrap();
    assert_eq!(report.status, ProviderPayloadProbeStatus::Failed);
    assert!(
        report.message.contains("request was not replayed"),
        "{report:?}"
    );
    assert_eq!(rows(&reader, &scope).await, before);
    assert_eq!(peer.calls.lock().unwrap().len(), 1);
    finish(writer, reader, peer).await;
}
