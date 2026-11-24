use std::collections::{HashMap, VecDeque};
use std::sync::Mutex;

use async_trait::async_trait;
use axum::body::Body;
use axum::http::{header::RETRY_AFTER, StatusCode};
use axum::response::IntoResponse;
use http_body_util::BodyExt;
use redis::AsyncCommands;

use neuro_gateway::db::{GatewayRateLimitDefinition, GatewayRoutePolicyConfig};
use neuro_gateway::metrics::request::GatewayMetrics;
use neuro_gateway::rate_limit::{
    build_provider_attempt_rate_limit_rule, build_request_rate_limit_rules,
    enforce_rate_limit_rules, enforce_rate_limit_rules_with_timeout, ProviderRateLimitRejections,
    RateLimitAdmission, RateLimitBackendDecision, RateLimitDimensions, RateLimitRule,
    RateLimitScope, RateLimitStore, RateLimitStoreError, RedisRateLimitStore,
};

fn definition(window_seconds: i32, max_requests: i32) -> GatewayRateLimitDefinition {
    GatewayRateLimitDefinition {
        window_seconds,
        max_requests,
    }
}

fn configured_policy() -> GatewayRoutePolicyConfig {
    let mut policy = GatewayRoutePolicyConfig {
        rate_limit_enforcement_version: Some("v1".to_string()),
        rate_limit_window_seconds: Some(60),
        rate_limit_max_requests: Some(10),
        api_key_rate_limit: Some(definition(60, 5)),
        provider_attempt_rate_limit: Some(definition(30, 2)),
        ..GatewayRoutePolicyConfig::default()
    };
    policy.model_rate_limits = Some(HashMap::from([("gpt-4o".to_string(), definition(20, 4))]));
    policy.endpoint_rate_limits = Some(HashMap::from([(
        "post /v1/chat/completions".to_string(),
        definition(15, 3),
    )]));
    policy
}

#[test]
fn legacy_route_policy_defaults_do_not_enable_runtime_rate_limits() {
    let policy = GatewayRoutePolicyConfig::default();

    assert_eq!(policy.rate_limit_enforcement_version, None);
    assert!(build_request_rate_limit_rules(&policy, &dimensions(None))
        .unwrap()
        .is_empty());
    assert!(
        build_provider_attempt_rate_limit_rule(&policy, &dimensions(Some("provider-a")))
            .unwrap()
            .is_none()
    );
}

fn dimensions<'a>(provider_account_id: Option<&'a str>) -> RateLimitDimensions<'a> {
    RateLimitDimensions {
        tenant_id: "tenant-a",
        project_id: "project-a",
        access_key_id: Some("sk-development-secret"),
        model: Some("GPT-4O"),
        endpoint: "chat_completions",
        endpoint_policy_alias: Some("POST /v1/chat/completions"),
        provider_account_id,
    }
}

#[test]
fn route_policy_resolves_project_key_model_and_endpoint_rules() {
    let rules = build_request_rate_limit_rules(&configured_policy(), &dimensions(None)).unwrap();

    assert_eq!(
        rules.iter().map(|rule| rule.scope).collect::<Vec<_>>(),
        vec![
            RateLimitScope::Project,
            RateLimitScope::AccessKey,
            RateLimitScope::Model,
            RateLimitScope::Endpoint,
        ]
    );
    assert_eq!(
        rules
            .iter()
            .map(|rule| (rule.window_seconds, rule.max_requests))
            .collect::<Vec<_>>(),
        vec![(60, 10), (60, 5), (20, 4), (15, 3)]
    );
}

#[test]
fn provider_attempt_rule_is_scoped_per_provider() {
    let first = build_provider_attempt_rate_limit_rule(
        &configured_policy(),
        &dimensions(Some("provider-a")),
    )
    .unwrap()
    .expect("configured provider attempt rule");
    let second = build_provider_attempt_rate_limit_rule(
        &configured_policy(),
        &dimensions(Some("provider-b")),
    )
    .unwrap()
    .expect("configured provider attempt rule");

    assert_eq!(first.scope, RateLimitScope::ProviderAttempt);
    assert_eq!((first.window_seconds, first.max_requests), (30, 2));
    assert_ne!(first.key, second.key);
}

