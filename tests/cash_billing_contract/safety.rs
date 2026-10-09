use super::{
    fixture::{self, Fixture, KEYS, PRICING},
    support,
};
use axum::http::StatusCode;
use serde_json::{json, Value};
use std::sync::atomic::Ordering;

#[test]
fn fixture_setup_future_is_boxed_on_default_windows_test_stacks() {
    assert!(std::mem::size_of_val(&Fixture::new()) <= 32);
}

#[tokio::test]
async fn changing_quota_units_between_checks_cannot_bypass_cash_admission() {
    let f = Fixture::new().await;
    let key = f.key(100_000).await;
    let mut input = fixture::key_input(100_000);
    input["quota"] = json!({"mode":"unlimited"});
    assert_eq!(
        f.manage(
            &format!("{KEYS}/{}", key["id"].as_str().unwrap()),
            Some(input)
        )
        .await
        .0,
        StatusCode::OK
    );
    let canonical =
        neuro_gateway::protocol::openai::normalize_chat_completions(fixture::request(false))
            .unwrap();
    let mut context = neuro_gateway::pipeline::PipelineContext::new(canonical, None);
    context.requesting_access_key_id = Some(key["id"].as_str().unwrap().into());
    context.admitted_access_balance_mode = Some("cash_prepaid".into());
    let error = neuro_gateway::cash_billing::admission::prepare(&mut context, &f.state)
        .await
        .unwrap_err();
    assert_eq!(error.code.as_deref(), Some("key_quota_changed"));
    assert_eq!(f.control.calls.load(Ordering::SeqCst), 0);
    f.finish().await;
}

#[tokio::test]
async fn missing_tariffs_unknown_bounds_and_exhaustion_are_rejected_before_upstream() {
    let f = Fixture::new().await;
    let key = f.key(100_000).await;
    for model in ["missing", "gpt-5.4"] {
        let mut request = fixture::request(false);
        request["model"] = json!(model);
        let (status, error) = fixture::relay(&f.state, &key, request, None).await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "{error}");
        assert_eq!(error["error"]["code"], "cash_pricing_unavailable");
    }
    f.price(1000, 2000, "0.8").await;
    for variant in 0..5 {
        let mut request = fixture::request(false);
        match variant {
            0 => {
                request.as_object_mut().unwrap().remove("max_tokens");
            }
            1 => request["n"] = json!(2),
            2 => request["max_tokens"] = json!(1_000_001),
            3 => {
                request["messages"][0]["content"] = json!([{"type":"image_url","image_url":{"url":"https://example.invalid/a.png"}}])
            }
            _ => request["max_tokens"] = json!(-1),
        }
        let (status, error) = fixture::relay(&f.state, &key, request, None).await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "{error}");
    }
    f.limit(&key, 1).await;
    assert_eq!(
        fixture::relay(&f.state, &key, fixture::request(false), None)
            .await
            .0,
        StatusCode::TOO_MANY_REQUESTS
    );
    for (group, code) in [
        ("unauthorized", "cash_pricing_unavailable"),
        ("disabled", "account_group_disabled"),
    ] {
        let (status, error) =
            fixture::relay(&f.state, &key, fixture::request(false), Some(group)).await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "{group}: {error}");
        assert_eq!(error["error"]["code"], code, "{group}: {error}");
    }
    assert_eq!(f.control.calls.load(Ordering::SeqCst), 0);
    assert_eq!(f.ledger(&key).await, json!([]));
    f.finish().await;
}

#[tokio::test]
async fn concurrent_http_admissions_do_not_spend_reserved_cash_again() {
    let f = Fixture::new().await;
    f.price(1000, 2000, "0.8").await;
    let key = f.key(100_000).await;
    f.control.block.store(true, Ordering::SeqCst);
    let state = f.state.clone();
    let original_key = key.clone();
    let first = tokio::spawn(async move {
        fixture::relay(&state, &original_key, fixture::request(false), None).await
    });
    f.calls(1).await;
    let reserved = f.balance(&key).await["reservedMicros"].as_i64().unwrap();
    f.limit(&key, reserved).await;
    let responses = futures::future::join_all(
        (0..8).map(|_| fixture::relay(&f.state, &key, fixture::request(false), None)),
    )
    .await;
    assert!(
        responses
            .iter()
            .all(|(status, _)| *status == StatusCode::TOO_MANY_REQUESTS),
        "{responses:?}"
    );
    assert_eq!(f.control.calls.load(Ordering::SeqCst), 1);
    f.control.permits.add_permits(1);
    assert_eq!(first.await.unwrap().0, StatusCode::OK);
    f.finalized(&key, 1).await;
    let balance = f.balance(&key).await;
    assert_eq!(balance["spentMicros"], 240);
    assert_eq!(balance["reservedMicros"], 0);
    f.finish().await;
}

#[tokio::test]
async fn malformed_or_missing_upstream_usage_is_pending_not_a_free_invoice() {
    let f = Fixture::new().await;
    f.price(1000, 2000, "0.8").await;
    let key = f.key(100_000).await;
    for usage in [
        None,
        Some(json!({})),
        Some(json!({"prompt_tokens":100})),
        Some(json!({"prompt_tokens":-1,"completion_tokens":50})),
    ] {
        *f.control.usage.lock().unwrap() = usage;
        let (status, body) = fixture::relay(&f.state, &key, fixture::request(false), None).await;
        assert_eq!(status, StatusCode::OK, "{body}");
    }
    let ledger = f.finalized(&key, 4).await;
    assert!(ledger
        .as_array()
        .unwrap()
        .iter()
        .all(|row| row["status"] == "unresolved" && row["amountMicros"].is_null()));
    let balance = f.balance(&key).await;
    assert_eq!(balance["spentMicros"], 0);
    assert_eq!(balance["pendingRequests"], 4);
    assert!(balance["reservedMicros"].as_i64().unwrap() > 0);
    f.finish().await;
}

#[tokio::test]
async fn ledger_is_management_only_and_bad_prices_do_not_mutate_the_previous_tariff() {
    let f = Fixture::new().await;
    f.price(1000, 2000, "0.8").await;
    let key = f.key(100_000).await;
    let path = format!("{KEYS}/{}/cash-ledger", key["id"].as_str().unwrap());
    for token in ["", "invalid", key["token"].as_str().unwrap()] {
        let (status, _, body) = fixture::api(&f.state, &path, token, None, None).await;
        assert!(!status.is_success(), "{body}");
    }
    for bad in [
        json!({"entries":[{"model":"cash-model","promptMicrosPer1kTokens":-1}]}),
        json!({"accountBillingMultipliers":{"account-a":"0.0000001"}}),
        json!({"accountBillingMultipliers":{"account-a":-1}}),
    ] {
        let (status, _, body) = f.manage(PRICING, Some(bad)).await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "{body}");
    }
    assert_eq!(
        fixture::relay(&f.state, &key, fixture::request(false), None)
            .await
            .0,
        StatusCode::OK
    );
    assert_eq!(f.finalized(&key, 1).await[0]["amountMicros"], 240);
    let (_, _, catalog) = fixture::api(
        &f.state,
        "/v1/internal/gateway/access/catalog",
        support::MANAGEMENT_TOKEN,
        None,
        None,
    )
    .await;
    assert!(catalog["balances"][0]["remainingTokens"].is_null());
    assert!(catalog["balances"][0]["remainingMessages"].is_null());
    assert_ne!(f.balance(&key).await, Value::Null);
    f.finish().await;
}
