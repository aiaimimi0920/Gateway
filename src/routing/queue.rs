// ---------------------------------------------------------------------------
// Candidate queue building
//
// Takes a slice of raw [`RouteCandidate`]s, scores each one, applies the
// chosen selection strategy, and returns an ordered queue ready for the
// dispatch loop to try in order.
// ---------------------------------------------------------------------------

use std::collections::HashMap;

use rand::Rng;

use crate::balance::BalanceStatus;

use super::candidate::RouteCandidate;
use super::score::build_routing_score;

// ---------------------------------------------------------------------------
// Health snapshot type alias
// ---------------------------------------------------------------------------

/// A compact snapshot of a provider account's runtime health, keyed by
/// `provider_account_id`.
///
#[derive(Debug, Clone)]
pub struct HealthSnapshot {
    pub status: String,
    pub active_concurrency: usize,
    pub failure_count: u32,
    pub breaker_open: bool,
    pub balance_status: Option<BalanceStatus>,
}

// ---------------------------------------------------------------------------
// build_candidate_queue
// ---------------------------------------------------------------------------

/// Build an ordered candidate queue from `candidates`.
///
/// # Strategy
///
/// - `"priority_weighted"` (default) — sort by effective weight (base weight ×
///   routing score), highest first.
/// - `"round_robin"` — shuffle deterministically by insertion order, then sort
///   stable by priority tier only (no weight randomisation).
/// - `"random"` — shuffle fully at random.
///
/// In all strategies:
/// 1. Candidates with a circuit breaker open **or** currently in cooldown are
///    moved to the end of the queue.
/// 2. The sticky provider (if any) is promoted to the **front** of the queue
///    regardless of strategy, provided it is not blocked/in-cooldown.
///
/// # Parameters
/// - `candidates`          — unordered slice of resolved provider accounts.
/// - `selection_strategy`  — one of `"priority_weighted"`, `"round_robin"`,
///   `"random"`.
/// - `sticky_provider_id`  — if set, that provider is placed first.
/// - `health_snapshots`    — runtime health keyed by `provider_account_id`;
///   providers absent from this map are treated as perfectly healthy.
/// - `concurrency_limit`   — global concurrency ceiling forwarded to the scorer.
pub fn build_candidate_queue<'a>(
    candidates: &[RouteCandidate],
    selection_strategy: &str,
    sticky_provider_id: Option<&str>,
    health_snapshots: &HashMap<String, HealthSnapshot>,
    concurrency_limit: Option<usize>,
) -> Vec<RouteCandidate> {
    if candidates.is_empty() {
        return Vec::new();
    }

    // ── Score each candidate ──────────────────────────────────────────────

    struct Scored {
        candidate: RouteCandidate,
        effective_weight: f64,
        blocked: bool,  // circuit breaker open OR in cooldown
        excluded: bool, // hard-filtered (e.g. provider quota exhausted)
    }

    let now = chrono_now_rfc3339();

    let mut scored: Vec<Scored> = candidates
        .iter()
        .map(|c| {
            let mut candidate = c.clone();
            let snap = health_snapshots.get(c.runtime_subject_id());

            let (active_concurrency, failure_count, breaker_open, status, balance_status) =
                match snap {
                    Some(snapshot) => (
                        snapshot.active_concurrency,
                        snapshot.failure_count,
                        snapshot.breaker_open,
                        snapshot.status.as_str(),
                        snapshot.balance_status.as_ref(),
                    ),
                    None => (0usize, c.failure_count, false, "active", None),
                };

            let quota_exhausted = balance_status
                .as_ref()
                .is_some_and(|balance| balance.is_unavailable);

            let routing_score = build_routing_score(
                status,
                failure_count,
                breaker_open,
                active_concurrency,
                concurrency_limit,
                balance_status,
            );

            if quota_exhausted {
                candidate.routing_score = Some(0.0);
                candidate.routing_health_weight = Some(routing_score.health_weight);
                candidate.routing_capacity_weight = Some(routing_score.capacity_weight);
                candidate.routing_degraded = Some(true);
                candidate.routing_breaker_open = Some(breaker_open);
                candidate.routing_degradation_reasons = routing_score.degradation_reasons.clone();

                return Scored {
                    candidate,
                    effective_weight: 0.0,
                    blocked: false,
                    excluded: true,
                };
            };

            let in_cooldown = c.is_in_cooldown(&now);
            let blocked = breaker_open || in_cooldown || routing_score.score <= 0.0;

            candidate.routing_score = Some(routing_score.score);
            candidate.routing_health_weight = Some(routing_score.health_weight);
            candidate.routing_capacity_weight = Some(routing_score.capacity_weight);
            candidate.routing_degraded = Some(routing_score.degraded);
            candidate.routing_breaker_open = Some(breaker_open);
            candidate.routing_degradation_reasons = routing_score.degradation_reasons.clone();

            let effective_weight = if blocked {
                0.0
            } else {
                (c.weight as f64) * routing_score.score
            };

            Scored {
                candidate,
                effective_weight,
                blocked,
                excluded: false,
            }
        })
        .collect();

    // ── Apply selection strategy ──────────────────────────────────────────

    match selection_strategy {
        "round_robin" => {
            // Sort healthy candidates by priority (descending), blocked to end.
            scored.sort_by(|a, b| match (a.blocked, b.blocked) {
                (true, false) => std::cmp::Ordering::Greater,
                (false, true) => std::cmp::Ordering::Less,
                _ => b.candidate.priority.cmp(&a.candidate.priority),
            });
        }
        "random" => {
            let mut rng = rand::thread_rng();
            // Separate healthy from blocked, shuffle healthy, append blocked.
            let (mut healthy, blocked_list): (Vec<Scored>, Vec<Scored>) = scored
                .into_iter()
                .filter(|scored| !scored.excluded)
                .partition(|s| !s.blocked);
            // Fisher-Yates via rand
            for i in (1..healthy.len()).rev() {
                let j = rng.gen_range(0..=i);
                healthy.swap(i, j);
            }
            healthy.extend(blocked_list);
            scored = healthy;
        }
        // "priority_weighted" is the default for any unknown strategy value
        _ => {
            // Sort by effective_weight descending; within equal weight, higher
            // priority wins; blocked candidates sink to the end.
            scored.sort_by(|a, b| match (a.blocked, b.blocked) {
                (true, false) => std::cmp::Ordering::Greater,
                (false, true) => std::cmp::Ordering::Less,
                _ => b
                    .effective_weight
                    .partial_cmp(&a.effective_weight)
                    .unwrap_or(std::cmp::Ordering::Equal)
                    .then_with(|| b.candidate.priority.cmp(&a.candidate.priority)),
            });
        }
    }

    // ── Sticky provider promotion ─────────────────────────────────────────

    let mut queue: Vec<RouteCandidate> = scored
        .into_iter()
        .filter(|scored| !scored.excluded)
        .map(|s| s.candidate)
        .collect();

    if let Some(sticky_id) = sticky_provider_id {
        if let Some(pos) = queue
            .iter()
            .position(|c| c.provider_account_id == sticky_id)
        {
            // Only promote if the sticky candidate is not in a blocked position
            // (blocked candidates are already at the end; leaving them there is
            // intentional — the sticky hint should not force a broken provider).
            if pos != 0 {
                let sticky = queue.remove(pos);
                queue.insert(0, sticky);
            }
        }
    }

    queue
}

