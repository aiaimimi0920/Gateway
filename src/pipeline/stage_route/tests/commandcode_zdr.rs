//! Exercise the real send-loop early skips without an upstream or a database service.
use super::*;
use serde_json::json;

fn document(enabled: bool) -> crate::routing::config::RouteConfigYaml {
    serde_json::from_value(json!({"providers":[
        {"id":"zdr","preset":"commandcode-api","base_url":crate::protocol::commandcode::BASE,
         "supported_models":["gpt-4o"],"request_wait":{"mode":"disabled"},
         "credentials":[{"id":"private","enabled":enabled,"api_key":"synthetic-private",
             "headers":{"x-cmd-zdr":"1"}}]},
        {"id":"ordinary","base_url":"http://127.0.0.1:1/v1",
         "supported_models":["gpt-4o"],"request_wait":{"mode":"disabled"},
         "credentials":[{"id":"ordinary-key","enabled":enabled,"api_key":"synthetic-ordinary"}]}
    ]}))
    .unwrap()
}

async fn check_skip(policy_denied: bool) {
    let state =
        make_state_with_route_store(RouteConfigStore::from_document(document(true)).unwrap());
    state.redis_pool.close();
    let mut ctx = PipelineContext::new(make_request(), None);
    run(&mut ctx, &state).await.unwrap();
    assert_eq!(ctx.candidates.len(), 2);
    ctx.candidates
        .sort_by_key(|c| c.provider_account_id != "zdr");
    // Candidates are already selected; eligibility must observe this newer dispatch state.
    state
        .route_config
        .replace_document(document(false))
        .unwrap();
    assert_eq!(
        state
            .route_config
            .snapshot()
            .credential_dispatch_allowed("zdr", Some("private")),
        Some(false)
    );
    if policy_denied {
        ctx.route_policy_config = Some(crate::db::GatewayRoutePolicyConfig {
            allowed_provider_account_ids: Some(vec!["ordinary".into()]),
            ..Default::default()
        });
    }
    let error = tokio::time::timeout(
        std::time::Duration::from_secs(5),
        Box::pin(crate::pipeline::stage_send::run(&mut ctx, &state)),
    )
    .await
    .unwrap()
    .err()
    .unwrap();
    assert_eq!(ctx.route_attempt_count(), 0);
    if policy_denied {
        assert!(!ctx.request_headers.contains_key("x-cmd-zdr"));
        assert_eq!(error.code.as_deref(), Some("provider_not_authorized"));
    } else {
        assert_eq!(
            ctx.request_headers.get("x-cmd-zdr").map(String::as_str),
            Some("1")
        );
        assert_eq!(error.code.as_deref(), Some("invalid_commandcode_zdr"));
    }
}

#[tokio::test]
async fn commandcode_zdr_eligibility_skip_cannot_downgrade_the_next_candidate() {
    check_skip(false).await;
}

#[tokio::test]
async fn commandcode_zdr_policy_denied_candidate_does_not_latch() {
    check_skip(true).await;
}
