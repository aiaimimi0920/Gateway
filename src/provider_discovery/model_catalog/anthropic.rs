//! Anthropic uses x-api-key/version headers and cursor pagination, not Bearer auth.
use super::{unavailable, valid_id, ModelCatalog, ANTHROPIC_SOURCE, LIMIT};
use crate::error::GatewayError;
use futures::StreamExt;
use serde_json::Value;
use std::{collections::HashSet, time::Duration};

pub(super) async fn fetch(
    client: &rquest::Client,
    source: &str,
    key: &str,
) -> Result<ModelCatalog, GatewayError> {
    tokio::time::timeout(Duration::from_secs(30), async {
        let mut models = Vec::new();
        let mut ids = HashSet::new();
        let mut cursors = HashSet::new();
        let mut cursor: Option<String> = None;
        let mut total_bytes = 0usize;
        let mut total_entries = 0usize;
        // Continuations are encoded query values on the original fixed origin, never URLs.
        for _ in 0..16 {
            let mut url = url::Url::parse(source).map_err(|_| unavailable())?;
            if let Some(value) = &cursor {
                url.query_pairs_mut().append_pair("after_id", value);
            }
            let response = client
                .get(url.as_str())
                .header("x-api-key", key)
                .header("anthropic-version", "2023-06-01")
                .timeout(Duration::from_secs(25))
                .send()
                .await
                .map_err(|_| unavailable())?;
            if !response.status().is_success()
                || response
                    .content_length()
                    .is_some_and(|n| n > (LIMIT - total_bytes) as u64)
            {
                return Err(unavailable());
            }
            let mut bytes = Vec::new();
            let mut stream = response.bytes_stream();
            while let Some(chunk) = stream.next().await {
                let chunk = chunk.map_err(|_| unavailable())?;
                total_bytes = total_bytes.saturating_add(chunk.len());
                if total_bytes > LIMIT {
                    return Err(unavailable());
                }
                bytes.extend_from_slice(&chunk);
            }
            let body: Value = serde_json::from_slice(&bytes).map_err(|_| unavailable())?;
            let entries = body["data"].as_array().ok_or_else(unavailable)?;
            total_entries = total_entries.saturating_add(entries.len());
            if total_entries > 2048 {
                return Err(unavailable());
            }
            for entry in entries {
                let id = entry["id"]
                    .as_str()
                    .filter(|id| valid_id(id))
                    .ok_or_else(unavailable)?;
                if ids.insert(id.to_owned()) {
                    models.push(id.to_owned());
                }
            }
            if !body["has_more"].as_bool().ok_or_else(unavailable)? {
                if models.is_empty() {
                    return Err(unavailable());
                }
                return Ok(ModelCatalog {
                    source_url: ANTHROPIC_SOURCE.into(),
                    models,
                    checked_at: time::OffsetDateTime::now_utc()
                        .format(&time::format_description::well_known::Rfc3339)
                        .unwrap(),
                });
            }
            let next = body["last_id"]
                .as_str()
                .filter(|id| valid_id(id))
                .ok_or_else(unavailable)?;
            if entries.is_empty() || !cursors.insert(next.to_owned()) {
                return Err(unavailable());
            }
            cursor = Some(next.to_owned());
        }
        Err(unavailable())
    })
    .await
    .map_err(|_| unavailable())?
}
