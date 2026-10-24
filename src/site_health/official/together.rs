//! Better Stack resource states, not the whole-page aggregate or percentage availability.
use super::Observation;
use serde_json::Value;

pub(super) fn parse(body: &Value) -> Option<Vec<Observation>> {
    let rows = body["included"].as_array().filter(|r| r.len() <= 512)?;
    let mut out = Vec::new();
    for row in rows {
        if row["type"] != "status_page_resource" {
            continue;
        }
        let attrs = &row["attributes"];
        let name = attrs["public_name"]
            .as_str()
            .filter(|v| !v.is_empty() && v.len() <= 256)?;
        if matches!(name, "Website" | "Playground") {
            continue;
        }
        let status = match attrs["status"].as_str()? {
            "operational" => "operational",
            "degraded" => "degraded_performance",
            "downtime" => "major_outage",
            "maintenance" => "under_maintenance",
            _ => "unknown",
        };
        // Public labels are not guaranteed to be API model IDs. No implicit model fan-out.
        out.push(Observation {
            name: name.into(),
            status: status.into(),
            model: None,
        });
    }
    Some(out)
}
