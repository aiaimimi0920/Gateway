//! New official catalog entries must stay scoped, bounded, and categorical.
use super::{parsing, HealthAdapter};
use serde_json::json;

#[test]
fn additional_statuspage_selectors_exclude_websites_and_never_fan_out_to_models() {
    for (adapter, names) in [
        (HealthAdapter::CohereStatus, vec!["All Endpoints"]),
        (HealthAdapter::GroqStatus, vec!["API"]),
        (HealthAdapter::PoeStatus, vec!["api.poe.com"]),
        (HealthAdapter::PerplexityStatus, vec!["API"]),
        (HealthAdapter::LinkupStatus, vec!["Search API", "Fetch API"]),
        (HealthAdapter::JinaStatus, vec!["r.jina.ai", "s.jina.ai"]),
        (HealthAdapter::MistralStatus, vec!["Completion API"]),
        (HealthAdapter::TavilyStatus, vec!["Tavily API Service"]),
        (
            HealthAdapter::YouStatus,
            vec![
                "APIs - Search/Index",
                "APIs - Live News",
                "APIs - Agentic Search",
            ],
        ),
    ] {
        let mut components = vec![json!({"name":"Website","status":"major_outage"})];
        for name in &names {
            components.push(json!({"name":name,"status":"operational"}));
        }
        let body = json!({"components":components});
        let rows = parsing::parse(adapter, &body).unwrap();
        assert_eq!(rows.len(), names.len(), "{}", adapter.id());
        assert!(rows
            .iter()
            .all(|o| o.status == "operational" && o.model.is_none()));
        let drift = json!({"components":[{"name":names[0],"status":"future-state"}]});
        assert_eq!(
            parsing::parse(adapter, &drift).unwrap()[0].status,
            "future-state"
        );
        assert!(parsing::parse(adapter, &json!({"components":null})).is_none());
        assert!(parsing::parse(adapter, &json!({"components":vec![json!({});513]})).is_none());
        assert!(parsing::parse(adapter, &json!({"components":[{"name":names[0]}]})).is_none());
        assert!(parsing::parse(
            adapter,
            &json!({"components":[{"name":"Website","status":"operational"}]})
        )
        .unwrap()
        .is_empty());
    }
}

#[test]
fn exa_v3_keeps_search_group_identity_and_unknown_states() {
    let body = json!({"components":[
        {"name":"Search API","status":"OPERATIONAL","group":null},
        {"name":"Default","status":"OPERATIONAL","group":{"id":"cmni631rn07mnhouiasltgt5q","name":"Search API"}},
        {"name":"People","status":"FUTURE","group":{"id":"cmni631rn07mnhouiasltgt5q","name":"Search API"}},
        {"name":"Default","status":"OPERATIONAL","group":{"id":"other","name":"Search API"}},
        {"name":"Websets","status":"MAJOROUTAGE","group":null}
    ]});
    let rows = parsing::parse(HealthAdapter::ExaStatus, &body).unwrap();
    assert_eq!(rows.len(), 3);
    assert_eq!(rows[0].status, "operational");
    assert_eq!(rows[1].name, "Search API / Default");
    assert_eq!(rows[2].status, "unknown");
    assert!(rows.iter().all(|o| o.model.is_none()));
    assert!(parsing::parse(HealthAdapter::ExaStatus, &json!([])).is_none());
}

#[test]
fn together_resources_normalize_only_documented_states_and_ignore_web_aggregate() {
    for (raw, expected) in [
        ("operational", "operational"),
        ("degraded", "degraded_performance"),
        ("downtime", "major_outage"),
        ("maintenance", "under_maintenance"),
        ("not_monitored", "unknown"),
        ("future", "unknown"),
    ] {
        let body = json!({"data":{"attributes":{"aggregate_state":"downtime"}},"included":[
            {"type":"status_page_section","attributes":{"name":"Models"}},
            {"type":"status_page_resource","attributes":{"public_name":"Website","status":"downtime"}},
            {"type":"status_page_resource","attributes":{"public_name":"Playground","status":"downtime"}},
            {"type":"status_page_resource","attributes":{"public_name":"Model label","status":raw,"availability":1.0}}
        ]});
        let rows = parsing::parse(HealthAdapter::TogetherStatus, &body).unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].status, expected);
        assert!(rows[0].model.is_none());
    }
    assert!(parsing::parse(HealthAdapter::TogetherStatus, &json!({"included":null})).is_none());
    assert!(parsing::parse(
        HealthAdapter::TogetherStatus,
        &json!({"included":vec![json!({});513]})
    )
    .is_none());
}
