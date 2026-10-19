use neuro_gateway::{
    error::classify_upstream_error,
    provider_failure::classify_provider_failure,
    provider_health_state::{transition_for_failure, transition_for_failure_with_retry_after},
};

#[test]
fn absent_hint_is_not_confused_with_the_retry_loops_default() {
    let error = classify_upstream_error(429, r#"{"error":{"message":"rate limit"}}"#, None);
    assert_eq!(error.upstream_retry_after_ms, None);
    let classification = classify_provider_failure(Some(429), None, Some(&error.message));
    assert_eq!(
        transition_for_failure_with_retry_after(&classification, error.upstream_retry_after_ms)
            .cooldown_seconds,
        Some(300)
    );
    assert_eq!(
        neuro_gateway::error::GatewayError::rate_limited("local limit", 25).upstream_retry_after_ms,
        None
    );
}

#[test]
fn zero_overflow_and_non_rate_limit_policies_are_preserved() {
    let zero = classify_upstream_error(429, r#"{"retry_after":0}"#, None);
    assert_eq!(zero.upstream_retry_after_ms, Some(0));
    let limited = classify_provider_failure(Some(429), None, Some("rate limit"));
    assert_eq!(
        transition_for_failure_with_retry_after(&limited, Some(0)).cooldown_seconds,
        Some(0)
    );
    assert_eq!(
        transition_for_failure_with_retry_after(&limited, Some(u64::MAX)).cooldown_seconds,
        Some((u64::MAX / 1_000 + 1) as i64)
    );
    for status in [400, 401, 404, 500, 503] {
        let classification = classify_provider_failure(Some(status), None, None);
        assert_eq!(
            transition_for_failure(&classification),
            transition_for_failure_with_retry_after(&classification, Some(172_800_000))
        );
    }
}
