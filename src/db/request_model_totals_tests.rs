use super::*;
use crate::db::{CreateRequestAuditInput, RequestAuditFilters};
use serde_json::json;

fn input(
    id: &str,
    provider: &str,
    credential: Option<serde_json::Value>,
) -> CreateRequestAuditInput {
    CreateRequestAuditInput {
        project_id: "test".into(),
        api_key_id: None,
        user_credential_id: None,
        access_key_id: None,
        source_access_key_id: None,
        session_id: None,
        route_policy_id: None,
        provider_account_id: Some(provider.into()),
        protocol_family: "openai".into(),
        endpoint_kind: "chat.completions".into(),
        requested_model: Some(" requested-model ".into()),
        resolved_model: Some(" model-a ".into()),
        model_alias: Some("alias".into()),
        stream: false,
        route_attempt_count: 1,
        response_id: id.into(),
        previous_response_id: None,
        route_trace: credential.map(|value| json!({"realCredentialRef": value})),
    }
}

#[tokio::test]
async fn retained_model_totals_are_unsampled_attributed_and_durable() {
    let root = std::env::temp_dir().join(format!(
        "gateway-local-model-totals-{}",
        uuid::Uuid::new_v4()
    ));
    let db = LocalRuntime::open(&root).await.unwrap();
    assert!(load_local_retained_model_totals(&db)
        .await
        .unwrap()
        .is_empty());
    db.create_audit(input("base", "pool-a", Some(json!(" credential-a "))))
        .await
        .unwrap();
    for (status, count) in [("completed", 1100), ("failed", 100), ("cancelled", 1)] {
        sqlx::query(
            "WITH RECURSIVE n(i) AS (SELECT 1 UNION ALL SELECT i + 1 FROM n WHERE i < ?)
             INSERT INTO request_audits (id,payload,created,status,owner)
             SELECT ? || i, json_set(payload, '$.id', ? || i, '$.status', ?), created, ?, owner
             FROM n, request_audits WHERE id = 'base'",
        )
        .bind(count)
        .bind(status)
        .bind(status)
        .bind(status)
        .bind(status)
        .execute(&db.pool)
        .await
        .unwrap();
    }
    // Different credentials/providers and unattributed historical calls stay separate.
    for (id, provider, credential) in [
        ("b", "pool-a", Some(json!("credential-b"))),
        ("other", "pool-b", Some(json!("credential-a"))),
        ("legacy", "pool-a", None),
        ("invalid", "pool-a", Some(json!(123))),
        ("blank", "pool-a", Some(json!(" "))),
    ] {
        db.create_audit(input(id, provider, credential))
            .await
            .unwrap();
    }
    let rows = load_local_retained_model_totals(&db).await.unwrap();
    assert_eq!(rows.len(), 4);
    let a = rows
        .iter()
        .find(|row| {
            row.provider_account_id == "pool-a"
                && row.credential_ref.as_deref() == Some("credential-a")
        })
        .unwrap();
    assert_eq!(
        (a.model.as_str(), a.request_count, a.success_count),
        ("model-a", 1202, 1100)
    );
    let legacy = rows
        .iter()
        .find(|row| row.credential_ref.is_none())
        .unwrap();
    assert_eq!((legacy.request_count, legacy.success_count), (3, 0));
    let recent = db
        .list_audits(&RequestAuditFilters {
            limit: Some(1000),
            ..Default::default()
        })
        .await
        .unwrap();
    assert_eq!(recent.len(), 1000);
    let version: i64 = sqlx::query_scalar("PRAGMA user_version")
        .fetch_one(&db.pool)
        .await
        .unwrap();
    assert_eq!(version, 1);
    db.close().await;
    let reopened = LocalRuntime::open(&root).await.unwrap();
    assert_eq!(
        serde_json::to_value(&rows).unwrap(),
        serde_json::to_value(load_local_retained_model_totals(&reopened).await.unwrap()).unwrap()
    );
    reopened.close().await;
    crate::local_runtime::test_support::remove_test_root(&root).await;
}

#[tokio::test]
async fn retained_model_totals_follow_model_fallback_and_keep_old_summary_compatible() {
    let root = std::env::temp_dir().join(format!(
        "gateway-local-model-totals-{}",
        uuid::Uuid::new_v4()
    ));
    let db = LocalRuntime::open(&root).await.unwrap();
    for (id, resolved, requested, expected) in [
        (
            "requested",
            Some(" "),
            Some(" requested-model "),
            "requested-model",
        ),
        ("alias", None, Some(""), "alias"),
        ("unknown", None, None, "unknown"),
    ] {
        let mut audit = input(id, "pool-a", None);
        audit.resolved_model = resolved.map(str::to_owned);
        audit.requested_model = requested.map(str::to_owned);
        if id == "unknown" {
            audit.model_alias = None;
        }
        db.create_audit(audit).await.unwrap();
        let rows = load_local_retained_model_totals(&db).await.unwrap();
        assert!(rows
            .iter()
            .any(|row| row.model == expected && row.request_count == 1 && row.success_count == 0));
    }
    db.create_audit(input("empty-provider", " ", None))
        .await
        .unwrap();
    let totals = load_local_retained_model_totals(&db).await.unwrap();
    assert_eq!(totals.len(), 3);
    let rows = db
        .list_audits(&RequestAuditFilters::default())
        .await
        .unwrap();
    let view = |retained_model_totals| SummaryWithModelTotals {
        summary: crate::db::request_audits::summarize_request_audit_rows(&rows),
        retained_model_totals,
    };
    let old = serde_json::to_value(view(None)).unwrap();
    assert!(old.get("retainedModelTotals").is_none());
    let new = serde_json::to_value(view(Some(totals))).unwrap();
    assert_eq!(new["totalRequests"], old["totalRequests"]);
    assert_eq!(new["retainedModelTotals"].as_array().unwrap().len(), 3);
    db.close().await;
    crate::local_runtime::test_support::remove_test_root(&root).await;
}
