use super::*;

#[tokio::test]
async fn local_shutdown_waits_for_detached_stream_finalizer() {
    let (root, db) = setup().await;
    db.create_audit(create_input("stream-close", true))
        .await
        .unwrap();
    let guard = db.track_finalizer();
    let callback_db = db.clone();
    let callback = tokio::spawn(async move {
        let _guard = guard;
        tokio::time::sleep(std::time::Duration::from_millis(20)).await;
        callback_db
            .finalize_audit("stream-close", finish_input())
            .await
            .unwrap();
    });
    db.close().await;
    callback.await.unwrap();
    let db = LocalRuntime::open(&root).await.unwrap();
    let rows = db
        .list_audits(&RequestAuditFilters::default())
        .await
        .unwrap();
    assert_eq!(rows[0].status, "completed");
    db.close().await;
    crate::local_runtime::test_support::remove_test_root(&root).await;
}

#[tokio::test]
async fn local_pressure_counts_all_running_requests_without_list_limit() {
    let (root, db) = setup().await;
    db.create_audit(create_input("base", false)).await.unwrap();
    sqlx::query("WITH RECURSIVE n(i) AS (SELECT 1 UNION ALL SELECT i + 1 FROM n WHERE i < 10001)
        INSERT INTO request_audits (id,payload,created,status,owner)
        SELECT 'copy-' || i, payload, created, status, owner FROM n, request_audits WHERE id = 'base'")
        .execute(&db.pool).await.unwrap();
    let report = db
        .pressure(
            &std::collections::HashMap::new(),
            &[],
            &GatewayRuntimePressureFilters::default(),
        )
        .await
        .unwrap();
    assert_eq!(report.total_running_requests, 10002);
    assert_eq!(report.projects[0].running_request_count, 10002);
    assert_eq!(report.providers[0].running_request_count, 10002);
    db.close().await;
    crate::local_runtime::test_support::remove_test_root(&root).await;
}

#[tokio::test]
async fn local_schema_rejects_newer_database_without_rewriting_it() {
    let (root, db) = setup().await;
    sqlx::query("PRAGMA user_version = 2")
        .execute(&db.pool)
        .await
        .unwrap();
    db.close().await;
    assert!(LocalRuntime::open(&root).await.is_err());
    crate::local_runtime::test_support::remove_test_root(&root).await;
}
use crate::db::*;

fn create_input(id: &str, stream: bool) -> CreateRequestAuditInput {
    CreateRequestAuditInput {
        project_id: "local-project".into(),
        api_key_id: Some("gateway-key".into()),
        user_credential_id: None,
        access_key_id: None,
        source_access_key_id: None,
        session_id: None,
        route_policy_id: None,
        provider_account_id: Some("provider-a".into()),
        protocol_family: "openai".into(),
        endpoint_kind: "chat.completions".into(),
        requested_model: Some("unpriced-test-model".into()),
        resolved_model: None,
        model_alias: None,
        stream,
        route_attempt_count: 1,
        response_id: id.into(),
        previous_response_id: None,
        route_trace: None,
    }
}
fn finish_input() -> FinalizeRequestAuditInput {
    FinalizeRequestAuditInput {
        status: "completed".into(),
        upstream_status: Some(200),
        duration_ms: 12,
        prompt_tokens: Some(1000),
        completion_tokens: Some(500),
        total_tokens: Some(1500),
        cache_creation_input_tokens: None,
        cache_read_input_tokens: Some(100),
        client_has_cache_control: false,
        auto_cache_applied: false,
        error_summary: None,
        access_key_id: None,
        source_access_key_id: None,
        session_id: None,
        route_policy_id: None,
        provider_account_id: Some("provider-a".into()),
        resolved_model: Some("unpriced-test-model".into()),
        model_alias: None,
        route_attempt_count: 2,
        route_trace: None,
        response_id: Some("upstream-id".into()),
    }
}
async fn setup() -> (std::path::PathBuf, LocalRuntime) {
    let root = std::env::temp_dir().join(format!("gateway-local-audit-{}", uuid::Uuid::new_v4()));
    let db = LocalRuntime::open(&root).await.unwrap();
    (root, db)
}

#[tokio::test]
async fn local_audit_restart_stream_terminal_idempotency_and_filters() {
    let (root, db) = setup().await;
    db.create_audit(create_input("a", true)).await.unwrap();
    db.create_audit(create_input("a", true)).await.unwrap();
    db.create_audit(create_input("b", false)).await.unwrap();
    db.finalize_audit("a", finish_input()).await.unwrap();
    let mut failure = finish_input();
    failure.status = "failed".into();
    db.finalize_audit("a", failure).await.unwrap();
    let live = LocalRuntime::open(&root).await.unwrap();
    let rows = live
        .list_audits(&RequestAuditFilters::default())
        .await
        .unwrap();
    assert_eq!(rows.len(), 2);
    assert_eq!(
        rows.iter().find(|r| r.id == "a").unwrap().status,
        "completed"
    );
    assert_eq!(rows.iter().find(|r| r.id == "b").unwrap().status, "running");
    let filtered = live
        .list_audits(&RequestAuditFilters {
            stream: Some(true),
            ..Default::default()
        })
        .await
        .unwrap();
    assert_eq!(filtered.len(), 1);
    assert_eq!(filtered[0].total_tokens, Some(1500));
    let summary = crate::db::request_audits::summarize_request_audit_rows(&rows);
    assert_eq!((summary.completed_count, summary.running_count), (1, 1));
    db.close().await;
    let rows = live
        .list_audits(&RequestAuditFilters::default())
        .await
        .unwrap();
    assert_eq!(
        rows.iter().find(|r| r.id == "b").unwrap().status,
        "cancelled"
    );
    live.close().await;
    let reopened = LocalRuntime::open(&root).await.unwrap();
    assert_eq!(
        reopened
            .list_audits(&RequestAuditFilters::default())
            .await
            .unwrap()
            .len(),
        2
    );
    assert!(reopened
        .list_audits(&RequestAuditFilters {
            created_from: Some("bad".into()),
            ..Default::default()
        })
        .await
        .is_err());
    reopened.close().await;
    crate::local_runtime::test_support::remove_test_root(&root).await;
}

#[tokio::test]
async fn local_cost_uses_persisted_tokens_and_shared_price_rules() {
    let (root, db) = setup().await;
    db.create_audit(create_input("a", false)).await.unwrap();
    db.finalize_audit("a", finish_input()).await.unwrap();
    let identities = vec![GatewayRuntimeProviderIdentity {
        id: "provider-a".into(),
        label: "A".into(),
        status: "active".into(),
        adapter: "openai_compatible".into(),
        protocol_family: "openai".into(),
        supported_models: vec!["unpriced-test-model".into()],
    }];
    let report = operator::get_local_cost_overview(&db, &identities)
        .await
        .unwrap();
    assert_eq!(report.summary.total_requests, 1);
    assert_eq!(report.summary.total_tokens, 1500);
    assert_eq!(report.summary.estimated_market_cost_micros, None);
    assert!(!report.provider_buckets[0].models[0].market_rate.configured);
    sqlx::query("INSERT INTO provider_pricing(provider_id,payload) VALUES (?,?)")
        .bind("provider-a")
        .bind(
            serde_json::json!({"modelPricing": {"unpriced-test-model": {
                "staticInputMicrosPer1kTokens": 2000, "staticOutputMicrosPer1kTokens": 6000
            }}})
            .to_string(),
        )
        .execute(&db.pool)
        .await
        .unwrap();
    db.close().await;
    let db = LocalRuntime::open(&root).await.unwrap();
    let report = operator::get_local_cost_overview(&db, &identities)
        .await
        .unwrap();
    assert_eq!(report.summary.estimated_market_cost_micros, Some(5000));
    assert_eq!(report.pricing_editors.len(), 1);
    db.close().await;
    crate::local_runtime::test_support::remove_test_root(&root).await;
}

#[tokio::test]
async fn local_crash_recovery_only_retires_stale_instance() {
    let (root, db) = setup().await;
    let stale = LocalRuntime::open(&root).await.unwrap();
    db.create_audit(create_input("live", false)).await.unwrap();
    stale
        .create_audit(create_input("stale", true))
        .await
        .unwrap();
    sqlx::query("UPDATE runtime_instances SET heartbeat = 0 WHERE id = ?")
        .bind(&stale.owner)
        .execute(&db.pool)
        .await
        .unwrap();
    db.maintain().await.unwrap();
    let rows = db
        .list_audits(&RequestAuditFilters::default())
        .await
        .unwrap();
    assert_eq!(
        rows.iter().find(|r| r.id == "live").unwrap().status,
        "running"
    );
    assert_eq!(
        rows.iter().find(|r| r.id == "stale").unwrap().status,
        "cancelled"
    );
    stale.close().await;
    db.close().await;
    crate::local_runtime::test_support::remove_test_root(&root).await;
}

#[tokio::test]
async fn local_model_health_persists_failure_and_success() {
    let (root, db) = setup().await;
    let input = RecordCredentialModelSuccessInput {
        provider_account_id: "provider-a".into(),
        provider_credential_id: Some("credential-a".into()),
        provider_credential_ref: None,
        protocol_profile: Some("openai".into()),
        model: "test-model".into(),
    };
    db.model_success(input.clone()).await.unwrap();
    let failure = RecordCredentialModelFailureInput {
        provider_account_id: input.provider_account_id.clone(),
        provider_credential_id: input.provider_credential_id.clone(),
        provider_credential_ref: None,
        protocol_profile: input.protocol_profile.clone(),
        model: input.model.clone(),
        upstream_status: Some(429),
        error_message: Some("rate limit; token=private-test-token".into()),
        classification: crate::provider_failure::classify_provider_failure(
            Some(429),
            None,
            Some("rate limit"),
        ),
    };
    db.model_failure(failure).await.unwrap();
    db.close().await;
    let db = LocalRuntime::open(&root).await.unwrap();
    let states = db
        .list_model_states(CredentialModelStateFilters::default())
        .await
        .unwrap();
    assert_eq!(states.len(), 1);
    assert_eq!(states[0].failure_count, 1);
    assert!(!states[0]
        .last_error
        .as_deref()
        .unwrap_or("")
        .contains("private-test-token"));
    db.model_success(input).await.unwrap();
    let states = db
        .list_model_states(CredentialModelStateFilters::default())
        .await
        .unwrap();
    assert_eq!(states[0].failure_count, 0);
    assert_eq!(states[0].status, "active");
    db.close().await;
    crate::local_runtime::test_support::remove_test_root(&root).await;
}
