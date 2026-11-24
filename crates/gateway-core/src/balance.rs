// ---------------------------------------------------------------------------
// BalanceStatus — upstream account balance awareness for routing decisions
//
// Ported from TypeScript balance-status.ts (NeuroLoom platform).
// Provides a lightweight value type plus a weight-adjustment helper used by
// the router to deprioritise or exclude providers with low / exhausted balances.
// ---------------------------------------------------------------------------

use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// BalanceStatus
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BalanceStatus {
    /// Provider account identifier (e.g. `"openai:acct_abc123"`).
    pub provider_account_id: String,

    /// Human-readable display string for dashboards / logs
    /// (e.g. `"$4.20 remaining"`).
    pub display: String,

    /// Optional human-readable reason used by routing diagnostics.
    pub reason: Option<String>,

    /// When `true`, the account is low on funds and should be preferred less
    /// than full-balance alternatives, but can still be used.
    pub should_deprioritize: bool,

    /// When `true`, the account has no usable balance and must be excluded
    /// from routing entirely.
    pub is_unavailable: bool,

    /// Remaining balance expressed as a fraction of the original/maximum
    /// balance (`0.0` = empty, `1.0` = full). `None` when the provider does
    /// not expose balance information.
    pub remaining_ratio: Option<f64>,
}

// ---------------------------------------------------------------------------
// Routing weight adjustment
// ---------------------------------------------------------------------------

/// Adjust a provider's routing weight based on its current balance status.
///
/// # Decision table
///
/// | Condition                          | Multiplier |
/// |------------------------------------|-----------|
/// | `is_unavailable`                   | × 0 (excluded) |
/// | `should_deprioritize`              | × 0.1 |
/// | otherwise                          | × 1.0 (unchanged) |
///
/// The multipliers are applied to `weight` in priority order (first matching
/// condition wins) so that an unavailable provider always gets weight 0
/// regardless of other flags.
pub fn merge_balance_into_routing_weight(weight: f64, balance: &BalanceStatus) -> f64 {
    if balance.is_unavailable {
        return 0.0;
    }

    if balance.should_deprioritize {
        return weight * 0.1;
    }

    weight
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn make_status(
        should_deprioritize: bool,
        is_unavailable: bool,
        remaining_ratio: Option<f64>,
    ) -> BalanceStatus {
        BalanceStatus {
            provider_account_id: "test:acct".to_string(),
            display: "test".to_string(),
            reason: None,
            should_deprioritize,
            is_unavailable,
            remaining_ratio,
        }
    }

    // ── is_unavailable ───────────────────────────────────────────────────

    #[test]
    fn unavailable_returns_zero_weight() {
        let status = make_status(false, true, Some(0.0));
        assert_eq!(merge_balance_into_routing_weight(100.0, &status), 0.0);
    }

    #[test]
    fn unavailable_overrides_deprioritize() {
        // Even when both flags are set, `is_unavailable` wins.
        let status = make_status(true, true, Some(0.5));
        assert_eq!(merge_balance_into_routing_weight(100.0, &status), 0.0);
    }

    // ── should_deprioritize ──────────────────────────────────────────────

    #[test]
    fn deprioritize_multiplies_by_0_1() {
        let status = make_status(true, false, Some(0.6));
        assert!((merge_balance_into_routing_weight(100.0, &status) - 10.0).abs() < 1e-9);
    }

    #[test]
    fn deprioritize_overrides_low_ratio() {
        // `should_deprioritize` checked before `remaining_ratio < 0.3`.
        let status = make_status(true, false, Some(0.1));
        // Should apply 0.1 multiplier, not the 0.5 from low ratio.
        assert!((merge_balance_into_routing_weight(100.0, &status) - 10.0).abs() < 1e-9);
    }

    // ── no penalty ───────────────────────────────────────────────────────

    #[test]
    fn healthy_balance_returns_weight_unchanged() {
        let status = make_status(false, false, Some(0.8));
        assert!((merge_balance_into_routing_weight(50.0, &status) - 50.0).abs() < 1e-9);
    }

    #[test]
    fn low_ratio_without_warning_or_exhausted_returns_weight_unchanged() {
        let status = make_status(false, false, Some(0.1));
        assert!((merge_balance_into_routing_weight(80.0, &status) - 80.0).abs() < 1e-9);
    }

    #[test]
    fn none_ratio_with_no_flags_returns_unchanged() {
        let status = make_status(false, false, None);
        assert!((merge_balance_into_routing_weight(50.0, &status) - 50.0).abs() < 1e-9);
    }

    #[test]
    fn zero_weight_input_always_stays_zero() {
        let status = make_status(false, false, Some(1.0));
        assert_eq!(merge_balance_into_routing_weight(0.0, &status), 0.0);
    }

    // ── serialisation ────────────────────────────────────────────────────

    #[test]
    fn balance_status_round_trips_through_json() {
        let status = BalanceStatus {
            provider_account_id: "openai:acct_123".to_string(),
            display: "$4.20 remaining".to_string(),
            reason: None,
            should_deprioritize: false,
            is_unavailable: false,
            remaining_ratio: Some(0.42),
        };
        let json = serde_json::to_string(&status).unwrap();
        let decoded: BalanceStatus = serde_json::from_str(&json).unwrap();
        assert_eq!(decoded.provider_account_id, status.provider_account_id);
        assert_eq!(decoded.display, status.display);
        assert_eq!(decoded.remaining_ratio, status.remaining_ratio);
    }

    #[test]
    fn balance_status_serialises_none_ratio() {
        let status = make_status(false, false, None);
        let v: serde_json::Value = serde_json::to_value(&status).unwrap();
        assert!(v["remaining_ratio"].is_null());
    }
}
