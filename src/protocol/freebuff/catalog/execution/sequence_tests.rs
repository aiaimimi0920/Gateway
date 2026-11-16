//! Numeric boundaries are a local JSON safety policy, not an asserted upstream billing limit.
use super::*;
use serde_json::json;
fn cost(value: f64) -> Accounting {
    let mut observation = Accounting::default();
    observation.observe(&json!({"cost":value})).unwrap();
    observation
}
#[test]
fn freebuff_catalog_sequence_per_step_rounding_and_separate_last_value_snapshots() {
    let mut ledger = RunAccounting::default();
    let mut first = cost(0.125);
    first
        .observe(&json!({"cost_details":{"upstream_inference_cost":0.125}}))
        .unwrap();
    ledger.record(&first);
    ledger.record(&cost(0.25));
    ledger.record(&cost(0.0));
    assert_eq!(
        ledger.totals().unwrap(),
        RunTotals {
            total_steps: 3,
            direct_credits: 52,
            total_credits: 52
        }
    );
    let mut rounded = RunAccounting::default();
    rounded.record(&cost(0.125));
    rounded.record(&cost(0.25));
    assert_eq!(rounded.totals().unwrap().total_credits, 39);
}
#[test]
fn freebuff_catalog_sequence_unknown_observation_cannot_be_repaired_by_later_known_steps() {
    for before in [false, true] {
        let mut ledger = RunAccounting::default();
        if before {
            ledger.record(&cost(0.25));
        }
        ledger.record(&Accounting::default());
        ledger.record(&cost(0.0));
        ledger.record(&cost(0.25));
        assert_eq!(ledger.totals(), Err(BusinessError::AccountingUnknown));
    }
}
#[test]
fn freebuff_catalog_sequence_safe_integer_overflow_stays_unknown_without_saturation() {
    for steps in [false, true] {
        let mut ledger = RunAccounting::default();
        if steps {
            ledger.steps = Some(MAX_SAFE_INTEGER);
        } else {
            ledger.credits = Some(MAX_SAFE_INTEGER);
        }
        ledger.record(&cost(1.0));
        ledger.record(&cost(0.0));
        assert_eq!(ledger.totals(), Err(BusinessError::AccountingUnknown));
    }
}
#[test]
fn freebuff_catalog_sequence_zero_without_any_attempt_is_known_but_missing_cost_is_not() {
    let mut ledger = RunAccounting::default();
    assert_eq!(
        ledger.totals().unwrap(),
        RunTotals {
            total_steps: 0,
            direct_credits: 0,
            total_credits: 0
        }
    );
    ledger.record(&Accounting::default());
    assert_eq!(ledger.totals(), Err(BusinessError::AccountingUnknown));
}