#[test]
fn redis_scope_keys_do_not_expose_raw_identity_or_api_key_material() {
    let rules = build_request_rate_limit_rules(&configured_policy(), &dimensions(None)).unwrap();

    for rule in rules {
        assert!(!rule.key.contains("tenant-a"));
        assert!(!rule.key.contains("project-a"));
        assert!(!rule.key.contains("sk-development-secret"));
        assert!(!rule.key.contains("GPT-4O"));
        assert!(!rule.key.contains("chat_completions"));
    }
}

#[test]
fn request_scope_keys_share_an_anonymous_redis_cluster_hash_tag() {
    let rules = build_request_rate_limit_rules(&configured_policy(), &dimensions(None)).unwrap();
    let tags = rules
        .iter()
        .map(|rule| {
            let start = rule.key.find('{').expect("Redis Cluster hash tag start") + 1;
            let end = rule.key[start..]
                .find('}')
                .map(|offset| start + offset)
                .expect("Redis Cluster hash tag end");
            &rule.key[start..end]
        })
        .collect::<std::collections::HashSet<_>>();

    assert_eq!(tags.len(), 1, "one Lua admission must use one hash slot");
    assert!(!tags.contains("tenant-a"));
    assert!(!tags.contains("project-a"));
}

struct SequenceStore {
    decisions: Mutex<VecDeque<Result<RateLimitBackendDecision, RateLimitStoreError>>>,
}

struct DelayedStore;

#[async_trait]
impl RateLimitStore for DelayedStore {
    async fn admit(
        &self,
        _rules: &[RateLimitRule],
    ) -> Result<RateLimitBackendDecision, RateLimitStoreError> {
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
        Ok(RateLimitBackendDecision::Allowed)
    }
}

impl SequenceStore {
    fn new(decisions: Vec<Result<RateLimitBackendDecision, RateLimitStoreError>>) -> Self {
        Self {
            decisions: Mutex::new(decisions.into()),
        }
    }
}

#[async_trait]
impl RateLimitStore for SequenceStore {
    async fn admit(
        &self,
        _rules: &[RateLimitRule],
    ) -> Result<RateLimitBackendDecision, RateLimitStoreError> {
        self.decisions
            .lock()
            .expect("sequence store mutex")
            .pop_front()
            .expect("test decision")
    }
}

#[tokio::test]
async fn redis_failure_is_fail_open_and_recovery_reenables_enforcement() {
    let store = SequenceStore::new(vec![
        Err(RateLimitStoreError::Unavailable(
            "redis unavailable".to_string(),
        )),
        Ok(RateLimitBackendDecision::Rejected {
            rule_index: 0,
            retry_after_millis: 1_500,
        }),
    ]);
    let rules = build_request_rate_limit_rules(&configured_policy(), &dimensions(None)).unwrap();

    let unavailable = enforce_rate_limit_rules(&store, &rules).await;
    assert!(matches!(
        unavailable,
        RateLimitAdmission::BypassedStoreUnavailable { .. }
    ));
    assert!(unavailable.into_gateway_result().is_ok());

    let recovered = enforce_rate_limit_rules(&store, &rules).await;
    let error = recovered
        .into_gateway_result()
        .expect_err("recovered Redis must enforce the configured limit");
    assert_eq!(error.http_status, Some(429));
    assert_eq!(error.code.as_deref(), Some("rate_limit_exceeded"));
}

#[tokio::test]
async fn slow_mutating_rate_limit_store_is_indeterminate_and_fails_closed() {
    let rules = build_request_rate_limit_rules(&configured_policy(), &dimensions(None)).unwrap();
    let admission = enforce_rate_limit_rules_with_timeout(
        &DelayedStore,
        &rules,
        std::time::Duration::from_millis(5),
    )
    .await;

    assert!(matches!(
        admission,
        RateLimitAdmission::IndeterminateStoreFailure { .. }
    ));
    let error = admission
        .into_gateway_result()
        .expect_err("an in-flight mutating timeout must fail closed");
    assert_eq!(error.http_status, Some(503));
    assert_eq!(
        error.code.as_deref(),
        Some("rate_limit_admission_indeterminate")
    );
}

