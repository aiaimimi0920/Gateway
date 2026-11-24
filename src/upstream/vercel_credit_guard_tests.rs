use super::*;
use serde_json::json;

#[tokio::test]
async fn vercel_metered_reads_balance_only_after_rejection_and_with_a_cooldown() {
    let guard = Guard::default();
    let key = [1; 32];
    let other = [2; 32];
    let start = Instant::now();
    let no_read = || async { panic!("unexpected balance query") };
    guard.admit_metered(&key, start, no_read).await.unwrap();
    guard.pause(key, start).await;
    let blocked = guard.admit_metered(&key, start, no_read).await.unwrap_err();
    assert!(local_denial(&blocked));
    assert!(!local_denial(
        &blocked.with_code("vercel_monthly_credit_exhausted")
    ));
    guard.admit_metered(&other, start, no_read).await.unwrap();

    let first = start + RECHECK;
    assert!(guard
        .admit_metered(&key, first, || async {
            // A sibling request does not issue another probe while this one runs.
            assert!(guard.admit_metered(&key, first, no_read).await.is_err());
            Some(0.0)
        })
        .await
        .is_err());
    assert!(guard.admit_metered(&key, first, no_read).await.is_err());
    guard
        .admit_metered(&key, first + RECHECK, || async { Some(4.25) })
        .await
        .unwrap();
    guard.admit_metered(&key, first, no_read).await.unwrap();
}

#[test]
fn vercel_balance_rejection_is_not_a_spend_budget_or_unreadable_402() {
    assert!(is_depletion(
        r#"{"error":{"type":"insufficient_credits","message":"No positive balance"}}"#
    ));
    assert!(is_depletion(
        r#"{"error":{"type":"payment_required","message":"No positive balance"}}"#
    ));
    assert!(!is_depletion(
        r#"{"error":{"type":"quota_for_entity_exceeded","message":"Insufficient balance"}}"#
    ));
    assert!(!is_depletion("<html>payment required</html>"));
}

#[test]
fn vercel_recognizes_only_ordinary_credit_and_checks_all_zero_price_components() {
    for (balance, expected) in [
        ("0", Some(0.0)),
        ("5", Some(5.0)),
        ("5.01", None),
        ("NaN", None),
    ] {
        assert_eq!(
            recognized_balance(&json!({"balance":balance, "total_used":"500"})),
            expected
        );
    }
    assert!(recognized_balance(&json!({"balance":"5","token_pass":{"remaining":"1"}})).is_none());
    let row = |pricing| json!({"type":"language", "pricing":pricing});
    assert!(zero_price(&row(json!({"input":"0","output":"0"}))));
    for pricing in [
        json!({"input":"0","output":"0","varies_by_provider":true}),
        json!({"input":"0","output":"0","input_cache_write":"0.001"}),
        json!({"input":"-1","output":"0"}),
    ] {
        assert!(!zero_price(&row(pricing)));
    }
}
