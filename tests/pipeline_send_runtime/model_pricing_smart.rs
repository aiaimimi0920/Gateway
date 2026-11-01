//! Pricing failures penalize availability and retain the existing smart-sorted queue.
use super::fixture::{candidate, context, TestState, Upstream, DEADLINE, JSON_REPLY};
use super::model_pricing::{with_tools, PRICING_BODY};
use axum::http::StatusCode;
use neuro_gateway::pipeline::{stage_route, stage_send};
use tokio::time::timeout;

#[tokio::test]
async fn model_pricing_fallback_uses_smart_sorted_authorized_candidates() {
    let rejected = Upstream::start(StatusCode::BAD_REQUEST, false, PRICING_BODY).await;
    let lower_score = Upstream::start(StatusCode::BAD_REQUEST, false, PRICING_BODY).await;
    let healthy = Upstream::start(StatusCode::OK, false, JSON_REPLY).await;
    let fixture = TestState::new();
    fixture.state.route_config.replace_document(serde_json::from_value(serde_json::json!({
        "providers":[
            {"id":"first","preset":"openai","base_url":rejected.base_url,"api_key":"send-test-token","supported_models":["caller-alias"]},
            {"id":"lower-score","preset":"openai","base_url":lower_score.base_url,"api_key":"send-test-token","supported_models":["caller-alias"]},
            {"id":"healthy","preset":"openai","base_url":healthy.base_url,"api_key":"send-test-token","supported_models":["caller-alias"]}
        ],
        "model_routes":[
            {"pattern":"caller-alias","provider_ids":["first"],"priority":100},
            {"pattern":"caller-alias","provider_ids":["lower-score"],"priority":90},
            {"pattern":"caller-alias","provider_ids":["healthy"],"priority":10}
        ]
    })).unwrap()).unwrap();
    // The same failure feedback as live sends moves 100 -> 85 -> 70 (lower tier).
    for _ in 0..2 {
        let mut selected = candidate(&lower_score, "lower-score");
        selected.provider_account_id = "lower-score".into();
        let mut ctx = context(false, vec![selected]);
        with_tools(&mut ctx);
        assert!(stage_send::run(&mut ctx, &fixture.state).await.is_err());
    }
    let mut ctx = context(false, Vec::new());
    with_tools(&mut ctx);
    ctx.request_headers.insert(
        "x-gateway-routing-order".into(),
        "availability,intelligence,price".into(),
    );
    Box::pin(stage_route::run(&mut ctx, &fixture.state))
        .await
        .unwrap();
    assert_eq!(
        ctx.candidates
            .iter()
            .map(|c| c.provider_account_id.as_str())
            .collect::<Vec<_>>(),
        vec!["first", "healthy", "lower-score"]
    );
    assert!(timeout(DEADLINE, stage_send::run(&mut ctx, &fixture.state))
        .await
        .unwrap()
        .is_ok());
    assert_eq!(rejected.requests().len(), 1);
    assert_eq!(healthy.requests().len(), 1);
    assert_eq!(
        lower_score.requests().len(),
        2,
        "must not send again to lower tier"
    );
    assert_eq!(ctx.selected_provider_id.as_deref(), Some("healthy"));
    rejected.finish().await;
    lower_score.finish().await;
    healthy.finish().await;
    fixture.finish().await;
}
