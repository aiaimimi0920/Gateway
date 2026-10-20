//! Finite HTTP recipes; successful schemas are detected automatically, not chosen by the user.
use super::{parsing, transport, ProbeResult, SiteHealthConfig, SiteSnapshot};
use serde_json::{json, Value};
use std::time::Duration;

pub async fn discover(
    provider_id: &str,
    base: &str,
    config: &SiteHealthConfig,
) -> anyhow::Result<SiteSnapshot> {
    config.validate(base)?;
    tokio::time::timeout(Duration::from_secs(25), run(provider_id, base, config))
        .await
        .map_err(|_| anyhow::anyhow!("Site health detection timed out"))?
}

async fn run(id: &str, base: &str, config: &SiteHealthConfig) -> anyhow::Result<SiteSnapshot> {
    let origin = transport::checked_url(config.site_url.as_deref().unwrap_or(base))?;
    let client = transport::client(&origin, config.allow_private_network).await?;
    let recipes = [
        ("new-api-perf-summary", "api/perf-metrics/summary?hours=24"),
        (
            "model-status-all",
            "api/enhancements/model-status/embed/status/all",
        ),
        ("channel-health", "api/channel/health/models?days=1"),
        (
            "model-status-embed",
            "api/model-status/embed/config/selected",
        ),
        ("model-health-slot", "api/model_health/slot_status"),
    ];
    let mut probes = Vec::with_capacity(recipes.len());
    for pair in recipes.chunks(2) {
        let (template, path) = pair[0];
        if let Some(&(other, other_path)) = pair.get(1) {
            let (first, second) = tokio::join!(
                probe(&client, &origin, template, path, config),
                probe(&client, &origin, other, other_path, config)
            );
            probes.extend([first, second]);
        } else {
            probes.push(probe(&client, &origin, template, path, config).await);
        }
    }
    // Prefer a valid source with the most scoped model evidence; stable recipe order breaks ties.
    let selected = probes
        .iter()
        .rev()
        .filter(|p| p.status == "supported")
        .max_by_key(|p| p.models.values().filter(|v| v.is_some()).count());
    let selected_template = selected.map(|p| p.template.clone());
    let models = selected.map(|p| p.models.clone()).unwrap_or_default();
    // Keep one bounded model table per site, not five redundant copies in the long-lived cache.
    for probe in &mut probes {
        probe.model_count = probe.models.len();
        probe.models.clear();
    }
    Ok(SiteSnapshot {
        provider_id: id.into(),
        checked_at: super::now(),
        binding: config.binding(base),
        selected_template,
        models,
        probes,
    })
}

async fn probe(
    client: &rquest::Client,
    origin: &url::Url,
    template: &str,
    path: &str,
    config: &SiteHealthConfig,
) -> ProbeResult {
    let mut endpoint = origin.join(path).expect("fixed relative recipe");
    let mut result = ProbeResult {
        template: template.into(),
        endpoint: endpoint.to_string(),
        status: "invalid_response".into(),
        http_status: None,
        model_count: 0,
        models: Default::default(),
    };
    if template == "model-health-slot" && config.monitor_groups.is_empty() {
        result.status = "parameters_required".into();
        return result;
    }
    let operation = async {
        if template == "model-status-embed" {
            let selection = transport::json(client, endpoint.as_str(), None).await?;
            if selection["success"] != true {
                return Err(("invalid_response".into(), Some(200)));
            }
            let models = selection["data"]
                .as_array()
                .filter(|m| {
                    m.len() <= 4096
                        && m.iter()
                            .all(|m| m.as_str().is_some_and(|m| !m.is_empty() && m.len() <= 256))
                })
                .ok_or_else(|| ("invalid_response".into(), Some(200)))?;
            if models.is_empty() {
                return Ok(Default::default());
            }
            let selected: std::collections::BTreeSet<_> =
                models.iter().filter_map(Value::as_str).collect();
            let window = selection["time_window"]
                .as_str()
                .filter(|s| !s.is_empty() && s.len() <= 32)
                .unwrap_or("24h");
            endpoint = origin.join("api/model-status/embed/status/batch").unwrap();
            endpoint.query_pairs_mut().append_pair("window", window);
            let body = transport::json(client, endpoint.as_str(), Some(&json!(selected))).await?;
            let mut rates = parsing::parse(template, &body, &config.monitor_groups)
                .ok_or_else(|| ("invalid_response".into(), Some(200)))?;
            rates.retain(|model, _| selected.contains(model.as_str()));
            return Ok(rates);
        }
        if template == "model-health-slot" {
            let mut combined = std::collections::BTreeMap::new();
            for group in &config.monitor_groups {
                endpoint.set_query(None);
                endpoint
                    .query_pairs_mut()
                    .append_pair("window", "6h")
                    .append_pair("group", group);
                let body = transport::json(client, endpoint.as_str(), None).await?;
                if body["data"]["group"] != Value::String(group.clone()) {
                    return Err(("group_mismatch".into(), Some(200)));
                }
                let models = parsing::parse(template, &body, &[])
                    .ok_or_else(|| ("invalid_response".into(), Some(200)))?;
                for (model, rate) in models {
                    combined
                        .entry(model)
                        .and_modify(|old: &mut Option<f64>| {
                            *old = match (*old, rate) {
                                (Some(a), Some(b)) => Some(a.min(b)),
                                _ => None,
                            }
                        })
                        .or_insert(rate);
                }
            }
            return Ok(combined);
        }
        let body = transport::json(client, endpoint.as_str(), None).await?;
        parsing::parse(template, &body, &config.monitor_groups)
            .ok_or_else(|| ("invalid_response".into(), Some(200)))
    };
    match operation.await {
        Ok(models) => {
            result.status = if models.is_empty() {
                "no_samples"
            } else {
                "supported"
            }
            .into();
            result.models = models;
            result.http_status = Some(200);
        }
        Err((status, http)) => {
            result.status = status;
            result.http_status = http;
        }
    }
    result.endpoint = endpoint.to_string();
    result
}
