//! Waiting uses the original request clock, not a new deadline on every retry.
use neuro_gateway::pipeline::request_budget::RequestBudget;
use neuro_gateway::protocol::responses::normalize_responses;
use std::time::Duration;

#[tokio::test(start_paused = true)]
async fn configured_fifteen_minutes_really_extends_the_default_five() {
    let req = normalize_responses(serde_json::json!({"model":"fixture", "input":"hello"})).unwrap();
    let budget = RequestBudget::for_request(&req);
    budget.configure(&req, Some(900)).unwrap();
    tokio::time::advance(Duration::from_secs(600)).await;
    assert!(budget.check().is_ok());
    tokio::time::advance(Duration::from_secs(301)).await;
    assert!(budget.check().is_err());
}

#[test]
fn route_policy_accepts_ten_and_fifteen_minutes_but_not_sixteen() {
    for seconds in [300, 600, 900] {
        let input =
            serde_json::from_value(serde_json::json!({"totalRequestTimeoutSeconds": seconds}))
                .unwrap();
        assert!(neuro_gateway::db::normalize_route_policy_config(input).is_ok());
    }
    let input =
        serde_json::from_value(serde_json::json!({"totalRequestTimeoutSeconds": 960})).unwrap();
    assert!(neuro_gateway::db::normalize_route_policy_config(input).is_err());
}
