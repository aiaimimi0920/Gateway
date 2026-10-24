//! Matrix rates are fractions; zero redacted volumes are not evidence of absent samples.
use super::super::ProbeCoverage;
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};

pub(super) fn coverage(body: &Value) -> Option<ProbeCoverage> {
    let c = body["data"]["coverage"].as_object()?;
    Some(ProbeCoverage {
        complete: c.get("coverage_complete").and_then(Value::as_bool),
        aggregation_lag_secs: c.get("aggregation_lag_seconds").and_then(Value::as_u64),
        computed_at: c
            .get("computed_at")
            .and_then(Value::as_str)
            .filter(|s| s.len() <= 64)
            .map(str::to_owned),
    })
}
pub(super) fn parse(body: &Value, groups: &[String]) -> Option<BTreeMap<String, Option<f64>>> {
    if body["code"].as_i64() != Some(0) || body["data"]["group_by"] != "platform_group_model" {
        return None;
    }
    let rows = body["data"]["items"]
        .as_array()
        .filter(|r| r.len() <= 4096)?;
    let mut models: BTreeMap<String, (Option<f64>, BTreeSet<String>)> = BTreeMap::new();
    for row in rows {
        let group = row["group_id"].as_i64().filter(|g| *g > 0)?.to_string();
        if !groups.is_empty() && !groups.contains(&group) {
            continue;
        }
        let name = row["model"]
            .as_str()
            .filter(|s| !s.is_empty() && s.len() <= 256)?;
        // Unknown means insufficient evidence, even if an empty metric serialized as zero.
        let rate = if !matches!(
            row["health"]["overall"].as_str(),
            Some("healthy" | "warning" | "critical")
        ) {
            None
        } else {
            row["metrics"]["success_rate"]
                .as_f64()
                .filter(|r| r.is_finite() && (0.0..=1.0).contains(r))
                .map(|r| r * 100.0)
        };
        let entry = models
            .entry(name.into())
            .or_insert((Some(100.0), BTreeSet::new()));
        entry.0 = match (entry.0, rate) {
            (Some(a), Some(b)) => Some(a.min(b)),
            _ => None,
        };
        entry.1.insert(group);
    }
    Some(
        models
            .into_iter()
            .map(|(m, (rate, seen))| {
                (
                    m,
                    if groups.iter().all(|g| seen.contains(g)) {
                        rate
                    } else {
                        None
                    },
                )
            })
            .collect(),
    )
}
