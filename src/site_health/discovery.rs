//! Bounded gateway capabilities. Family selection never broadcasts monitoring secrets.
mod gateways;
mod new_api;
mod readers;
mod sub2api;
use super::{parsing, transport, HealthAdapter, ProbePolicy, ProbeResult, SiteSnapshot};
use serde_json::{json, Value};
use std::time::Duration;

pub async fn discover(
    provider_id: &str,
    base: &str,
    config: &ProbePolicy,
) -> anyhow::Result<SiteSnapshot> {
    anyhow::ensure!(config.is_enabled(), "Site health plan is disabled");
    config.validate(base)?;
    // Azure publishes a ~7 MiB region matrix; do not extend other adapters' existing budget.
    let seconds = if config.adapter == HealthAdapter::AzureStatus {
        35
    } else {
        25
    };
    tokio::time::timeout(Duration::from_secs(seconds), run(provider_id, base, config))
        .await
        .map_err(|_| anyhow::anyhow!("Site health detection timed out"))?
}

async fn run(id: &str, base: &str, config: &ProbePolicy) -> anyhow::Result<SiteSnapshot> {
    if config.adapter.official_url().is_some() {
        return super::official::discover(id, base, config).await;
    }
    let origin = transport::checked_url(config.site_url.as_deref().unwrap_or(base))?;
    let client = transport::client(&origin, config.allow_private_network).await?;
    let (mut probes, selected) = gateways::detect(&client, &origin, config).await;
    let selected = selected.map(|index| &probes[index]);
    let selected_template = selected.map(|p| p.template.clone());
    let models = selected.map(|p| p.models.clone()).unwrap_or_default();
    // Keep one bounded model table per site, not five redundant copies in the long-lived cache.
    for probe in &mut probes {
        probe.model_count = probe.models.len();
        probe.models.clear();
    }
    Ok(SiteSnapshot {
        provider_id: id.into(),
        profile_id: None,
        checked_at: super::now(),
        binding: super::configuration::binding(config, base, None),
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
    config: &ProbePolicy,
) -> ProbeResult {
    let mut endpoint = origin.join(path).expect("fixed relative recipe");
    let mut result = ProbeResult {
        observations: Vec::new(),
        template: template.into(),
        endpoint: endpoint.to_string(),
        status: "invalid_response".into(),
        http_status: None,
        model_count: 0,
        models: Default::default(),
        coverage: None,
    };
    if template == "model-health-slot" && config.monitor_groups.is_empty() {
        result.status = "parameters_required".into();
        return result;
    }
    let operation = async {
        if template == "new-api-perf-summary" {
            return new_api::probe(client, origin, config, &mut result.coverage).await;
        }
        if template == "sub2api-v2" {
            for group in &config.monitor_groups {
                endpoint.query_pairs_mut().append_pair("group_id", group);
            }
            let body = transport::json(client, endpoint.as_str(), None, &config.auth).await?;
            result.coverage = sub2api::coverage(&body);
            return sub2api::parse(&body, &config.monitor_groups)
                .ok_or_else(|| ("invalid_response".into(), Some(200)));
        }
        // These legacy schemas do not carry remote group identity. Never pass off an
        // aggregate as evidence for a requested remote classification.
        if !config.monitor_groups.is_empty()
            && matches!(template, "channel-health" | "model-status-embed")
        {
            return Err(("group_scope_unsupported".into(), None));
        }
        if template == "model-status-embed" {
            let selection = transport::json(client, endpoint.as_str(), None, &config.auth).await?;
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
            let body = transport::json(
                client,
                endpoint.as_str(),
                Some(&json!(selected)),
                &config.auth,
            )
            .await?;
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
                let body = transport::json(client, endpoint.as_str(), None, &config.auth).await?;
                if body["data"]["group"] != Value::String(group.clone()) {
                    return Err(("group_mismatch".into(), Some(200)));
                }
                let models = parsing::parse(template, &body, &[])
                    .ok_or_else(|| ("invalid_response".into(), Some(200)))?;
                for (model, rate) in models {
                    combined
                        .entry(model)
                        .and_modify(|(old, count): &mut (Option<f64>, usize)| {
                            *count += 1;
                            *old = match (*old, rate) {
                                (Some(a), Some(b)) => Some(a.min(b)),
                                _ => None,
                            }
                        })
                        .or_insert((rate, 1));
                }
            }
            return Ok(combined
                .into_iter()
                .map(|(model, (rate, count))| {
                    (
                        model,
                        if count == config.monitor_groups.len() {
                            rate
                        } else {
                            None
                        },
                    )
                })
                .collect());
        }
        let body = transport::json(client, endpoint.as_str(), None, &config.auth).await?;
        parsing::parse(template, &body, &config.monitor_groups)
            .ok_or_else(|| ("invalid_response".into(), Some(200)))
    };
    match operation.await {
        Ok(models) => {
            result.status = if models.is_empty() {
                "no_samples"
            } else if result
                .coverage
                .as_ref()
                .is_some_and(|c| c.complete == Some(false))
            {
                "partial_coverage"
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
