//! Family-level negotiation: anonymous discovery, then at most one authenticated reader.
use super::{probe, readers::Reader, ProbePolicy, ProbeResult};
use crate::site_health::MonitoringAuthKind;

pub(super) fn supported(probe: &ProbeResult) -> bool {
    matches!(probe.status.as_str(), "supported" | "partial_coverage")
}

fn recognized(probe: &ProbeResult) -> bool {
    supported(probe) || probe.status == "no_samples"
}

pub(super) fn best(probes: &[ProbeResult]) -> Option<usize> {
    (0..probes.len())
        .rev()
        .filter(|&i| supported(&probes[i]))
        .max_by_key(|&i| probes[i].models.values().filter(|v| v.is_some()).count())
}

async fn read(
    client: &rquest::Client,
    origin: &url::Url,
    reader: Reader,
    config: &ProbePolicy,
) -> ProbeResult {
    let (id, path) = reader.recipe();
    probe(client, origin, id, path, config).await
}

pub(super) async fn detect(
    client: &rquest::Client,
    origin: &url::Url,
    config: &ProbePolicy,
) -> (Vec<ProbeResult>, Option<usize>) {
    let readers = Reader::candidates(config.adapter);
    // Legacy explicit readers keep their exact request/credential scope. No migration
    // on read and no automatic secret grant are needed for existing saved plans.
    if let Some(reader) = Reader::legacy(config.adapter) {
        let result = read(client, origin, reader, config).await;
        let selected = supported(&result).then_some(0);
        return (vec![result], selected);
    }
    let authenticated = config.auth.kind != MonitoringAuthKind::None;
    let mut public = config.clone();
    public.auth = Default::default();
    let mut probes = Vec::with_capacity(readers.len());
    for pair in readers.chunks(2) {
        let public_probe = |reader| {
            let mut policy = public.clone();
            if authenticated {
                // Discovery proves schema, not scoped health. Avoid fetching a whole
                // detail matrix twice; only the final reader can publish model evidence.
                if reader == Reader::PerfSummary {
                    policy.monitor_groups.clear();
                }
                if reader == Reader::Slot {
                    policy.monitor_groups.truncate(1);
                }
            }
            async move { read(client, origin, reader, &policy).await }
        };
        if let Some(&other) = pair.get(1) {
            let (a, b) = tokio::join!(public_probe(pair[0]), public_probe(other));
            probes.extend([a, b]);
        } else {
            probes.push(public_probe(pair[0]).await);
        }
    }
    if !authenticated {
        let selected = best(&probes);
        return (probes, selected);
    }
    let selected = best(&probes)
        .or_else(|| probes.iter().position(recognized))
        .or_else(|| {
            Reader::authenticated_default(config.adapter)
                .and_then(|reader| readers.iter().position(|r| *r == reader))
        });
    if let Some(index) = selected {
        for probe in &mut probes {
            if recognized(probe) {
                probe.status = "capability_detected".into();
                probe.coverage = None;
            }
        }
        // Never retry credentials against other recipes after 401/403, redirects or
        // schema errors. Public recognition is not proof of the user's model access.
        probes[index] = read(client, origin, readers[index], config).await;
        let selected = supported(&probes[index]).then_some(index);
        (probes, selected)
    } else {
        probes.push(ProbeResult {
            observations: Vec::new(),
            template: config.adapter.id().into(),
            endpoint: origin.to_string(),
            status: "authentication_target_unconfirmed".into(),
            http_status: None,
            model_count: 0,
            models: Default::default(),
            coverage: None,
        });
        (probes, None)
    }
}
