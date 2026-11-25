use super::*;
use axum::{extract::State, http::HeaderMap, routing::post, Json, Router};
use std::sync::atomic::{AtomicUsize, Ordering};

#[test]
fn cloudflare_usage_missing_data_is_not_zero() {
    for invalid in [
        r#"{"data":{"viewer":{"accounts":[]}}}"#,
        r#"{"data":{"viewer":{"accounts":[{}]}}}"#,
        r#"{"data":{"viewer":{"accounts":[{"total":[{"sum":{}}]}]}}}"#,
        r#"{"errors":[{"message":"denied"}],"data":{"viewer":{"accounts":[{"total":[]}]}}}"#,
        r#"{"errors":{},"data":{"viewer":{"accounts":[{"total":[]}]}}}"#,
        r#"{"data":{"viewer":{"accounts":[{"total":[{"sum":{"totalNeurons":-1}}]}]}}}"#,
    ] {
        assert_eq!(parse_usage(invalid.as_bytes()), None);
    }
    assert_eq!(
        parse_usage(br#"{"data":{"viewer":{"accounts":[{"total":[]}]}}}"#),
        Some(0.0)
    );
    assert_eq!(
        parse_usage(br#"{"errors":null,"data":{"viewer":{"accounts":[{"total":[]}]}}}"#),
        Some(0.0)
    );
}

#[test]
fn cloudflare_usage_threshold_stays_paused_until_utc_reset() {
    let at = |value| OffsetDateTime::parse(value, &Rfc3339).unwrap();
    let start = at("2026-11-24T23:00:00Z");
    let paused = Snapshot {
        checked_at: start,
        usage: Some(9_000.0),
    };
    let cached = paused.cached_decision(at("2026-11-24T23:59:59Z")).unwrap();
    let error = cached.unwrap_err();
    assert_eq!(
        error.code.as_deref(),
        Some("cloudflare_neuron_budget_paused")
    );
    assert!(local_denial(&error));
    assert!(!error.retryable);
    assert_eq!(error.upstream_retry_after_ms, Some(1_000));
    assert!(matches!(
        error.fallback_hint,
        crate::error::FallbackHint::FallbackProvider { .. }
    ));
    assert!(paused.cached_decision(at("2026-11-25T00:00:00Z")).is_none());

    let available = Snapshot {
        usage: Some(8_999.9),
        ..paused
    };
    assert!(available.cached_decision(start).unwrap().is_ok());
    assert!(available.cached_decision(start + TTL).is_none());
}

#[tokio::test]
async fn cloudflare_usage_account_cache_shares_threshold_across_tokens() {
    let calls = Arc::new(AtomicUsize::new(0));
    let router = Router::new()
        .route("/graphql", post(
            |State(calls): State<Arc<AtomicUsize>>, headers: HeaderMap, Json(body): Json<serde_json::Value>| async move {
                calls.fetch_add(1, Ordering::Relaxed);
                assert_eq!(headers["authorization"], "Bearer synthetic");
                let vars = &body["variables"];
                let start = OffsetDateTime::parse(vars["start"].as_str().unwrap(), &Rfc3339).unwrap();
                let end = OffsetDateTime::parse(vars["end"].as_str().unwrap(), &Rfc3339).unwrap();
                assert_eq!(start, end.date().midnight().assume_utc());
                let account = vars["account"].as_str().unwrap();
                let usage = match account {
                    "one" => 9_000,
                    "two" => 0,
                    _ => return Json(serde_json::json!({"errors":[{"message":"denied"}]})),
                };
                Json(serde_json::json!({"errors":null,"data":{"viewer":{"accounts":[{"total":[{"sum":{"totalNeurons":usage}}]}]}}}))
            },
        ))
        .with_state(calls.clone());
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("http://{}/graphql", listener.local_addr().unwrap());
    let server = tokio::spawn(async move { axum::serve(listener, router).await.unwrap() });
    let mut check = Check {
        guard: Arc::default(),
        http: crate::http_client::builder().no_proxy().build().unwrap(),
        account: "one".into(),
        token: "synthetic".into(),
        fixture_url: Some(url),
    };
    // A waiter owns this empty slot even before it acquires the account lock.
    let held = Arc::new(Mutex::new(None));
    {
        let mut cells = check.guard.accounts.lock().await;
        cells.insert("held".into(), held.clone());
        for n in 1..1024 {
            cells.insert(n.to_string(), Default::default());
        }
    }
    assert_eq!(
        check.admit().await.unwrap_err().code.as_deref(),
        Some("cloudflare_neuron_budget_paused")
    );
    check.token = "different".into();
    assert!(Arc::ptr_eq(
        &check.guard.accounts.lock().await["held"],
        &held
    ));
    assert_eq!(
        check.admit().await.unwrap_err().code.as_deref(),
        Some("cloudflare_neuron_budget_paused")
    );
    check.account = "two".into();
    check.token = "synthetic".into();
    assert!(check.admit().await.is_ok());
    assert!(check.admit().await.is_ok());
    check.account = "three".into();
    assert_eq!(
        check.admit().await.unwrap_err().code.as_deref(),
        Some("cloudflare_neuron_usage_unavailable")
    );
    assert_eq!(calls.load(Ordering::Relaxed), 3);
    server.abort();
}