#[tokio::test]
async fn invalid_rule_index_is_indeterminate_and_fails_closed() {
    let rules = build_request_rate_limit_rules(&configured_policy(), &dimensions(None)).unwrap();
    let store = SequenceStore::new(vec![Ok(RateLimitBackendDecision::Rejected {
        rule_index: rules.len() + 3,
        retry_after_millis: 1_500,
    })]);

    let admission = enforce_rate_limit_rules(&store, &rules).await;
    assert!(matches!(
        admission,
        RateLimitAdmission::IndeterminateStoreFailure { .. }
    ));
    let error = admission
        .into_gateway_result()
        .expect_err("an invalid store rule index must fail closed");
    assert_eq!(error.http_status, Some(503));
    assert_eq!(
        error.code.as_deref(),
        Some("rate_limit_admission_indeterminate")
    );
}

#[tokio::test]
async fn rejected_admission_returns_stable_429_body_and_retry_after_header() {
    let store = SequenceStore::new(vec![Ok(RateLimitBackendDecision::Rejected {
        rule_index: 0,
        retry_after_millis: 1_500,
    })]);
    let rules = build_request_rate_limit_rules(&configured_policy(), &dimensions(None)).unwrap();
    let error = enforce_rate_limit_rules(&store, &rules)
        .await
        .into_gateway_result()
        .expect_err("limit must reject");

    let response = error.into_response();
    assert_eq!(response.status(), StatusCode::TOO_MANY_REQUESTS);
    assert_eq!(
        response.headers().get(RETRY_AFTER).unwrap(),
        "2",
        "Retry-After uses whole seconds rounded up"
    );
    let bytes = Body::new(response.into_body())
        .collect()
        .await
        .unwrap()
        .to_bytes();
    let body: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(body["error"]["code"], "rate_limit_exceeded");
}

#[test]
fn rate_limit_metrics_distinguish_rejections_and_store_fail_open() {
    let metrics = GatewayMetrics::new();
    metrics.observe_rate_limit_admission(false, false);
    metrics.observe_rate_limit_admission(true, false);
    metrics.observe_rate_limit_admission(false, true);

    let output = metrics.render_prometheus();
    assert!(output.contains("gateway_rate_limit_checks_total 3"));
    assert!(output.contains("gateway_rate_limit_rejections_total 1"));
    assert!(output.contains("gateway_rate_limit_store_failures_total 1"));
}

#[test]
fn provider_rejections_return_the_minimum_retry_after_and_real_errors_take_priority() {
    let mut rejections = ProviderRateLimitRejections::default();
    rejections.observe(60_000);
    rejections.observe(1_500);

    let error = rejections
        .clone()
        .resolve_terminal_error(None)
        .expect("provider rejections must produce a terminal 429");
    assert_eq!(error.http_status, Some(429));
    assert!(matches!(
        error.fallback_hint,
        neuro_gateway::error::FallbackHint::Retry {
            delay_ms: 1_500,
            ..
        }
    ));

    let upstream = neuro_gateway::error::GatewayError::service_unavailable("provider failed")
        .with_code("provider_unavailable");
    let error = rejections
        .resolve_terminal_error(Some(upstream))
        .expect("a real provider error must be retained");
    assert_eq!(error.code.as_deref(), Some("provider_unavailable"));
}

