use super::*;

fn key(id: &str) -> LatencyKey {
    LatencyKey {
        subject: id.into(),
        model: "model".into(),
        profile: "openai".into(),
        adapter: "openai_compatible".into(),
        endpoint: 0,
        streaming: false,
    }
}

#[test]
fn ewma_uses_only_recent_samples_and_expires_at_exact_boundary() {
    let cache = RoutingLatency::default();
    let now = Instant::now();
    let key = key("one");
    assert_eq!(cache.estimate_at(&key, now), None);
    cache.record_at(key.clone(), Duration::from_millis(100), now);
    cache.record_at(
        key.clone(),
        Duration::from_millis(200),
        now + Duration::from_secs(1),
    );
    assert_eq!(
        cache.estimate_at(&key, now + Duration::from_secs(600)),
        Some(130.0)
    );
    assert_eq!(
        cache.estimate_at(&key, now + Duration::from_secs(601)),
        None
    );
    cache.record_at(
        key.clone(),
        Duration::from_millis(500),
        now + Duration::from_secs(602),
    );
    assert_eq!(
        cache.estimate_at(&key, now + Duration::from_secs(602)),
        Some(500.0)
    );
}

#[test]
fn stream_timing_does_not_share_buffered_history_or_expiry() {
    let cache = RoutingLatency::default();
    let now = Instant::now();
    let buffered = key("credential");
    let mut streaming = buffered.clone();
    streaming.streaming = true;
    cache.record_at(buffered.clone(), Duration::from_millis(100), now);
    cache.record_at(
        streaming.clone(),
        Duration::from_millis(700),
        now + Duration::from_secs(500),
    );
    assert_eq!(cache.estimate_at(&buffered, now + TTL), None);
    assert_eq!(cache.estimate_at(&streaming, now + TTL), Some(700.0));
}

#[test]
fn capacity_and_expiry_index_stay_bounded_on_hot_and_cold_keys() {
    let cache = RoutingLatency {
        capacity: 2,
        ..Default::default()
    };
    let now = Instant::now();
    cache.record_at(key("one"), Duration::from_millis(1), now);
    cache.record_at(
        key("two"),
        Duration::from_millis(2),
        now + Duration::from_secs(1),
    );
    for n in 2..50 {
        cache.record_at(
            key("one"),
            Duration::from_millis(3),
            now + Duration::from_secs(n),
        );
    }
    cache.record_at(
        key("three"),
        Duration::from_millis(4),
        now + Duration::from_secs(50),
    );
    assert_eq!(
        cache.estimate_at(&key("two"), now + Duration::from_secs(50)),
        None
    );
    assert!(cache
        .estimate_at(&key("one"), now + Duration::from_secs(50))
        .is_some());
    let locked = cache.cache.lock();
    assert_eq!(locked.samples.len(), 2);
    assert_eq!(locked.oldest.len(), 2);
}

#[test]
fn bounded_samples_are_finite_and_never_fake_zero_latency() {
    let cache = RoutingLatency::default();
    let now = Instant::now();
    cache.record_at(key("zero"), Duration::ZERO, now);
    cache.record_at(key("huge"), Duration::MAX, now);
    assert_eq!(cache.estimate_at(&key("zero"), now), Some(1.0));
    assert_eq!(cache.estimate_at(&key("huge"), now), Some(86_400_000.0));
}

#[test]
fn every_identity_dimension_is_independent() {
    let cache = RoutingLatency::default();
    let now = Instant::now();
    let original = key("one");
    cache.record_at(original.clone(), Duration::from_millis(10), now);
    let mut variants = vec![original.clone(); 6];
    variants[0].subject = "two".into();
    variants[1].model = "other-model".into();
    variants[2].profile = "other-profile".into();
    variants[3].adapter = "other-adapter".into();
    variants[4].endpoint = 1;
    variants[5].streaming = true;
    for variant in variants {
        assert_eq!(cache.estimate_at(&variant, now), None);
    }
}
