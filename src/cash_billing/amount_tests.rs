use crate::cash_billing::{amount, multiplier_ppm, CashBalance, CashQuote};

#[test]
fn exact_three_layer_cash_and_single_micro_rounding() {
    assert_eq!(
        amount::charge(
            1000,
            0,
            2_000_000,
            0,
            multiplier_ppm("1.5").unwrap(),
            multiplier_ppm("0.8").unwrap()
        )
        .unwrap(),
        2_400_000
    );
    assert_eq!(amount::charge(1, 0, 1, 0, 1_000_000, 1_000_000).unwrap(), 1);
    assert_eq!(
        amount::charge(10_000, 10_000, 10, 20, 0, 1_000_000).unwrap(),
        0
    );
}

#[test]
fn cash_values_reject_overflow_negative_and_unrepresentable_precision() {
    for value in [
        "NaN",
        "inf",
        "-1",
        "1.0000001",
        "1000001",
        "1e999",
        "1e-2147483648",
        "",
        ".",
        "1.2.3",
    ] {
        assert!(multiplier_ppm(value).is_err(), "{value}");
    }
    for (text, expected) in [
        ("0", 0),
        ("1e-2", 10_000),
        ("0.000001", 1),
        ("1.250000", 1_250_000),
    ] {
        assert_eq!(multiplier_ppm(text).unwrap(), expected);
    }
    assert!(amount::charge(u64::MAX, u64::MAX, i64::MAX, i64::MAX, u64::MAX, u64::MAX).is_err());
    assert!(amount::charge(1, 0, -1, 0, 1, 1).is_err());
}

#[test]
fn only_final_amount_is_spent_and_lowering_limit_does_not_erase_it() {
    let mut balance = CashBalance::new(10_000);
    balance.reserve(8000).unwrap();
    assert_eq!(balance.spent_micros, 0);
    assert_eq!(balance.remaining_micros(), 2000);
    balance.settle(8000, 2400).unwrap();
    balance.total_micros = 2000;
    assert_eq!(balance.remaining_micros(), -400);
    assert!(balance.reserve(1).is_err());
    assert_eq!(balance.spent_micros, 2400);
}

#[test]
fn reported_upstream_overrun_records_the_full_debt_instead_of_capping_the_bill() {
    let mut balance = CashBalance::new(1000);
    balance.reserve(1000).unwrap();
    balance.settle(1000, 1200).unwrap();
    assert_eq!(balance.spent_micros, 1200);
    assert_eq!(balance.remaining_micros(), -200);
    assert!(balance.reserve(1).is_err());
}

#[test]
fn cached_input_is_counted_once_for_each_usage_dialect() {
    let quote = CashQuote {
        provider_account_id: "p".into(),
        credential_id: "c".into(),
        model: "m".into(),
        group_id: None,
        price_source: "fixture".into(),
        prompt_micros_per_1k_tokens: 1000,
        completion_micros_per_1k_tokens: 2000,
        group_multiplier_ppm: 1_000_000,
        account_multiplier_ppm: 1_000_000,
        cache_tokens_separate: true,
    };
    let usage = crate::protocol::canonical::TokenUsage {
        prompt_tokens: 10,
        completion_tokens: 5,
        total_tokens: 15,
        cache_read_input_tokens: Some(20),
        cache_creation_input_tokens: Some(30),
    };
    assert_eq!(quote.actual_charge(&usage).unwrap(), 70);
    assert_eq!(
        CashQuote {
            cache_tokens_separate: false,
            ..quote
        }
        .actual_charge(&usage)
        .unwrap(),
        20
    );
}