// ---------------------------------------------------------------------------
// Internal helper — current RFC 3339 timestamp (seconds resolution)
// ---------------------------------------------------------------------------

fn chrono_now_rfc3339() -> String {
    // We intentionally avoid pulling in `chrono` or `time` just for this.
    // std::time::SystemTime gives us seconds since UNIX epoch which is
    // sufficient for the lexicographic cooldown comparison.
    use std::time::{SystemTime, UNIX_EPOCH};
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    // Format as a sortable RFC 3339-like string (UTC, seconds precision).
    let (y, mo, d, h, mi, s) = unix_to_ymdhms(secs);
    format!("{:04}-{:02}-{:02}T{:02}:{:02}:{:02}Z", y, mo, d, h, mi, s)
}

/// Minimal UNIX timestamp → (year, month, day, hour, min, sec) converter.
/// Accurate for the range 2000-2100 which is all we need.
fn unix_to_ymdhms(mut secs: u64) -> (u64, u64, u64, u64, u64, u64) {
    let s = secs % 60;
    secs /= 60;
    let mi = secs % 60;
    secs /= 60;
    let h = secs % 24;
    let mut days = secs / 24;

    // Days since 1970-01-01 → calendar date using the proleptic Gregorian
    // calendar algorithm from https://howardhinnant.github.io/date_algorithms.html
    days += 719468; // shift epoch to 0000-03-01
    let era = days / 146097;
    let doe = days - era * 146097;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let mo = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if mo <= 2 { y + 1 } else { y };
    (y, mo, d, h, mi, s)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::super::candidate::{ProviderAccountPayload, ProviderExecutionMode, RouteCandidate};
    use super::*;
    use std::collections::HashMap;

    fn make_payload() -> ProviderAccountPayload {
        ProviderAccountPayload {
            discovered_protocols: Vec::new(),
            adapter: "openai_compatible".to_string(),
            base_url: "https://api.example.com".to_string(),
            api_key: "sk-test".to_string(),
            credential_id: None,
            expires_at: None,
            runtime_state_object_key: None,
            account_name: None,
            execution_mode: None,
            endpoint_execution_modes: None,
            default_model: None,
            headers: HashMap::new(),
            auth_mode: None,
            anthropic_version: None,
            beta_headers: None,
            auth_header_name: None,
            auth_token: None,
            responses_path: None,
            chat_completions_path: None,
            completions_path: None,
            embeddings_path: None,
            audio_transcriptions_path: None,
            audio_speech_path: None,
            messages_path: None,
            search_path: None,
            fetch_path: None,
            research_path: None,
            balance_path: None,
            search_query_field: None,
            fetch_urls_field: None,
            extra_body: None,
            session_auth: None,
            keepalive: None,
        }
    }

    fn make_candidate(id: &str, priority: i32, weight: i32) -> RouteCandidate {
        RouteCandidate {
            provider_account_id: id.to_string(),
            provider_credential_id: None,
            label: id.to_string(),
            payload: make_payload(),
            protocol_family: "openai".to_string(),
            protocol_profile: "openai".to_string(),
            supported_protocol_families: vec!["openai_chat".to_string()],
            adapter: "openai_compatible".to_string(),
            model_alias: None,
            upstream_model: None,
            resolved_execution_mode: ProviderExecutionMode::DirectHttp,
            priority,
            weight,
            failure_count: 0,
            cooldown_until: None,
            routing_score: None,
            routing_health_weight: None,
            routing_capacity_weight: None,
            routing_degraded: None,
            routing_breaker_open: None,
            routing_degradation_reasons: Vec::new(),
        }
    }

    // ── empty input ──────────────────────────────────────────────────────

    #[test]
    fn empty_candidates_returns_empty_queue() {
        let q = build_candidate_queue(&[], "priority_weighted", None, &HashMap::new(), None);
        assert!(q.is_empty());
    }

    // ── priority_weighted ordering ───────────────────────────────────────

    #[test]
    fn priority_weighted_sorts_by_effective_weight() {
        let candidates = vec![
            make_candidate("low", 1, 5),
            make_candidate("high", 1, 100),
            make_candidate("mid", 1, 50),
        ];
        let q = build_candidate_queue(
            &candidates,
            "priority_weighted",
            None,
            &HashMap::new(),
            None,
        );
        assert_eq!(q[0].provider_account_id, "high");
        assert_eq!(q[1].provider_account_id, "mid");
        assert_eq!(q[2].provider_account_id, "low");
    }

    // ── blocked candidates sink to end ───────────────────────────────────

    #[test]
    fn breaker_open_candidate_goes_to_end() {
        let candidates = vec![make_candidate("good", 1, 10), make_candidate("bad", 1, 100)];
        let mut snaps: HashMap<String, HealthSnapshot> = HashMap::new();
        snaps.insert(
            "bad".to_string(),
            HealthSnapshot {
                status: "active".to_string(),
                active_concurrency: 0,
                failure_count: 0,
                breaker_open: true,
                balance_status: None,
            },
        ); // breaker open
        let q = build_candidate_queue(&candidates, "priority_weighted", None, &snaps, None);
        assert_eq!(q[0].provider_account_id, "good");
        assert_eq!(q[1].provider_account_id, "bad");
    }

    // ── sticky provider promotion ─────────────────────────────────────────

    #[test]
    fn sticky_provider_is_promoted_to_front() {
        let candidates = vec![
            make_candidate("a", 1, 100),
            make_candidate("b", 1, 10),
            make_candidate("c", 1, 50),
        ];
        let q = build_candidate_queue(
            &candidates,
            "priority_weighted",
            Some("c"),
            &HashMap::new(),
            None,
        );
        assert_eq!(q[0].provider_account_id, "c");
    }

    #[test]
    fn sticky_provider_already_first_no_change() {
        let candidates = vec![make_candidate("a", 1, 100), make_candidate("b", 1, 10)];
        let q = build_candidate_queue(
            &candidates,
            "priority_weighted",
            Some("a"),
            &HashMap::new(),
            None,
        );
        assert_eq!(q[0].provider_account_id, "a");
    }

    // ── round_robin ordering ─────────────────────────────────────────────

    #[test]
    fn round_robin_orders_by_priority_descending() {
        let candidates = vec![
            make_candidate("p1", 1, 10),
            make_candidate("p3", 3, 10),
            make_candidate("p2", 2, 10),
        ];
        let q = build_candidate_queue(&candidates, "round_robin", None, &HashMap::new(), None);
        assert_eq!(q[0].priority, 3);
        assert_eq!(q[1].priority, 2);
        assert_eq!(q[2].priority, 1);
    }

    // ── random strategy ──────────────────────────────────────────────────

    #[test]
    fn random_returns_all_candidates() {
        let candidates = vec![
            make_candidate("a", 1, 10),
            make_candidate("b", 1, 10),
            make_candidate("c", 1, 10),
        ];
        let q = build_candidate_queue(&candidates, "random", None, &HashMap::new(), None);
        assert_eq!(q.len(), 3);
    }

    // ── single candidate ────────────────────────────────────────────────

    #[test]
    fn single_candidate_queue_contains_that_candidate() {
        let candidates = vec![make_candidate("only", 1, 10)];
        let q = build_candidate_queue(
            &candidates,
            "priority_weighted",
            None,
            &HashMap::new(),
            None,
        );
        assert_eq!(q.len(), 1);
        assert_eq!(q[0].provider_account_id, "only");
    }

    // ── cooldown candidates go to end ────────────────────────────────────

    #[test]
    fn cooldown_candidate_is_blocked_and_goes_to_end() {
        let mut cool = make_candidate("cool", 1, 100);
        cool.cooldown_until = Some("2099-01-01T00:00:00Z".to_string()); // far future
        let normal = make_candidate("norm", 1, 5);
        let candidates = vec![cool, normal];
        let q = build_candidate_queue(
            &candidates,
            "priority_weighted",
            None,
            &HashMap::new(),
            None,
        );
        assert_eq!(q[0].provider_account_id, "norm");
        assert_eq!(q[1].provider_account_id, "cool");
    }

    #[test]
    fn exhausted_quota_candidate_is_excluded() {
        let candidates = vec![
            make_candidate("healthy", 1, 10),
            make_candidate("spent", 1, 100),
        ];
        let mut snaps: HashMap<String, HealthSnapshot> = HashMap::new();
        snaps.insert(
            "spent".to_string(),
            HealthSnapshot {
                status: "active".to_string(),
                active_concurrency: 0,
                failure_count: 0,
                breaker_open: false,
                balance_status: Some(BalanceStatus {
                    provider_account_id: "spent".to_string(),
                    display: "quota exhausted".to_string(),
                    reason: Some("provider quota exhausted".to_string()),
                    should_deprioritize: false,
                    is_unavailable: true,
                    remaining_ratio: Some(0.0),
                }),
            },
        );
        let q = build_candidate_queue(&candidates, "priority_weighted", None, &snaps, None);
        assert_eq!(q.len(), 1);
        assert_eq!(q[0].provider_account_id, "healthy");
    }
}
