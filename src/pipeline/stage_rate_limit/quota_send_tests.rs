//! Run the same Pending::send seam used by both JSON and SSE against a loopback 402.
use super::*;
use crate::{
    pipeline::{request_budget::RequestBudget, stage_rate_limit::attempt_budget::tests::gate},
    preset::{compile_provider_account, get_builtin_preset, AccountOverrides},
    protocol::canonical::EndpointKind,
    retry::RetryPolicy,
    upstream::{common::RequestPlan, profiled_request::Pending, vercel_credit_guard::Check},
};
use axum::{http::StatusCode, routing::post, Router};
use serde_json::json;
use std::{sync::atomic::AtomicUsize, time::Duration};

#[tokio::test]
async fn quota_send_counts_real_402_but_local_pause_leaves_single_attempt_for_backup() {
    let calls = Arc::new(AtomicUsize::default());
    let upstream = calls.clone();
    let router = Router::new().route("/", post(move || {
        let calls = upstream.clone();
        async move {
            calls.fetch_add(1, Ordering::Relaxed);
            (StatusCode::PAYMENT_REQUIRED, r#"{"error":{"type":"insufficient_credits","message":"Insufficient balance"}}"#)
        }
    })).route("/backup", post(|| async { StatusCode::OK }));
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("http://{}/", listener.local_addr().unwrap());
    let server = tokio::spawn(async move { axum::serve(listener, router).await.unwrap() });
    let account: AccountOverrides = serde_json::from_value(json!({
        "base_url": url,
        "api_key": format!("fixture-{}", uuid::Uuid::new_v4()),
        "auth_mode": "vercel_monthly_credit_key",
    }))
    .unwrap();
    let mut payload =
        compile_provider_account(&get_builtin_preset("vercel-ai-gateway").unwrap(), &account);
    payload.credential_id = Some("fixture".into());
    let plan = RequestPlan {
        method: rquest::Method::POST,
        url: url.clone(),
        query: vec![],
        body: Some(json!({"model":"openai/gpt-5-mini"})),
        response_kind: EndpointKind::ChatCompletions,
    };
    let http = crate::http_client::builder().no_proxy().build().unwrap();
    let send = || async {
        Pending::new(http.post(&url), "fixture", false, Duration::from_secs(3))?
            .with_vercel(Check::for_plan(&http, &payload, &plan)?)
            .send()
            .await
    };
    let mut first = gate();
    first.defer_until_quota = guarded(&payload);
    let observed = AtomicUsize::default();
    let policy = RetryPolicy::default();
    let error = first
        .execute_observed(
            send,
            |o| {
                assert!(!o.succeeded);
                observed.fetch_add(1, Ordering::Relaxed);
            },
            &policy,
        )
        .await
        .unwrap_err();
    assert_eq!(
        error.code.as_deref(),
        Some("vercel_monthly_credit_exhausted")
    );
    assert!(crate::error::is_explicit_admission_rejection(&error));
    assert_eq!(first.request_budget.attempts(), 1);
    let mut next = gate();
    next.defer_until_quota = guarded(&payload);
    next.request_budget = RequestBudget::new(1, Duration::from_secs(5));
    let error = next
        .execute_observed(send, |_| panic!("local refusal is not a send"), &policy)
        .await
        .unwrap_err();
    assert_eq!(error.code.as_deref(), Some("vercel_monthly_credit_paused"));
    assert!(!crate::error::is_explicit_admission_rejection(&error));
    assert_eq!(next.request_budget.attempts(), 0);
    assert!(next.attempted_provider_ids.lock().is_empty());
    assert_eq!(calls.load(Ordering::Relaxed), 1);
    next.defer_until_quota = false;
    next.provider_account_id = "authorized-backup".into();
    next.execute_observed(
        || async {
            http.post(format!("{url}backup"))
                .send()
                .await
                .and_then(|r| r.error_for_status())
                .map(|_| ())
                .map_err(|e| GatewayError::server_error(e.to_string()))
        },
        |_| {
            observed.fetch_add(1, Ordering::Relaxed);
        },
        &policy,
    )
    .await
    .unwrap();
    assert_eq!(
        next.attempted_provider_ids.lock().as_slice(),
        ["authorized-backup"]
    );
    assert_eq!(next.request_budget.attempts(), 1);
    assert_eq!(observed.load(Ordering::Relaxed), 2);
    assert_eq!(calls.load(Ordering::Relaxed), 1);
    server.abort();
}
