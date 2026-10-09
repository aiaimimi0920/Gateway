use super::fixture::{self, Fixture, PRICING};
use axum::http::StatusCode;
use serde_json::json;
use std::sync::atomic::Ordering;

#[tokio::test]
async fn buffered_and_sse_use_actual_usage_lowest_authorized_or_explicit_group() {
    let f = Fixture::new().await;
    f.price(1000, 2000, "0.8").await;
    let key = f.key(100_000).await;
    let mut request = fixture::request(false);
    request["model"] = json!("cash-alias");
    let (status, body) = fixture::relay(&f.state, &key, request, None).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let first = f.finalized(&key, 1).await;
    assert_eq!(first[0]["amountMicros"], 240);
    assert_eq!(first[0]["settledQuote"]["groupId"], "cheap");
    assert_eq!(first[0]["settledQuote"]["model"], "cash-model");
    assert_eq!(first[0]["settledQuote"]["credentialId"], "account-a");
    let (status, body) = fixture::relay(&f.state, &key, fixture::request(true), Some("dear")).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert!(body.as_str().unwrap().contains("[DONE]"));
    let ledger = f.finalized(&key, 2).await;
    assert_eq!(ledger[0]["amountMicros"], 320);
    assert_eq!(ledger[0]["settledQuote"]["groupId"], "dear");
    let sum: i64 = ledger
        .as_array()
        .unwrap()
        .iter()
        .map(|row| row["amountMicros"].as_i64().unwrap())
        .sum();
    let balance = f.balance(&key).await;
    assert_eq!(sum, 560);
    assert_eq!(balance["spentMicros"], sum);
    assert_eq!(balance["reservedMicros"], 0);
    assert_eq!(balance["pendingRequests"], 0);
    let (_, _, catalog) = f.manage("/v1/internal/gateway/access/catalog", None).await;
    assert_eq!(catalog["cashQuotaSupported"], true);
    assert_eq!(catalog["balances"][0]["cash"], balance);
    f.finish().await;
}

#[tokio::test]
async fn changing_all_three_tariff_layers_cannot_reprice_a_reserved_request() {
    let mut f = Fixture::new().await;
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
    assert!(reserved > 240);
    f.price(9000, 18000, "0.5").await;
    f.document.account_groups[0].billing_multiplier = Some(3.0);
    f.document.account_groups[1].billing_multiplier = Some(4.0);
    f.state
        .route_config
        .replace_document(f.document.clone())
        .unwrap();
    f.control.block.store(false, Ordering::SeqCst);
    f.control.permits.add_permits(1);
    assert_eq!(first.await.unwrap().0, StatusCode::OK);
    let ledger = f.finalized(&key, 1).await;
    assert_eq!(ledger[0]["amountMicros"], 240);
    assert_eq!(ledger[0]["reservedMicros"], reserved);
    let (status, body) = fixture::relay(&f.state, &key, fixture::request(false), None).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let ledger = f.finalized(&key, 2).await;
    assert_eq!(ledger[0]["amountMicros"], 2700);
    assert_eq!(f.balance(&key).await["spentMicros"], 2940);
    f.finish().await;
}

#[tokio::test]
async fn anthropic_cache_and_cross_protocol_stream_bill_input_once() {
    let f = Fixture::with_adapter("anthropic_compatible").await;
    f.price(1000, 2000, "0.8").await;
    let key = f.key(100_000).await;
    for stream in [false, true] {
        let (status, body) = fixture::relay(&f.state, &key, fixture::request(stream), None).await;
        assert_eq!(status, StatusCode::OK, "{body}");
    }
    let ledger = f.finalized(&key, 2).await;
    for row in ledger.as_array().unwrap() {
        assert_eq!(row["amountMicros"], 240, "{row}");
        assert_eq!(row["settledQuote"]["cacheTokensSeparate"], true);
        assert_eq!(row["usage"]["cache_read_input_tokens"], 20);
    }
    assert_eq!(f.balance(&key).await["spentMicros"], 480);
    f.finish().await;
}

#[tokio::test]
async fn selected_account_not_card_summary_determines_the_account_multiplier() {
    let f = Fixture::new().await;
    let mut document = serde_json::to_value(&f.document).unwrap();
    document["providers"][0]["supported_models"]
        .as_array_mut()
        .unwrap()
        .push(json!("account-b-model"));
    document["providers"][0]["credentials"][0]["supported_models"] = json!(["cash-model"]);
    document["providers"][0]["credentials"].as_array_mut().unwrap().push(json!({
        "id":"account-b","api_key":"second-fixture-secret","supported_models":["account-b-model"]}));
    document["account_groups"][0]["provider_credential_ids"] = json!(["account-a", "account-b"]);
    f.state
        .route_config
        .replace_document(serde_json::from_value(document).unwrap())
        .unwrap();
    let (status, _, body) = f
        .manage(
            PRICING,
            Some(json!({"entries":[{"model":"account-b-model",
        "promptMicrosPer1kTokens":1000,"completionMicrosPer1kTokens":2000}],
        "accountBillingMultipliers":{"account-a":"100","account-b":"0.4"}})),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let key = f.key(100_000).await;
    let mut request = fixture::request(false);
    request["model"] = json!("account-b-model");
    let (status, body) = fixture::relay(&f.state, &key, request, None).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let ledger = f.finalized(&key, 1).await;
    assert_eq!(ledger[0]["amountMicros"], 120);
    assert_eq!(ledger[0]["settledQuote"]["credentialId"], "account-b");
    f.finish().await;
}
