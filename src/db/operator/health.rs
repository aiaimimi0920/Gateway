//! Provider health projection from cooldown and routing scores.

use super::*;

pub(super) fn build_provider_health(
    provider_account: &GatewayProviderAccountView,
    snapshot: Option<&ConcurrencySnapshot>,
    breaker_open: bool,
    balance_status: Option<&BalanceStatus>,
) -> GatewayProviderHealthView {
    let (effective_status, effective_cooldown_until, effective_failure_count) =
        normalize_operator_provider_runtime_state(provider_account);
    let (active_concurrency, provider_limit) = snapshot
        .map(|snapshot| (snapshot.active_count, Some(snapshot.current_limit)))
        .unwrap_or((0, None));
    let routing_score = build_routing_score(
        effective_status.as_str(),
        effective_failure_count.max(0) as u32,
        breaker_open,
        active_concurrency,
        provider_limit,
        balance_status,
    );

    GatewayProviderHealthView {
        provider_account_id: provider_account.id.clone(),
        label: provider_account.label.clone(),
        adapter: provider_account.adapter.clone(),
        protocol_family: provider_account.protocol_family.clone(),
        status: effective_status,
        cooldown_until: effective_cooldown_until,
        failure_count: effective_failure_count,
        last_error: provider_account.last_error.clone(),
        last_health_check_at: provider_account.last_health_check_at.clone(),
        active_concurrency,
        breaker_open,
        routing_score: round_score(routing_score.score),
        health_weight: round_score(routing_score.health_weight),
        capacity_weight: round_score(routing_score.capacity_weight),
        degraded: routing_score.degraded,
        saturated: routing_score.capacity_weight <= 0.0,
        degradation_reasons: routing_score.degradation_reasons,
    }
}

fn normalize_operator_provider_runtime_state(
    provider_account: &GatewayProviderAccountView,
) -> (String, Option<String>, i32) {
    let cooldown_active = provider_account
        .cooldown_until
        .as_deref()
        .and_then(parse_rfc3339)
        .is_some_and(|until| until > OffsetDateTime::now_utc());

    if provider_account.status == "cooling" && !cooldown_active {
        return ("active".to_string(), None, 0);
    }

    (
        provider_account.status.clone(),
        provider_account.cooldown_until.clone(),
        provider_account.failure_count,
    )
}

fn parse_rfc3339(value: &str) -> Option<OffsetDateTime> {
    OffsetDateTime::parse(value, &time::format_description::well_known::Rfc3339).ok()
}
