// ---------------------------------------------------------------------------
// Routing score calculation
//
// Rust port of provider-routing-score.ts.
//
// The score is a [0.0, 1.0] float that reflects the health and available
// capacity of a provider account.  It is multiplied with the candidate's base
// weight in `queue.rs` to produce the effective sort key.
// ---------------------------------------------------------------------------

// ---------------------------------------------------------------------------
// RoutingScore
// ---------------------------------------------------------------------------

use crate::balance::{merge_balance_into_routing_weight, BalanceStatus};

/// The result of scoring a single provider candidate.
#[derive(Debug, Clone)]
pub struct RoutingScore {
    /// Composite score in [0.0, 1.0].  Higher is better.
    pub score: f64,

    /// Contribution from health signals (circuit breaker, failure rate).
    pub health_weight: f64,

    /// Contribution from capacity signals (active concurrency vs. limit).
    pub capacity_weight: f64,

    /// `true` if the candidate is operating below its normal performance.
    pub degraded: bool,

    /// Human-readable reasons for any degradation.
    pub degradation_reasons: Vec<String>,
}

// ---------------------------------------------------------------------------
// Constants (mirroring the TypeScript defaults)
// ---------------------------------------------------------------------------

/// Weight given to health when computing the composite score.
const HEALTH_CONTRIBUTION: f64 = 0.6;

/// Weight given to capacity when computing the composite score.
const CAPACITY_CONTRIBUTION: f64 = 0.4;

/// Penalty multiplier applied per failure beyond the first.
const FAILURE_PENALTY_PER_COUNT: f64 = 0.1;

/// Score assigned to an open circuit breaker (before other adjustments).
const BREAKER_OPEN_SCORE: f64 = 0.0;

/// Score assigned when the provider status is something other than "active".
const UNHEALTHY_STATUS_SCORE: f64 = 0.05;

// ---------------------------------------------------------------------------
// build_routing_score
// ---------------------------------------------------------------------------

