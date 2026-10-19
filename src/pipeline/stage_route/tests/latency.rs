//! Real affinity storage must survive advisory sorting, but not health checks.
use super::*;
use crate::routing::latency::LatencyKey;
use std::time::Duration;

#[tokio::test]
async fn local_affinity_survives_latency_only_while_the_candidate_is_eligible() {
    let document = serde_yaml::from_str(
        r#"
providers:
  - {id: sticky, preset: openai, base_url: 'http://127.0.0.1:1', api_key: fixture-key}
  - {id: fast, preset: openai, base_url: 'http://127.0.0.1:1', api_key: fixture-key}
model_routes:
  - {pattern: gpt-4o, provider_ids: [sticky, fast], priority: 10}
"#,
    )
    .unwrap();
    let store = RouteConfigStore::from_document(document).unwrap();
    let root = std::env::temp_dir().join(format!(
        "gateway-local-reference-affinity-{}",
        uuid::Uuid::new_v4()
    ));
    let local = crate::local_runtime::LocalRuntime::open(&root)
        .await
        .unwrap();
    let mut state = make_state_with_route_store(store);
    Arc::get_mut(&mut state).unwrap().local_runtime = Some(local.clone());
    let mut req = make_request();
    req.explicit_session_key = Some("reference-sticky".into());
    let candidates = state.route_config.resolve_candidates(Some("gpt-4o"));
    let fast = candidates
        .iter()
        .find(|c| c.provider_account_id == "fast")
        .unwrap();
    let sticky = candidates
        .iter()
        .find(|c| c.provider_account_id == "sticky")
        .unwrap();
    state.routing_latency.record(
        LatencyKey::for_request(fast, &req).unwrap(),
        Duration::from_millis(10),
    );
    local.set_credential_affinity("session:reference-sticky", "gpt-4o", "sticky");
    let mut healthy = PipelineContext::new(req.clone(), None);
    run(&mut healthy, &state).await.unwrap();
    let healthy_first = healthy.candidates[0].provider_account_id.clone();
    local
        .model_failure(db::RecordCredentialModelFailureInput {
            provider_account_id: sticky.provider_account_id.clone(),
            provider_credential_id: sticky.provider_credential_id.clone(),
            provider_credential_ref: sticky.payload.credential_id.clone(),
            protocol_profile: Some(sticky.protocol_profile.clone()),
            model: "gpt-4o".into(),
            upstream_status: Some(429),
            upstream_retry_after_ms: Some(172_800_000),
            error_message: Some("rate limit".into()),
            classification: crate::provider_failure::classify_provider_failure(
                Some(429),
                None,
                Some("rate limit"),
            ),
        })
        .await
        .unwrap();
    let mut cooling = PipelineContext::new(req, None);
    run(&mut cooling, &state).await.unwrap();
    let cooling_first = cooling.candidates[0].provider_account_id.clone();
    local.close().await;
    drop(state);
    crate::local_runtime::test_support::remove_test_root(&root).await;
    assert_eq!(
        healthy_first, "sticky",
        "latency must not steal an eligible conversation affinity"
    );
    assert_eq!(
        cooling_first, "fast",
        "affinity must not revive a cooling candidate"
    );
}
