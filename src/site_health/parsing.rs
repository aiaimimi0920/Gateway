//! Template semantics: no samples, partial rows and duplicate IDs are never healthy evidence.
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};

pub(super) fn parse(
    template: &str,
    body: &Value,
    groups: &[String],
) -> Option<BTreeMap<String, Option<f64>>> {
    if body["success"] != true {
        return None;
    }
    if template == "model-health-slot" && body["data"]["enabled"] != true {
        return Some(BTreeMap::new());
    }
    let rows = if matches!(template, "new-api-perf-summary" | "model-health-slot") {
        body["data"]["models"].as_array()?
    } else {
        body["data"].as_array()?
    };
    if rows.len() > 4096 {
        return None;
    }
    if template == "channel-health" {
        return channel_rates(rows);
    }
    let mut out = BTreeMap::new();
    let mut seen: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    for row in rows {
        if template == "model-status-all" && !groups.is_empty() {
            let group = row["group_name"]
                .as_str()
                .or_else(|| row["group"].as_str())
                .unwrap_or("");
            if !groups.iter().any(|g| g == group) {
                continue;
            }
        }
        let name = row["model_name"]
            .as_str()
            .filter(|s| !s.is_empty() && s.len() <= 256)?;
        let mut rate = row["success_rate"]
            .as_f64()
            .filter(|n| n.is_finite() && (0.0..=100.0).contains(n));
        if template == "model-status-all" && !groups.is_empty() {
            let group = row["group_name"]
                .as_str()
                .or_else(|| row["group"].as_str())?;
            seen.entry(name.into()).or_default().insert(group.into());
        }
        if matches!(template, "model-status-all" | "model-health-slot")
            && (row["total_requests"].as_u64().unwrap_or(0) == 0 || row["current_status"] == "blue")
        {
            rate = None;
        }
        if out.contains_key(name) {
            out.insert(name.to_owned(), None);
        } else {
            out.insert(name.to_owned(), rate);
        }
    }
    if template == "model-status-all" && !groups.is_empty() {
        for (model, rate) in &mut out {
            if !groups
                .iter()
                .all(|g| seen.get(model).is_some_and(|s| s.contains(g)))
            {
                *rate = None;
            }
        }
    }
    Some(out)
}

fn channel_rates(rows: &[Value]) -> Option<BTreeMap<String, Option<f64>>> {
    let mut counts: BTreeMap<String, [f64; 4]> = BTreeMap::new();
    for row in rows {
        let name = row["model_name"]
            .as_str()
            .filter(|s| !s.trim().is_empty() && s.len() <= 256)?;
        let mut values = [0.0; 4];
        for (i, key) in [
            "test_count",
            "success_count",
            "client_error_count",
            "moderation_count",
        ]
        .iter()
        .enumerate()
        {
            if i >= 2 && row.get(key).is_none() {
                continue;
            }
            values[i] = row[key]
                .as_f64()
                .filter(|n| n.is_finite() && *n >= 0.0 && *n <= 1e15 && n.fract() == 0.0)?;
        }
        if values[1] + values[2] + values[3] > values[0] {
            return None;
        }
        let sum = counts.entry(name.to_owned()).or_default();
        for i in 0..4 {
            sum[i] += values[i];
        }
    }
    Some(
        counts
            .into_iter()
            .map(|(name, [total, success, client, moderation])| {
                (
                    name,
                    (total > client).then(|| (success + moderation) / (total - client) * 100.0),
                )
            })
            .collect(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn channels_aggregate_counts_and_allow_missing_optional_counts() {
        let data = json!({"success":true,"data":[{"model_name":"m","test_count":10,"success_count":8},
            {"model_name":"m","test_count":10,"success_count":7,"client_error_count":2,"moderation_count":1}]});
        let rates = parse("channel-health", &data, &[]).unwrap();
        assert!((rates["m"].unwrap() - 16.0 / 18.0 * 100.0).abs() < 0.0001);
        for invalid in [json!(-1), json!(1.5), json!(1e16), json!("10")] {
            let mut data = data.clone();
            data["data"][0]["test_count"] = invalid;
            assert!(parse("channel-health", &data, &[]).is_none());
        }
    }

    #[test]
    fn no_samples_duplicates_out_of_range_and_groups_are_not_healthy() {
        let data = json!({"success":true,"data":[{"model_name":"m","success_rate":99,"total_requests":0},
            {"model_name":"off","success_rate":99,"total_requests":4,"current_status":"blue"},
            {"model_name":"duplicate","success_rate":99,"total_requests":4},
            {"model_name":"duplicate","success_rate":99,"total_requests":4},
            {"model_name":"invalid","success_rate":101,"total_requests":4}]});
        assert!(parse("model-status-all", &data, &[])
            .unwrap()
            .values()
            .all(Option::is_none));
        assert!(parse("model-status-all", &data, &["paid".into()])
            .unwrap()
            .is_empty());
        assert!(parse("new-api-perf-summary", &json!({"success":false}), &[]).is_none());
    }
}
