//! Same-origin discovery, bounded requests, explicit outcomes; no upstream error bodies persist.
use super::{
    catalogue, registry, CredentialDiscovery, DiscoveredProtocol, ProbeAttempt, ProtocolCapability,
    ProtocolProbe,
};
use futures::{stream, StreamExt};
use serde_json::Value;

pub(super) fn checked_url(value: &str) -> anyhow::Result<url::Url> {
    let url = url::Url::parse(value).map_err(|_| anyhow::anyhow!("Invalid service URL."))?;
    anyhow::ensure!(
        matches!(url.scheme(), "http" | "https")
            && url.host_str().is_some()
            && url.username().is_empty()
            && url.password().is_none()
            && url.query().is_none()
            && url.fragment().is_none(),
        "Service URL must be HTTP(S), without credentials, query or fragment."
    );
    Ok(url)
}

pub(super) async fn read_json(response: rquest::Response) -> anyhow::Result<Value> {
    anyhow::ensure!(
        response.status().is_success(),
        "Upstream rejected discovery."
    );
    let mut stream = response.bytes_stream();
    let mut bytes = Vec::new();
    while let Some(chunk) = stream.next().await {
        let chunk = chunk?;
        anyhow::ensure!(
            bytes.len() + chunk.len() <= 1_048_576,
            "Discovery response exceeds 1 MiB."
        );
        bytes.extend_from_slice(&chunk);
    }
    serde_json::from_slice(&bytes).map_err(|_| anyhow::anyhow!("Discovery returned invalid JSON."))
}

pub(super) async fn discover(url: &str, key: &str) -> anyhow::Result<CredentialDiscovery> {
    checked_url(url)?;
    let client = crate::http_client::builder()
        .redirect(rquest::redirect::Policy::none())
        .timeout(super::REQUEST_TIMEOUT)
        .build()?;
    let models = catalogue::load(&client, url, key).await?;
    let mut results = stream::iter(registry::ALL.into_iter().enumerate().map(
        |(index, protocol)| {
            let client = &client;
            let models = &models;
            async move {
                (
                    index,
                    probe_protocol(client, url, models, key, protocol).await,
                )
            }
        },
    ))
    .buffer_unordered(4)
    .collect::<Vec<_>>()
    .await;
    results.sort_by_key(|r| r.0);
    let mut protocols = Vec::new();
    let mut probes = Vec::new();
    for (_, (capability, probe)) in results {
        if let Some(c) = capability {
            protocols.push(c);
        }
        probes.push(probe);
    }
    let first = protocols
        .first()
        .ok_or_else(|| anyhow::anyhow!("No working text protocol found."))?;
    Ok(CredentialDiscovery {
        source_url: url.trim_end_matches('/').into(),
        api_base: first.api_base.clone(),
        protocol: first.protocol,
        verified_models: first.verified_models.clone(),
        models,
        protocols,
        probes,
        checked_at: time::OffsetDateTime::now_utc()
            .format(&time::format_description::well_known::Rfc3339)?,
        binding: super::binding(url, key),
    })
}

async fn probe_protocol(
    client: &rquest::Client,
    url: &str,
    models: &[String],
    key: &str,
    protocol: DiscoveredProtocol,
) -> (Option<ProtocolCapability>, ProtocolProbe) {
    let mut report = ProtocolProbe {
        protocol,
        routing_ready: protocol.routing_ready(),
        status: "unconfirmed".into(),
        attempts: Vec::new(),
    };
    let ordered = super::sampling::models(models, protocol);
    let operation = async {
        for base in registry::bases(url, protocol).unwrap_or_default() {
            for model in ordered.iter().take(3) {
                let (path, body) = registry::request(protocol, model);
                let endpoint = format!("{base}{path}");
                let request =
                    registry::authenticate(client.post(&endpoint).json(&body), protocol, key);
                let mut attempt = ProbeAttempt {
                    endpoint,
                    model: Some((*model).clone()),
                    status: "pending".into(),
                    http_status: None,
                };
                report.attempts.push(attempt.clone());
                match request.send().await {
                    Ok(response) => {
                        let status = response.status().as_u16();
                        attempt.http_status = Some(status);
                        attempt.status = match status {
                            401 | 403 => "authentication_required",
                            404 | 405 => "not_found",
                            429 => "rate_limited",
                            200..=299 => "invalid_response",
                            _ => "rejected",
                        }
                        .into();
                        if (200..300).contains(&status) {
                            match read_json(response).await {
                                Ok(body) if registry::has_reply(protocol, &body) => {
                                    attempt.status = "supported".into();
                                    *report.attempts.last_mut().unwrap() = attempt;
                                    report.status = "supported".into();
                                    return Some(ProtocolCapability {
                                        protocol,
                                        api_base: base,
                                        verified_models: vec![(*model).clone()],
                                        failed_models: Vec::new(),
                                    });
                                }
                                Err(error)
                                    if error
                                        .downcast_ref::<rquest::Error>()
                                        .is_some_and(|e| e.is_timeout()) =>
                                {
                                    attempt.status = "timeout".into();
                                }
                                _ => {}
                            }
                        }
                        *report.attempts.last_mut().unwrap() = attempt;
                        // Aggregators can return 401/403 for a model's upstream channel,
                        // even though the same account works with another catalogue model.
                        if matches!(status, 405 | 429) {
                            break;
                        }
                    }
                    Err(error) => {
                        attempt.status = "transport_error".into();
                        if error.is_timeout() {
                            attempt.status = "timeout".into();
                        }
                        *report.attempts.last_mut().unwrap() = attempt;
                        break;
                    }
                }
            }
        }
        None
    };
    let capability = operation.await;
    if capability.is_none() && report.attempts.iter().any(|a| a.status == "timeout") {
        report.status = "timeout".into();
    }
    (capability, report)
}
