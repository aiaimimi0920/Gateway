//! Native, bounded Qwen Chat directory pagination. It never probes generation or changes region.
use super::{qwen_endpoint::QwenEndpoint, unavailable, valid_id, ModelCatalog, LIMIT};
use crate::error::GatewayError;
use futures::StreamExt;
use serde_json::Value;
use std::{collections::HashSet, time::Duration};

const PAGE_SIZE: usize = 20;
const MAX_ENTRIES: usize = 2048;
const MAX_PAGES: usize = MAX_ENTRIES.div_ceil(PAGE_SIZE);
const TOTAL_BYTES: usize = 8 * 1024 * 1024;

pub async fn fetch_catalog(base: &str, key: &str) -> Result<ModelCatalog, GatewayError> {
    let endpoint = QwenEndpoint::parse(base).ok_or_else(|| {
        GatewayError::bad_request(
            "Qwen catalog requires an explicit supported workspace API endpoint.",
        )
    })?;
    if key.is_empty() || key.len() > 16384 || !key.bytes().all(|b| b.is_ascii_graphic()) {
        return Err(GatewayError::bad_request("Invalid catalog API key"));
    }
    let client = crate::http_client::builder()
        .redirect(rquest::redirect::Policy::none())
        .build()
        .map_err(|_| unavailable())?;
    fetch_from(&client, &endpoint.catalog_source, key, &endpoint).await
}

// The transport URL is private. Only unit tests substitute a random loopback origin.
async fn fetch_from(
    client: &rquest::Client,
    source: &str,
    key: &str,
    endpoint: &QwenEndpoint,
) -> Result<ModelCatalog, GatewayError> {
    tokio::time::timeout(Duration::from_secs(30), async {
        let mut models = Vec::new();
        let mut seen = HashSet::new();
        let mut bytes_read = 0usize;
        let mut entries_read = 0usize;
        let mut expected_total = None;
        for page in 1..=MAX_PAGES {
            let body = read_page(client, source, key, page, &mut bytes_read).await?;
            if body["success"] != true || !body["code"].is_null() {
                return Err(unavailable());
            }
            let output = &body["output"];
            let total = output["total"].as_u64().ok_or_else(unavailable)?;
            if total == 0
                || total > MAX_ENTRIES as u64
                || expected_total.is_some_and(|expected| expected != total)
                || output["page_no"].as_u64() != Some(page as u64)
                || output["page_size"].as_u64() != Some(PAGE_SIZE as u64)
            {
                return Err(unavailable());
            }
            expected_total = Some(total);
            let entries = output["models"].as_array().ok_or_else(unavailable)?;
            if entries.len() != PAGE_SIZE.min(total as usize - entries_read) {
                return Err(unavailable());
            }
            entries_read += entries.len();
            for item in entries {
                let id = item["model"]
                    .as_str()
                    .filter(|id| valid_id(id))
                    .ok_or_else(unavailable)?;
                // A repeated page/identity must not masquerade as a complete directory.
                if !seen.insert(id.to_owned()) {
                    return Err(unavailable());
                }
                if chat_metadata(item)? {
                    models.push(id.to_owned());
                }
            }
            if entries_read == total as usize {
                if models.is_empty() {
                    return Err(unavailable());
                }
                return Ok(ModelCatalog {
                    source_url: endpoint.catalog_source.clone(),
                    account_base_url: Some(endpoint.base_url.clone()),
                    supported_endpoints: Default::default(),
                    models,
                    checked_at: time::OffsetDateTime::now_utc()
                        .format(&time::format_description::well_known::Rfc3339)
                        .map_err(|_| unavailable())?,
                });
            }
        }
        Err(unavailable())
    })
    .await
    .map_err(|_| unavailable())?
}

async fn read_page(
    client: &rquest::Client,
    source: &str,
    key: &str,
    page: usize,
    total_bytes: &mut usize,
) -> Result<Value, GatewayError> {
    let mut url = url::Url::parse(source).map_err(|_| unavailable())?;
    url.query_pairs_mut()
        .append_pair("page_no", &page.to_string())
        .append_pair("page_size", &PAGE_SIZE.to_string())
        .append_pair("providers", "qwen")
        .append_pair("supports", "inference")
        .append_pair("capabilities", "TG")
        .append_pair("capabilities", "Reasoning");
    let response = client
        .get(url.as_str())
        .bearer_auth(key)
        .timeout(Duration::from_secs(25))
        .send()
        .await
        .map_err(|_| unavailable())?;
    if !response.status().is_success()
        || response
            .content_length()
            .is_some_and(|size| size > LIMIT.min(TOTAL_BYTES - *total_bytes) as u64)
    {
        return Err(unavailable());
    }
    let mut bytes = Vec::new();
    let mut stream = response.bytes_stream();
    while let Some(chunk) = stream.next().await {
        let chunk = chunk.map_err(|_| unavailable())?;
        *total_bytes = total_bytes.saturating_add(chunk.len());
        if *total_bytes > TOTAL_BYTES || bytes.len().saturating_add(chunk.len()) > LIMIT {
            return Err(unavailable());
        }
        bytes.extend_from_slice(&chunk);
    }
    serde_json::from_slice(&bytes).map_err(|_| unavailable())
}

fn chat_metadata(item: &Value) -> Result<bool, GatewayError> {
    if let Some(provider) = item.get("provider") {
        if provider.as_str().ok_or_else(unavailable)? != "qwen" {
            return Ok(false);
        }
    }
    if let Some(capabilities) = item.get("capabilities") {
        let values = capabilities.as_array().ok_or_else(unavailable)?;
        if values.iter().any(|value| !value.is_string()) {
            return Err(unavailable());
        }
        if !values
            .iter()
            .any(|value| value == "TG" || value == "Reasoning")
        {
            return Ok(false);
        }
    }
    // The API sample omits provider/capabilities; its declared Text I/O is still required.
    let metadata = &item["inference_metadata"];
    for field in ["request_modality", "response_modality"] {
        let Some(values) = metadata[field].as_array() else {
            return Ok(false);
        };
        if values.iter().any(|value| !value.is_string()) {
            return Err(unavailable());
        }
        if !values.iter().any(|value| value == "Text") {
            return Ok(false);
        }
    }
    Ok(true)
}

#[cfg(test)]
#[path = "qwen_adversarial_tests.rs"]
mod adversarial_tests;
#[cfg(test)]
#[path = "qwen_test_support.rs"]
mod test_support;
#[cfg(test)]
#[path = "qwen_tests.rs"]
mod tests;