#[tokio::test]
#[ignore = "requires GATEWAY_RATE_LIMIT_REDIS_URL Redis fixture"]
async fn redis_fixed_window_rejection_does_not_partially_increment_other_scopes() {
    let redis_url = std::env::var("GATEWAY_RATE_LIMIT_REDIS_URL")
        .expect("ignored Redis contract requires GATEWAY_RATE_LIMIT_REDIS_URL");
    let pool = deadpool_redis::Config::from_url(redis_url)
        .create_pool(Some(deadpool_redis::Runtime::Tokio1))
        .expect("create Redis test pool");
    let unique = uuid::Uuid::new_v4().to_string();
    let policy = GatewayRoutePolicyConfig {
        rate_limit_enforcement_version: Some("v1".to_string()),
        rate_limit_window_seconds: Some(60),
        rate_limit_max_requests: Some(1),
        api_key_rate_limit: Some(definition(60, 10)),
        model_rate_limits: None,
        endpoint_rate_limits: None,
        ..GatewayRoutePolicyConfig::default()
    };
    let tenant_id = format!("tenant-{unique}");
    let project_id = format!("project-{unique}");
    let access_key_id = format!("key-{unique}");
    let dimensions = RateLimitDimensions {
        tenant_id: &tenant_id,
        project_id: &project_id,
        access_key_id: Some(&access_key_id),
        model: None,
        endpoint: "chat_completions",
        endpoint_policy_alias: Some("post /v1/chat/completions"),
        provider_account_id: None,
    };
    let rules = build_request_rate_limit_rules(&policy, &dimensions).unwrap();
    assert_eq!(rules.len(), 2);
    let store = RedisRateLimitStore::new(&pool);

    assert_eq!(
        store.admit(&rules).await.unwrap(),
        RateLimitBackendDecision::Allowed
    );
    assert!(matches!(
        store.admit(&rules).await.unwrap(),
        RateLimitBackendDecision::Rejected { rule_index: 0, .. }
    ));

    let access_key = &rules[1].key;
    let mut connection = pool.get().await.unwrap();
    let access_count: u64 = connection.get(access_key).await.unwrap();
    assert_eq!(
        access_count, 1,
        "rejected batch must not partially increment"
    );
    let keys = rules.iter().map(|rule| &rule.key).collect::<Vec<_>>();
    let _: usize = redis::cmd("DEL")
        .arg(keys)
        .query_async(&mut connection)
        .await
        .unwrap();
}

#[tokio::test]
#[ignore = "requires GATEWAY_RATE_LIMIT_REDIS_URL Redis fixture"]
async fn redis_fixed_window_repairs_missing_ttl_before_rejecting() {
    let redis_url = std::env::var("GATEWAY_RATE_LIMIT_REDIS_URL")
        .expect("ignored Redis contract requires GATEWAY_RATE_LIMIT_REDIS_URL");
    let pool = deadpool_redis::Config::from_url(redis_url)
        .create_pool(Some(deadpool_redis::Runtime::Tokio1))
        .expect("create Redis test pool");
    let unique = uuid::Uuid::new_v4().to_string();
    let policy = GatewayRoutePolicyConfig {
        rate_limit_enforcement_version: Some("v1".to_string()),
        rate_limit_window_seconds: Some(60),
        rate_limit_max_requests: Some(1),
        ..GatewayRoutePolicyConfig::default()
    };
    let tenant_id = format!("tenant-{unique}");
    let project_id = format!("project-{unique}");
    let dimensions = RateLimitDimensions {
        tenant_id: &tenant_id,
        project_id: &project_id,
        access_key_id: None,
        model: None,
        endpoint: "chat_completions",
        endpoint_policy_alias: Some("post /v1/chat/completions"),
        provider_account_id: None,
    };
    let rules = build_request_rate_limit_rules(&policy, &dimensions).unwrap();
    let rule = &rules[0];
    let mut connection = pool.get().await.unwrap();
    let _: () = connection.set(&rule.key, rule.max_requests).await.unwrap();
    let store = RedisRateLimitStore::new(&pool);

    let decision = store.admit(&rules).await.unwrap();
    assert!(matches!(
        decision,
        RateLimitBackendDecision::Rejected { .. }
    ));
    let ttl: i64 = connection.pttl(&rule.key).await.unwrap();
    assert!(ttl > 0, "a saturated key without TTL must be repaired");

    let _: usize = connection.del(&rule.key).await.unwrap();
}
