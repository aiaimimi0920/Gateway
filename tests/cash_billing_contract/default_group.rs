use super::fixture::{self, Fixture, KEYS};
use axum::http::StatusCode;
use serde_json::json;

#[tokio::test]
async fn implicit_default_group_authorizes_and_bills_without_a_document_migration() {
    let mut f = Fixture::new().await;
    f.document.account_groups.clear();
    f.state
        .route_config
        .replace_document(f.document.clone())
        .unwrap();
    f.price(1000, 2000, "0.8").await;
    let mut input = fixture::key_input(100_000);
    input["metadata"]["accountGroupIds"] = json!(["default"]);
    let (status, _, key) = f.manage(KEYS, Some(input)).await;
    assert_eq!(status, StatusCode::OK, "{key}");
    let (status, body) =
        fixture::relay(&f.state, &key, fixture::request(false), Some("default")).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let ledger = f.finalized(&key, 1).await;
    assert_eq!(ledger[0]["settledQuote"]["groupId"], "default");
    assert_eq!(ledger[0]["amountMicros"], 160);
    f.finish().await;
}
