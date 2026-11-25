use super::*;
use serde_json::json;

#[tokio::test]
async fn vercel_metered_reads_balance_only_after_rejection_and_with_a_cooldown() {
    let guard = Ledger::default();
    let key = [1; 32];
    let other = [2; 32];
    let start = Instant::now();
    let no_read = || async { panic!("unexpected balance query") };
    guard.admit(key, start, no_read).await.unwrap().pause(start);
    let blocked = guard.admit(key, start, no_read).await.err().unwrap();
    assert!(local_denial(&blocked));
    assert_eq!(blocked.http_status, Some(402));
    assert!(matches!(
        blocked.fallback_hint,
        crate::error::FallbackHint::FallbackProvider { .. }
    ));
    assert!(!local_denial(
        &blocked.with_code("vercel_monthly_credit_exhausted")
    ));
    guard.admit(other, start, no_read).await.unwrap();

    let first = start + RECHECK;
    assert!(guard
        .admit(key, first, || async {
            // A sibling request does not issue another probe while this one runs.
            assert!(guard.admit(key, first, no_read).await.is_err());
            Some(0.0)
        })
        .await
        .is_err());
    assert!(guard.admit(key, first, no_read).await.is_err());
    guard
        .admit(key, first + RECHECK, || async { Some(4.25) })
        .await
        .unwrap();
    guard.admit(key, first, no_read).await.unwrap();
}

#[tokio::test]
async fn vercel_pause_table_never_forgets_a_rejection_at_capacity() {
    let ledger = Ledger::default();
    let start = Instant::now();
    let no_read = || async { panic!("unexpected credit query") };
    let key = |n: usize| -> Key { Sha256::digest(n.to_le_bytes()).into() };
    for n in 0..ledger::CAPACITY - 1 {
        ledger
            .admit(key(n), start, no_read)
            .await
            .unwrap()
            .pause(start);
    }
    let active = ledger
        .admit(key(ledger::CAPACITY), start, no_read)
        .await
        .unwrap();
    let other = key(ledger::CAPACITY + 1);
    assert!(ledger.admit(other, start, no_read).await.is_err());
    drop(active);
    ledger
        .admit(other, start, no_read)
        .await
        .unwrap()
        .pause(start);
    assert!(ledger.admit(key(0), start, no_read).await.is_err());
    ledger
        .admit(key(0), start + RECHECK, || async { Some(2.0) })
        .await
        .unwrap();
    ledger
        .admit(key(ledger::CAPACITY), start + RECHECK, no_read)
        .await
        .unwrap();
}

#[tokio::test]
async fn vercel_stale_balance_cannot_clear_a_newer_402() {
    let ledger = Ledger::default();
    let start = Instant::now();
    let k = [4; 32];
    let no_read = || async { panic!("unexpected credit query") };
    let late = ledger.admit(k, start, no_read).await.unwrap();
    ledger.admit(k, start, no_read).await.unwrap().pause(start);
    let recheck = start + RECHECK;
    assert!(ledger
        .admit(k, recheck, || async {
            late.pause(recheck);
            Some(4.0)
        })
        .await
        .is_err());
    assert!(ledger.admit(k, recheck, no_read).await.is_err());
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
