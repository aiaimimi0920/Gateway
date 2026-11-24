//! Multi-account membership remains provider-local, bounded, distinct and homogeneous.
use neuro_gateway::console::document::validate_route_document;
use serde_json::{json, Value};

fn document(scopes: Value, count: usize) -> Value {
    json!({"providers":[{"id":"pool","preset":"openai","base_url":"http://127.0.0.1:9",
    "credentials":(0..count).map(|index| json!({"id":format!("a{index}"),"api_key":"synthetic"})).collect::<Vec<_>>(),
    "test_plans":[{"id":"multi","name":"Multi account","scopes":scopes,"policy":{
        "automaticEnabled":false,"intervalMinutes":60,"modelSelection":"all","models":[],
        "cases":[{"id":"ok","name":"OK","prompt":"Reply OK","difficulty":1,"enabled":true}]
    }}]}]})
}

#[test]
fn accepts_multiple_accounts_and_keeps_membership_guards() {
    for count in [1, 2, 128] {
        let scopes = json!((0..count)
            .map(|index| json!({"kind":"account","id":format!("a{index}")}))
            .collect::<Vec<_>>());
        assert!(
            validate_route_document(serde_json::from_value(document(scopes, count)).unwrap())
                .is_ok()
        );
    }
    for scopes in [
        json!([]),
        json!([{"kind":"account","id":"a0"},{"kind":"account","id":"a0"}]),
        json!([{"kind":"account","id":"a0"},{"kind":"account","id":"foreign"}]),
        json!([{"kind":"pool"},{"kind":"account","id":"a0"}]),
        json!([{"kind":"pool"},{"kind":"pool"}]),
        json!((0..129)
            .map(|index| json!({"kind":"account","id":format!("a{index}")}))
            .collect::<Vec<_>>()),
    ] {
        assert!(
            validate_route_document(serde_json::from_value(document(scopes, 129)).unwrap())
                .is_err()
        );
    }
    assert!(validate_route_document(
        serde_json::from_value(document(json!([{"kind":"pool"}]), 2)).unwrap()
    )
    .is_ok());
}