/// Compute a [`RoutingScore`] for a provider candidate.
///
/// # Parameters
/// - `status` — provider health status string, typically `"active"`,
///   `"degraded"`, or `"inactive"`.
/// - `failure_count` — number of recent failures recorded for this candidate.
/// - `breaker_open` — whether the circuit breaker is currently open.
/// - `active_concurrency` — number of requests currently in-flight to this
///   provider.
/// - `concurrency_limit` — optional maximum in-flight requests; if `None` the
///   capacity score is always 1.0.
pub fn build_routing_score(
    status: &str,
    failure_count: u32,
    breaker_open: bool,
    active_concurrency: usize,
    concurrency_limit: Option<usize>,
    balance_status: Option<&BalanceStatus>,
) -> RoutingScore {
    let mut degradation_reasons: Vec<String> = Vec::new();

    // ── Short-circuit: open circuit breaker → score = 0 ──────────────────
    // An open breaker means the provider is not serviceable at all; skip the
    // capacity calculation entirely so the composite is exactly 0.0.

    if breaker_open {
        degradation_reasons.push("circuit breaker open".to_string());
        return RoutingScore {
            score: 0.0,
            health_weight: BREAKER_OPEN_SCORE,
            capacity_weight: 0.0,
            degraded: true,
            degradation_reasons,
        };
    }

    // ── Health score ──────────────────────────────────────────────────────

    let health_weight = if status != "active" {
        degradation_reasons.push(format!("provider status is '{}'", status));
        UNHEALTHY_STATUS_SCORE
    } else {
        // Reduce by 10 % per failure, clamped to [0.0, 1.0].
        let penalty = (failure_count as f64) * FAILURE_PENALTY_PER_COUNT;
        let raw = 1.0 - penalty;
        if failure_count > 0 {
            degradation_reasons.push(format!("failure count = {}", failure_count));
        }
        raw.clamp(0.0, 1.0)
    };

    // ── Capacity score ────────────────────────────────────────────────────

    let capacity_weight = match concurrency_limit {
        None => 1.0,
        Some(limit) if limit == 0 => {
            // Limit of 0 is nonsensical — treat as unlimited.
            1.0
        }
        Some(limit) => {
            let ratio = active_concurrency as f64 / limit as f64;
            if ratio >= 1.0 {
                degradation_reasons.push(format!(
                    "at concurrency limit ({}/{})",
                    active_concurrency, limit
                ));
                0.0
            } else {
                // Linear capacity score: fully available → 1.0, fully used → 0.0
                let score = 1.0 - ratio;
                if ratio > 0.7 {
                    degradation_reasons.push(format!(
                        "high concurrency utilisation ({:.0}%)",
                        ratio * 100.0
                    ));
                }
                score
            }
        }
    };

    // ── Composite ─────────────────────────────────────────────────────────

    let mut score = (health_weight * HEALTH_CONTRIBUTION + capacity_weight * CAPACITY_CONTRIBUTION)
        .clamp(0.0, 1.0);

    if let Some(balance) = balance_status {
        score = merge_balance_into_routing_weight(score, balance).clamp(0.0, 1.0);
        if balance.is_unavailable {
            degradation_reasons.push(
                balance
                    .reason
                    .clone()
                    .unwrap_or_else(|| "provider quota exhausted".to_string()),
            );
        } else if balance.should_deprioritize {
            degradation_reasons.push(
                balance
                    .reason
                    .clone()
                    .unwrap_or_else(|| "provider quota nearing limit".to_string()),
            );
        }
    }

    let degraded = !degradation_reasons.is_empty();

    RoutingScore {
        score,
        health_weight,
        capacity_weight,
        degraded,
        degradation_reasons,
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    // ── perfect provider ──────────────────────────────────────────────────

    #[test]
    fn perfect_provider_scores_one() {
        let s = build_routing_score("active", 0, false, 0, None, None);
        assert!(
            (s.score - 1.0).abs() < 1e-9,
            "score should be 1.0, got {}",
            s.score
        );
        assert!(!s.degraded);
        assert!(s.degradation_reasons.is_empty());
    }

    // ── circuit breaker open ──────────────────────────────────────────────

    #[test]
    fn breaker_open_scores_zero() {
        let s = build_routing_score("active", 0, true, 0, None, None);
        assert_eq!(s.health_weight, 0.0);
        assert_eq!(s.score, 0.0);
        assert!(s.degraded);
        assert!(s
            .degradation_reasons
            .iter()
            .any(|r| r.contains("circuit breaker")));
    }

    // ── inactive status ───────────────────────────────────────────────────

    #[test]
    fn inactive_status_scores_very_low() {
        let s = build_routing_score("inactive", 0, false, 0, None, None);
        // health_weight = 0.05, capacity_weight = 1.0
        // score = 0.05*0.6 + 1.0*0.4 = 0.03 + 0.4 = 0.43
        assert!((s.health_weight - 0.05).abs() < 1e-9);
        assert!(s.degraded);
    }

    // ── failure count penalties ───────────────────────────────────────────

    #[test]
    fn single_failure_reduces_health_by_10pct() {
        let s = build_routing_score("active", 1, false, 0, None, None);
        assert!((s.health_weight - 0.9).abs() < 1e-9);
        assert!(s.degraded);
    }

    #[test]
    fn ten_failures_clamps_health_to_zero() {
        let s = build_routing_score("active", 10, false, 0, None, None);
        assert_eq!(s.health_weight, 0.0);
    }

    #[test]
    fn twenty_failures_health_does_not_go_below_zero() {
        let s = build_routing_score("active", 20, false, 0, None, None);
        assert!(s.health_weight >= 0.0);
        assert!(s.score >= 0.0);
    }

    // ── concurrency capacity ──────────────────────────────────────────────

    #[test]
    fn no_concurrency_limit_gives_full_capacity() {
        let s = build_routing_score("active", 0, false, 999, None, None);
        assert!((s.capacity_weight - 1.0).abs() < 1e-9);
    }

    #[test]
    fn half_capacity_used_gives_half_capacity_score() {
        let s = build_routing_score("active", 0, false, 5, Some(10), None);
        assert!((s.capacity_weight - 0.5).abs() < 1e-9);
    }

    #[test]
    fn full_concurrency_gives_zero_capacity() {
        let s = build_routing_score("active", 0, false, 10, Some(10), None);
        assert_eq!(s.capacity_weight, 0.0);
        assert!(s.degraded);
        assert!(s
            .degradation_reasons
            .iter()
            .any(|r| r.contains("concurrency limit")));
    }

    #[test]
    fn over_concurrency_clamps_capacity_to_zero() {
        let s = build_routing_score("active", 0, false, 15, Some(10), None);
        assert_eq!(s.capacity_weight, 0.0);
    }

    #[test]
    fn zero_concurrency_limit_treated_as_unlimited() {
        let s = build_routing_score("active", 0, false, 5, Some(0), None);
        assert!((s.capacity_weight - 1.0).abs() < 1e-9);
    }

    // ── composite score range ─────────────────────────────────────────────

    #[test]
    fn composite_score_always_in_0_1_range() {
        // Exhaustive-ish sampling of edge cases
        for &failures in &[0u32, 1, 5, 10, 100] {
            for &breaker in &[false, true] {
                for status in &["active", "inactive", "degraded"] {
                    for &active in &[0usize, 5, 10, 20] {
                        let s =
                            build_routing_score(status, failures, breaker, active, Some(10), None);
                        assert!(
                            s.score >= 0.0 && s.score <= 1.0,
                            "score out of range: {} (status={}, failures={}, breaker={}, active={})",
                            s.score, status, failures, breaker, active
                        );
                    }
                }
            }
        }
    }

    // ── high utilisation warns ────────────────────────────────────────────

    #[test]
    fn high_utilisation_marks_degraded() {
        // 8/10 = 80% utilisation > 70% threshold
        let s = build_routing_score("active", 0, false, 8, Some(10), None);
        assert!(s.degraded);
        assert!(s
            .degradation_reasons
            .iter()
            .any(|r| r.contains("utilisation")));
    }

    #[test]
    fn low_utilisation_does_not_warn() {
        // 3/10 = 30% utilisation — below the 70% warn threshold
        let s = build_routing_score("active", 0, false, 3, Some(10), None);
        assert!(!s.degraded);
    }

    #[test]
    fn exhausted_balance_forces_score_zero() {
        let balance = BalanceStatus {
            provider_account_id: "prov-1".to_string(),
            display: "quota exhausted".to_string(),
            reason: Some("provider quota exhausted".to_string()),
            should_deprioritize: false,
            is_unavailable: true,
            remaining_ratio: Some(0.0),
        };
        let s = build_routing_score("active", 0, false, 0, None, Some(&balance));
        assert_eq!(s.score, 0.0);
        assert!(s
            .degradation_reasons
            .iter()
            .any(|reason| reason.contains("quota exhausted")));
    }

    #[test]
    fn warning_balance_penalises_score() {
        let balance = BalanceStatus {
            provider_account_id: "prov-1".to_string(),
            display: "quota warning".to_string(),
            reason: Some("provider quota nearing limit".to_string()),
            should_deprioritize: true,
            is_unavailable: false,
            remaining_ratio: Some(0.15),
        };
        let s = build_routing_score("active", 0, false, 0, None, Some(&balance));
        assert!(s.score < 1.0);
        assert!(s
            .degradation_reasons
            .iter()
            .any(|reason| reason.contains("nearing limit")));
    }
}
