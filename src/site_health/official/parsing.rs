//! Finite official schemas. Unrecognized states remain unknown rather than healthy.
use super::{HealthAdapter, Observation};
use serde_json::Value;

pub(super) fn parse(adapter: HealthAdapter, body: &Value) -> Option<Vec<Observation>> {
    if adapter == HealthAdapter::XaiStatus {
        return super::xai::parse(body);
    }
    if adapter == HealthAdapter::QwenStatus {
        return super::aliyun::parse(body);
    }
    if adapter == HealthAdapter::DeepseekStatus {
        return deepseek(body);
    }
    if adapter == HealthAdapter::OpenrouterStatus {
        return openrouter(body);
    }
    if adapter == HealthAdapter::GeminiStatus {
        return super::gemini::parse(body);
    }
    if adapter == HealthAdapter::ExaStatus {
        return super::statuspage::exa(body);
    }
    if adapter == HealthAdapter::TogetherStatus {
        return super::together::parse(body);
    }
    super::statuspage::parse(adapter, body)
}

fn openrouter(body: &Value) -> Option<Vec<Observation>> {
    let mut out = Vec::new();
    let groups = body["components"].as_array().filter(|r| r.len() <= 64)?;
    // The public Datadog feed separates API Gateway from website health.
    for group in groups.iter().filter(|g| g["name"] == "API - Gateway") {
        for row in group["components"].as_array().filter(|r| r.len() <= 64)? {
            let name = row["name"].as_str().filter(|v| v.len() <= 256)?;
            let status = row["status"].as_str().filter(|v| v.len() <= 64)?;
            let status = match status {
                "degraded" => "degraded_performance",
                "outage" => "major_outage",
                other => other,
            };
            out.push(Observation {
                name: name.into(),
                status: status.into(),
                model: None,
            });
        }
    }
    Some(out)
}

fn deepseek(body: &Value) -> Option<Vec<Observation>> {
    let data = &body["data"];
    if data["page"]["custom_domain"] != "status.deepseek.com" {
        return None;
    }
    let rows = data["page"]["components"]
        .as_array()
        .filter(|r| r.len() <= 512)?;
    let changes = data["active_changes"]
        .as_array()
        .filter(|r| r.len() <= 512)?;
    let mut out = Vec::new();
    let mut impacted = std::collections::BTreeMap::new();
    let mut incomplete = false;
    let mut count = 0usize;
    for change in changes {
        let Some(affected) = change["affected_components"].as_array() else {
            incomplete = true;
            continue;
        };
        count += affected.len();
        if count > 4096 {
            return None;
        }
        for component in affected {
            let Some(id) = component["component_id"]
                .as_str()
                .filter(|v| v.len() <= 128)
            else {
                incomplete = true;
                continue;
            };
            let next = match component["status"].as_str() {
                Some("operational") => 0,
                Some("degraded") => 2,
                Some("partial_outage") => 3,
                Some("full_outage") => 4,
                _ => 1,
            };
            let level = impacted.entry(id).or_insert(0);
            *level = (*level).max(next);
        }
    }
    for row in rows {
        let name = row["name"].as_str().filter(|v| v.len() <= 256)?;
        if !name.contains("API") {
            continue;
        }
        let id = row["component_id"].as_str()?;
        let level = impacted
            .get(id)
            .copied()
            .unwrap_or(0)
            .max(usize::from(incomplete));
        let status = [
            "operational",
            "unknown",
            "degraded_performance",
            "partial_outage",
            "major_outage",
        ][level];
        // Explicit identities verified against official API documentation, never guessed from labels.
        let models: &[&str] = match id {
            "01KY4MVS8BM3F9JSYWACGQVG7A" if name == "DeepSeek V4 Pro API服务(API Service)" => {
                &["deepseek-v4-pro"]
            }
            "01KY4MVS8BSBSVW6053QJ37RJE" if name == "DeepSeek V4.1 Flash API服务(API Service)" => {
                &[
                    "deepseek-flash",
                    "deepseek-v4-flash",
                    "deepseek-v4-flash-vision-exp",
                ]
            }
            _ => &[],
        };
        for model in models {
            out.push(Observation {
                name: name.into(),
                model: Some((*model).into()),
                status: status.into(),
            });
        }
        if !models.is_empty() {
            continue;
        }
        out.push(Observation {
            name: name.into(),
            model: None,
            status: status.into(),
        });
    }
    Some(out)
}
