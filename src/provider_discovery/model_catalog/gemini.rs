//! Fixed-origin Gemini metadata only: no generation, redirects, or credential-bearing URLs.
use super::{unavailable, valid_id, ModelCatalog, GEMINI_SOURCE, LIMIT};
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
        let mut tokens = HashSet::new();
        let mut token: Option<String> = None;
        let mut total_bytes = 0usize;
        let mut total_entries = 0usize;
        // Continuations are encoded query values, never replacement destinations.
        for _ in 0..16 {
            let mut url = url::Url::parse(source).map_err(|_| unavailable())?;
            if let Some(value) = token.as_deref() {
                url.query_pairs_mut().append_pair("pageToken", value);
            }
            let response = client
                .get(url.as_str())
                .header("x-goog-api-key", key)
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
            let entries = body["models"].as_array().ok_or_else(unavailable)?;
            total_entries = total_entries.saturating_add(entries.len());
            if total_entries > 2048 {
                return Err(unavailable());
            }
            for item in entries {
                // An absent capability is not evidence of a usable generation model.
                let Some(methods) = item.get("supportedGenerationMethods") else {
                    continue;
                };
                let methods = methods.as_array().ok_or_else(unavailable)?;
                if methods.iter().any(|method| !method.is_string()) {
                    return Err(unavailable());
                }
                if !methods.iter().any(|method| method == "generateContent") {
                    continue;
                }
                let id = item["name"]
                    .as_str()
                    .and_then(|name| name.strip_prefix("models/"))
                    .filter(|id| valid_id(id) && !id.contains('/'))
                    .ok_or_else(unavailable)?;
                if ids.insert(id.to_owned()) {
                    models.push(id.to_owned());
                }
            }
            token = match body.get("nextPageToken") {
                None | Some(Value::Null) => None,
                Some(Value::String(value)) if value.is_empty() => None,
                Some(Value::String(value))
                    if value.len() <= 4096 && !value.chars().any(char::is_control) =>
                {
                    Some(value.clone())
                }
                _ => return Err(unavailable()),
            };
            if let Some(value) = &token {
                if !tokens.insert(value.clone()) {
                    return Err(unavailable());
                }
            } else {
                if models.is_empty() {
                    return Err(unavailable());
                }
                return Ok(ModelCatalog {
                    source_url: GEMINI_SOURCE.into(),
                    models,
                    checked_at: time::OffsetDateTime::now_utc()
                        .format(&time::format_description::well_known::Rfc3339)
                        .unwrap(),
                });
            }
        }
        Err(unavailable())
    })
    .await
    .map_err(|_| unavailable())?
}
