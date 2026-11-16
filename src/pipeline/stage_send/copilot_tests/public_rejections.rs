//! Public rejection paths reuse saved management identity without internal attempt construction.
use super::*;
use crate::access_balance::{quota::KeyQuotaInput, AccessBalanceStore};

#[tokio::test]
async fn copilot_public_cash_rejects_without_reservation_or_attempt() {
    let f = saved().await;
    f.local
        .save_access_key_with_quota(
            Some(&f.key),
            fixture::input(json!({
                "scopes":["relay"],"models":["public-alias"],"providerIds":["copilot"]
            })),
            Some(&KeyQuotaInput {
                mode: "cash_prepaid".into(),
                limit: Some(1_000_000),
                currency: Some("USD".into()),
            }),
        )
        .await
        .unwrap();
    let store = AccessBalanceStore::Sqlite(&f.local);
    let before = serde_json::to_value(store.cash_balance(&f.key).await.unwrap()).unwrap();
    for limit in [false, true] {
        let mut body = json!({"model":"public-alias","input":"ok"});
        if limit {
            body["max_output_tokens"] = json!(16);
        }
        let (status, value) = call(&f, "/v1/responses", body).await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "{value}");
        assert_eq!(
            value["error"]["code"],
            if limit {
                "cash_adapter_unsupported"
            } else {
                "cash_output_limit_required"
            }
        );
    }
    assert_eq!(
        serde_json::to_value(store.cash_balance(&f.key).await.unwrap()).unwrap(),
        before
    );
    assert!(store.cash_ledger(&f.key).await.unwrap().is_empty());
    let rows = f
        .local
        .list_audits(&crate::db::RequestAuditFilters::default())
        .await
        .unwrap();
    assert_eq!(rows.len(), 2);
    for row in rows {
        assert_eq!(row.status, "failed");
        assert_eq!(row.route_trace.as_ref().unwrap()["routeAttemptCount"], 0);
        assert!(row.upstream_status.is_none() && row.total_tokens.is_none());
    }
    finish(f).await;
}

#[tokio::test]
#[ignore = "requires nonexistent explicit absolute CLI and fresh probe/workspace roots"]
async fn copilot_public_missing_cli_refunds_and_never_falls_back() {
    let path = std::path::PathBuf::from(std::env::var_os("GATEWAY_COPILOT_CLI").unwrap());
    assert!(path.is_absolute() && !path.exists());
    let f = saved().await;
    for (path, body) in [
        (
            "/v1/chat/completions",
            json!({"model":"public-alias","messages":[{"role":"user","content":"ok"}]}),
        ),
        (
            "/v1/responses",
            json!({"model":"public-alias","input":"ok"}),
        ),
    ] {
        let (status, value) = call(&f, path, body).await;
        assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE, "{value}");
        assert_eq!(value["error"]["code"], "copilot_execution_unavailable");
        assert!(!value.to_string().contains("synthetic-token"));
    }
    assert_eq!(
        f.local
            .access_balance(&f.key)
            .await
            .unwrap()
            .unwrap()
            .remaining_messages,
        Some(2)
    );
    let rows = f
        .local
        .list_audits(&crate::db::RequestAuditFilters::default())
        .await
        .unwrap();
    assert_eq!(rows.len(), 2);
    for row in rows {
        assert_eq!(row.status, "failed");
        assert_eq!(row.route_trace.as_ref().unwrap()["routeAttemptCount"], 1);
        assert!(row.upstream_status.is_none() && row.total_tokens.is_none());
    }
    assert_eq!(
        std::fs::read_dir(std::env::var_os("GATEWAY_COPILOT_WORKSPACE_ROOT").unwrap())
            .unwrap()
            .count(),
        0
    );
    finish(f).await;
}
