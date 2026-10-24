//! Explicit API component selectors: a shared wire shape does not imply shared product scope.
use super::{HealthAdapter, Observation};
use serde_json::Value;

pub(super) fn parse(adapter: HealthAdapter, body: &Value) -> Option<Vec<Observation>> {
    let names: &[&str] = match adapter {
        HealthAdapter::OpenaiStatus => &["Codex API"],
        HealthAdapter::AnthropicStatus => &["Claude API (api.anthropic.com)", "Claude Code"],
        HealthAdapter::NvidiaStatus => &["NVIDIA Build", "Cloud Functions"],
        HealthAdapter::CohereStatus => &["All Endpoints"],
        HealthAdapter::GroqStatus | HealthAdapter::PerplexityStatus => &["API"],
        HealthAdapter::PoeStatus => &["api.poe.com"],
        HealthAdapter::LinkupStatus => &["Search API", "Fetch API"],
        HealthAdapter::JinaStatus => &["r.jina.ai", "s.jina.ai"],
        HealthAdapter::MistralStatus => &["Completion API"],
        HealthAdapter::TavilyStatus => &["Tavily API Service"],
        HealthAdapter::YouStatus => &[
            "APIs - Search/Index",
            "APIs - Live News",
            "APIs - Agentic Search",
        ],
        _ => return None,
    };
    let rows = body["components"].as_array().filter(|r| r.len() <= 512)?;
    let mut out = Vec::new();
    for row in rows {
        let name = row["name"]
            .as_str()
            .filter(|v| !v.is_empty() && v.len() <= 256)?;
        if !names.contains(&name) {
            continue;
        }
        let status = row["status"]
            .as_str()
            .filter(|v| !v.is_empty() && v.len() <= 64)?;
        out.push(Observation {
            name: name.into(),
            status: status.into(),
            model: None,
        });
    }
    Some(out)
}

/// Exa v3 uses uppercase states and grouped components rather than Statuspage's schema.
pub(super) fn exa(body: &Value) -> Option<Vec<Observation>> {
    let rows = body["components"].as_array().filter(|r| r.len() <= 512)?;
    let mut out = Vec::new();
    for row in rows {
        let name = row["name"]
            .as_str()
            .filter(|v| !v.is_empty() && v.len() <= 256)?;
        let group = &row["group"];
        // Keep the parent identity so another group's "Default" cannot become Search API health.
        let selected = (name == "Search API" && group.is_null())
            || (group["id"] == "cmni631rn07mnhouiasltgt5q" && group["name"] == "Search API");
        if !selected {
            continue;
        }
        let status = row["status"]
            .as_str()
            .filter(|v| !v.is_empty() && v.len() <= 64)?;
        out.push(Observation {
            name: if group.is_null() {
                name.into()
            } else {
                format!("Search API / {name}")
            },
            // Only the observed documented healthy spelling is normalized; future states stay unknown.
            status: if status == "OPERATIONAL" {
                "operational"
            } else {
                "unknown"
            }
            .into(),
            model: None,
        });
    }
    Some(out)
}
