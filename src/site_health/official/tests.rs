use super::{gemini, parsing, HealthAdapter};
use serde_json::json;

#[test]
fn statuspage_filters_products_without_percentages_or_model_fanout() {
    let body = json!({"components":[
        {"name":"Responses","status":"major_outage"},
        {"name":"Codex API","status":"operational"},
        {"name":"NVIDIA Build","status":"degraded_performance"},
        {"name":"NGC Catalog","status":"operational"},
        {"name":"Claude API (api.anthropic.com)","status":"future_state"}]});
    let openai = parsing::parse(HealthAdapter::OpenaiStatus, &body).unwrap();
    assert_eq!(openai.len(), 1);
    assert_eq!(openai[0].name, "Codex API");
    assert!(openai[0].model.is_none());
    let nvidia = parsing::parse(HealthAdapter::NvidiaStatus, &body).unwrap();
    assert_eq!(nvidia.len(), 1);
    assert_eq!(nvidia[0].status, "degraded_performance");
    assert!(parsing::parse(HealthAdapter::AnthropicStatus, &json!({"ok":true})).is_none());
}

#[test]
fn datadog_reads_gateway_leaf_components_not_website_health() {
    let body = json!({"components":[{"name":"API - Gateway","components":[
        {"name":"Chat (/api/v1/chat/completions)","status":"degraded"}]},
        {"name":"Web & Application Services","status":"major_outage"}]});
    let rows = parsing::parse(HealthAdapter::OpenrouterStatus, &body).unwrap();
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].status, "degraded_performance");
    assert!(rows[0].model.is_none());
}

#[test]
fn flashduty_active_changes_are_scoped_and_schema_drift_is_unknown() {
    let mut body = json!({"data":{"page":{"custom_domain":"status.deepseek.com","components":[
        {"component_id":"a","name":"DeepSeek model API"}, {"component_id":"web","name":"Chatservice"}]},
        "active_changes":[]}});
    fn parse(value: &serde_json::Value) -> Vec<super::Observation> {
        parsing::parse(HealthAdapter::DeepseekStatus, value).unwrap()
    }
    assert_eq!(parse(&body)[0].status, "operational");
    body["data"]["active_changes"] =
        json!([{"affected_components":[{"component_id":"web","status":"full_outage"}]}]);
    assert_eq!(parse(&body)[0].status, "operational");
    body["data"]["active_changes"] =
        json!([{"affected_components":[{"component_id":"a","status":"partial_outage"}]}]);
    assert_eq!(parse(&body)[0].status, "partial_outage");
    body["data"]["active_changes"] = json!([
        {"affected_components":[{"component_id":"a","status":"full_outage"}]},
        {"affected_components":[{"component_id":"a","status":"future"}]},
        {"affected_components":[{"component_id":"a","status":"degraded"}]}]);
    assert_eq!(parse(&body)[0].status, "major_outage");
    body["data"]["active_changes"] = json!([{"unexpected":true}]);
    assert_eq!(parse(&body)[0].status, "unknown");
    body["data"]["active_changes"] = json!(null);
    assert!(parsing::parse(HealthAdapter::DeepseekStatus, &body).is_none());
}

#[test]
fn deepseek_exact_component_identity_maps_only_documented_model_ids() {
    let body = json!({"data":{"page":{"custom_domain":"status.deepseek.com","components":[
        {"component_id":"01KY4MVS8BSBSVW6053QJ37RJE","name":"DeepSeek V4.1 Flash API服务(API Service)"}]},"active_changes":[]}});
    let rows = parsing::parse(HealthAdapter::DeepseekStatus, &body).unwrap();
    assert_eq!(rows.len(), 3);
    assert_eq!(rows[0].model.as_deref(), Some("deepseek-flash"));
    assert!(!rows
        .iter()
        .any(|o| o.model.as_deref() == Some("deepseek-v4.1-flash")));
}

#[test]
fn gemini_resolved_history_and_ui_incidents_do_not_block_api() {
    let body = json!([[[
        ["ui", "AI Studio", 1, [[1, "", ["300"], "outage"]], 3, [3]],
        [
            "api",
            "Gemini",
            1,
            [[4, "", ["200"], "resolved"], [1, "", ["100"], "start"]],
            1,
            [1]
        ]
    ]]]);
    assert_eq!(gemini::parse(&body).unwrap()[0].status, "operational");
    let body = json!([[[["api", "Gemini", 1, [[2, "", ["200"], "identified"]], 1, [1]]]]]);
    assert_eq!(
        gemini::parse(&body).unwrap()[0].status,
        "degraded_performance"
    );
    assert!(gemini::parse(&json!({"components":[]})).is_none());
    assert!(gemini::parse(&json!([[[["malformed"]]]])).is_none());
}
