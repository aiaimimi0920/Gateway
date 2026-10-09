//! Management UI reads use a narrow projection and retain partial-write semantics.
use super::fixture::{self, Fixture, PRICING};
use axum::http::StatusCode;
use serde_json::json;

#[tokio::test]
async fn pricing_read_is_authorized_redacted_and_preserves_partial_updates() {
    let f = Fixture::new().await;
    let key = f.key(100_000).await;
    for token in ["", "invalid", key["token"].as_str().unwrap()] {
        let (status, _, _) = fixture::api(&f.state, PRICING, token, None, None).await;
        assert!(!status.is_success());
    }
    let (status, headers, body) = f.manage(PRICING, None).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(headers["cache-control"], "no-store");
    assert_eq!(body, json!({"accountBillingMultipliers":{}}));
    f.price(1000, 2000, "8e-1").await;
    let (_, _, body) = f.manage(PRICING, None).await;
    assert_eq!(
        body,
        json!({"accountBillingMultipliers":{"account-a":"0.800000"}})
    );
    assert!(!body.to_string().contains("secret"));
    let (status, _, _) = f
        .manage(
            PRICING,
            Some(json!({
                "accountBillingMultipliers":{"account-b":"0", "account-a":null}
            })),
        )
        .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        f.manage(PRICING, None).await.2,
        json!({"accountBillingMultipliers":{"account-b":"0.000000"}})
    );
    let (status, _) = fixture::relay(&f.state, &key, fixture::request(false), None).await;
    assert_eq!(status, StatusCode::OK);
    // Model pricing survived the multiplier-only patch; the removed override is default 1.
    assert_eq!(f.finalized(&key, 1).await[0]["amountMicros"], 300);
    assert_eq!(
        f.manage(&PRICING.replace("cash-fixture", "unknown"), None)
            .await
            .0,
        StatusCode::NOT_FOUND
    );
    f.finish().await;
}
