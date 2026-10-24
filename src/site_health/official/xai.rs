//! xAI's current product tree is separate from Grok website and incident history.
use super::Observation;
use serde_json::Value;

pub(super) fn parse(body: &Value) -> Option<Vec<Observation>> {
    let products = body["products"].as_array().filter(|v| v.len() <= 64)?;
    let mut out = Vec::new();
    let mut found = false;
    for product in products.iter().filter(|p| p["id"] == "api") {
        if found || product["name"] != "API" {
            return None;
        }
        found = true;
        out.push(observation("xAI API", product["status"].as_str()?)?);
        let components = product["components"].as_array().filter(|v| v.len() <= 64)?;
        for component in components {
            let id = component["id"].as_str()?;
            let name = component["name"]
                .as_str()
                .filter(|v| !v.is_empty() && v.len() <= 200)?;
            if !matches!(id, "api-global" | "api-us" | "api-eu-west-1") {
                continue;
            }
            out.push(observation(
                &format!("xAI API / {name}"),
                component["status"].as_str()?,
            )?);
        }
    }
    Some(out)
}

fn observation(name: &str, status: &str) -> Option<Observation> {
    if status.is_empty() || status.len() > 64 {
        return None;
    }
    // Only the healthy value is verified for this custom contract. Never guess new states healthy.
    Some(Observation {
        name: name.into(),
        model: None,
        status: if status == "available" {
            "operational"
        } else {
            "unknown"
        }
        .into(),
    })
}
