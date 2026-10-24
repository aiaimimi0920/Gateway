//! New API summary is aggregate evidence; explicit remote groups require bounded exact queries.
use super::*;
use std::collections::BTreeMap;
type Result<T> = std::result::Result<T, (String, Option<u16>)>;
const MAX_DETAILS: usize = 16;

pub(super) async fn probe(
    client: &rquest::Client,
    origin: &url::Url,
    config: &ProbePolicy,
    coverage: &mut Option<super::super::ProbeCoverage>,
) -> Result<BTreeMap<String, Option<f64>>> {
    let endpoint = origin.join("api/perf-metrics/summary?hours=24").unwrap();
    let body = transport::json(client, endpoint.as_str(), None, &config.auth).await?;
    let mut models = parsing::parse("new-api-perf-summary", &body, &[])
        .ok_or_else(|| ("invalid_response".into(), Some(200)))?;
    if config.monitor_groups.is_empty() {
        return Ok(models);
    }
    // Keep unqueried models unknown instead of silently reusing the unscoped summary.
    for rate in models.values_mut() {
        *rate = None;
    }
    let limit = MAX_DETAILS / config.monitor_groups.len();
    *coverage = Some(super::super::ProbeCoverage {
        complete: Some(models.len() <= limit),
        aggregation_lag_secs: None,
        computed_at: None,
    });
    for (model, rate) in models.iter_mut().take(limit) {
        let mut scoped = Some(100.0_f64);
        for group in &config.monitor_groups {
            let mut endpoint = origin.join("api/perf-metrics").unwrap();
            endpoint
                .query_pairs_mut()
                .append_pair("model", model)
                .append_pair("group", group)
                .append_pair("hours", "24");
            let body = transport::json(client, endpoint.as_str(), None, &config.auth).await?;
            let group_rate = detail_rate(&body, model, group)?;
            scoped = match (scoped, group_rate) {
                (Some(a), Some(b)) => Some(a.min(b)),
                _ => None,
            };
        }
        *rate = scoped;
    }
    Ok(models)
}

fn detail_rate(body: &Value, model: &str, group: &str) -> Result<Option<f64>> {
    if body["success"] != true || body["data"]["model_name"].as_str() != Some(model) {
        return Err(("group_mismatch".into(), Some(200)));
    }
    let rows = body["data"]["groups"]
        .as_array()
        .filter(|r| r.len() <= 4096)
        .ok_or_else(|| ("invalid_response".into(), Some(200)))?;
    let mut found = rows.iter().filter(|r| r["group"].as_str() == Some(group));
    let rate = found
        .next()
        .and_then(|r| r["success_rate"].as_f64())
        .filter(|r| r.is_finite() && (0.0..=100.0).contains(r));
    Ok(if found.next().is_some() { None } else { rate })
}
