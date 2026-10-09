use super::*;
use serde_json::json;

fn plan() -> CredentialTestPolicy {
    serde_json::from_value(json!({"modelSelection":"selected","models":["a"],"cases":[
        {"id":"one","name":"one","prompt":"Reply OK","expectedAnswer":"OK","difficulty":1,"enabled":true}
    ]})).unwrap()
}

#[test]
fn rejects_unbounded_ambiguous_and_unselected_inputs() {
    let base = plan();
    assert!(base.validate().is_ok());
    let mut invalid = base.clone();
    invalid.models = vec!["a".into(), "a".into()];
    assert!(invalid.validate().is_err());
    invalid = base.clone();
    invalid.models = vec!["*".into()];
    assert!(invalid.validate().is_err());
    invalid = base.clone();
    invalid.models = vec!["a\n".into()];
    assert!(invalid.validate().is_err());
    invalid = base.clone();
    invalid.cases[0].enabled = false;
    assert!(invalid.validate().is_err());
    invalid = base.clone();
    invalid.cases[0].prompt = "中".repeat(3000);
    assert!(invalid.validate().is_err());
    invalid = base.clone();
    invalid.cases[0].difficulty = 4;
    assert!(invalid.validate().is_err());
    invalid = base.clone();
    invalid.interval_minutes = 0;
    assert!(invalid.validate().is_err());
    invalid = base.clone();
    invalid.cases = vec![base.cases[0].clone(); 17];
    assert!(invalid.validate().is_err());
    invalid = base;
    invalid.models = (0..129).map(|index| format!("model-{index}")).collect();
    assert!(invalid.validate().is_err());
}
