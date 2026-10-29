//! Cohere's paginated name-based Chat directory is not an OpenAI /models response.
use super::{unavailable, valid_id, ModelCatalog, COHERE_SOURCE, LIMIT};
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
        // All continuation requests keep the fixed origin and query; tokens are never URLs.
        for _ in 0..16 {
            let mut url = url::Url::parse(source).map_err(|_| unavailable())?;
            if let Some(value) = token.as_deref() {
                url.query_pairs_mut().append_pair("page_token", value);
            }
            let request = client
                .get(url.as_str())
                .bearer_auth(key)
                .timeout(Duration::from_secs(25));
            let response = request.send().await.map_err(|_| unavailable())?;
            if !response.status().is_success()
                || response
                    .content_length()
                    .is_some_and(|n| n > (LIMIT - total_bytes) as u64)
            {
                return Err(unavailable());
            }
            let mut body = Vec::new();
            let mut stream = response.bytes_stream();
            while let Some(chunk) = stream.next().await {
                let chunk = chunk.map_err(|_| unavailable())?;
                total_bytes = total_bytes.saturating_add(chunk.len());
                if total_bytes > LIMIT {
                    return Err(unavailable());
                }
                body.extend_from_slice(&chunk);
            }
            let body: Value = serde_json::from_slice(&body).map_err(|_| unavailable())?;
            let entries = body["models"].as_array().ok_or_else(unavailable)?;
            total_entries = total_entries.saturating_add(entries.len());
            if total_entries > 2048 {
                return Err(unavailable());
            }
            for item in entries {
                // Optional metadata cannot prove a routable Chat model; skip incomplete entries.
                if item.get("name").is_none() || item.get("endpoints").is_none() {
                    continue;
                }
                let id = item["name"]
                    .as_str()
                    .filter(|id| valid_id(id))
                    .ok_or_else(unavailable)?;
                let endpoints = item["endpoints"].as_array().ok_or_else(unavailable)?;
                if endpoints.iter().any(|v| !v.is_string())
                    || item.get("is_deprecated").is_some_and(|v| !v.is_boolean())
                {
                    return Err(unavailable());
                }
                if item["is_deprecated"] == true || !endpoints.iter().any(|v| v == "chat") {
                    continue;
                }
                if ids.insert(id.to_owned()) {
                    models.push(id.to_owned());
                }
            }
            token = match body.get("next_page_token") {
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
                    source_url: COHERE_SOURCE.into(),
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
