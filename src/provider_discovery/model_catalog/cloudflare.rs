//! Official text-generation directory, with one bounded pagination/byte/time budget.
use super::*;
use crate::protocol::cloudflare_workers_ai::{self as cf, Endpoint};

pub async fn fetch_catalog(base: &str, key: &str) -> Result<ModelCatalog, GatewayError> {
    let endpoint = Endpoint::parse(base).ok_or_else(cf::invalid)?;
    if !cf::valid_key(key) {
        return Err(cf::invalid());
    }
    let client = crate::http_client::builder()
        .redirect(rquest::redirect::Policy::none())
        .build()
        .map_err(|_| unavailable())?;
    fetch_from(&client, &endpoint, key, &endpoint.source_url()).await
}

// Only tests replace the source URL; the public entry point derives it from the account grammar.
async fn fetch_from(
    client: &rquest::Client,
    endpoint: &Endpoint,
    key: &str,
    first_url: &str,
) -> Result<ModelCatalog, GatewayError> {
    tokio::time::timeout(Duration::from_secs(30), async {
        let mut models = Vec::new();
        let mut seen = HashSet::new();
        let mut received = 0usize;
        for page in 1..=21 {
            let url = first_url.strip_suffix("page=1").ok_or_else(unavailable)?;
            let response = client
                .get(format!("{url}page={page}"))
                .bearer_auth(key)
                .timeout(Duration::from_secs(25))
                .send()
                .await
                .map_err(|_| unavailable())?;
            if !response.status().is_success()
                || response
                    .content_length()
                    .is_some_and(|size| size > LIMIT.saturating_sub(received) as u64)
            {
                return Err(unavailable());
            }
            let mut stream = response.bytes_stream();
            let mut bytes = Vec::new();
            while let Some(chunk) = stream.next().await {
                let chunk = chunk.map_err(|_| unavailable())?;
                received = received.saturating_add(chunk.len());
                if received > LIMIT {
                    return Err(unavailable());
                }
                bytes.extend_from_slice(&chunk);
            }
            let body: serde_json::Value =
                serde_json::from_slice(&bytes).map_err(|_| unavailable())?;
            if body["success"] != true {
                return Err(unavailable());
            }
            let entries = body["result"]
                .as_array()
                .filter(|items| items.len() <= 100)
                .ok_or_else(unavailable)?;
            for entry in entries {
                let id = entry["name"]
                    .as_str()
                    .filter(|id| valid_id(id))
                    .ok_or_else(unavailable)?;
                if seen.insert(id.to_owned()) {
                    models.push(id.to_owned());
                }
                if models.len() > 2048 {
                    return Err(unavailable());
                }
            }
            let finished = match body.get("result_info") {
                Some(info) => {
                    let current = info["page"].as_u64().ok_or_else(unavailable)?;
                    let pages = info["total_pages"]
                        .as_u64()
                        .filter(|n| *n <= 21)
                        .ok_or_else(unavailable)?;
                    if current != page || pages < page && !entries.is_empty() {
                        return Err(unavailable());
                    }
                    current >= pages
                }
                None => entries.len() < 100,
            };
            if finished {
                if models.is_empty() {
                    return Err(unavailable());
                }
                return Ok(ModelCatalog {
                    source_url: endpoint.source_url(),
                    account_base_url: Some(endpoint.base_url.clone()),
                    supported_endpoints: Default::default(),
                    checked_at: time::OffsetDateTime::now_utc()
                        .format(&time::format_description::well_known::Rfc3339)
                        .unwrap(),
                    models,
                });
            }
        }
        Err(unavailable())
    })
    .await
    .map_err(|_| unavailable())?
}

#[cfg(test)]
#[path = "cloudflare_tests.rs"]
mod tests;
