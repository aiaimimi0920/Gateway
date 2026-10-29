//! Bounded fetch JSON; HTTP 200 does not prove that any page was extracted.
use crate::{error::GatewayError, routing::candidate::ProviderAccountPayload};
use futures::StreamExt;
use serde_json::Value;

const MAX_FETCH_BYTES: usize = 8 * 1024 * 1024;

fn invalid(message: &str) -> GatewayError {
    let mut error = GatewayError::server_error(message).with_code("invalid_fetch_response");
    error.http_status = Some(502);
    error
}

pub(crate) async fn read_json(
    response: rquest::Response,
    payload: &ProviderAccountPayload,
    req: &crate::protocol::canonical::CanonicalRelayRequest,
) -> Result<Value, GatewayError> {
    let mut stream = response.bytes_stream();
    let mut bytes = Vec::new();
    while let Some(chunk) = stream.next().await {
        let chunk = chunk.map_err(|_| invalid("Failed to read fetch response"))?;
        if chunk.len() > MAX_FETCH_BYTES.saturating_sub(bytes.len()) {
            return Err(invalid("Fetch response exceeded 8 MiB"));
        }
        bytes.extend_from_slice(&chunk);
    }
    let body =
        serde_json::from_slice(&bytes).map_err(|_| invalid("Fetch returned invalid JSON"))?;
    let metadata_only = req
        .raw_body
        .get("formats")
        .and_then(Value::as_array)
        .is_some_and(|formats| formats.len() == 1 && formats[0] == "metadata");
    validate(payload, &body, metadata_only)?;
    Ok(body)
}

fn has_error(value: &Value) -> bool {
    value
        .get("error")
        .is_some_and(|v| !v.is_null() && v != false && v != "")
}

fn nonempty(value: Option<&Value>) -> bool {
    value
        .and_then(Value::as_str)
        .is_some_and(|text| !text.trim().is_empty())
}

fn page_url(value: &Value) -> bool {
    value.get("url").and_then(Value::as_str).is_some_and(|url| {
        url::Url::parse(url)
            .is_ok_and(|u| matches!(u.scheme(), "http" | "https") && u.host_str().is_some())
    })
}

fn page(value: &Value, fields: &[&str]) -> bool {
    !has_error(value) && page_url(value) && fields.iter().any(|field| nonempty(value.get(*field)))
}

pub(crate) fn validate(
    payload: &ProviderAccountPayload,
    body: &Value,
    metadata_only: bool,
) -> Result<usize, GatewayError> {
    let count = match payload.fetch_path.as_deref() {
        Some("/v1/contents") => body.as_array().map(|rows| {
            rows.iter()
                .filter(|row| {
                    page(row, &["markdown", "html"])
                        || (metadata_only
                            && !has_error(row)
                            && page_url(row)
                            && row
                                .get("metadata")
                                .and_then(Value::as_object)
                                .is_some_and(|m| !m.is_empty()))
                })
                .count()
        }),
        Some("/extract") => (!has_error(body))
            .then(|| {
                body.get("results").and_then(Value::as_array).map(|rows| {
                    rows.iter()
                        .filter(|row| page(row, &["raw_content"]))
                        .count()
                })
            })
            .flatten(),
        Some("/scrape") => (!has_error(body) && body.get("code").is_none_or(|code| code == 200))
            .then(|| {
                body.get("data")
                    .filter(|row| page(row, &["content"]))
                    .map(|_| 1)
            })
            .flatten(),
        _ => return Ok(0), // Existing vendors retain their native response contract.
    };
    count
        .filter(|count| *count > 0)
        .ok_or_else(|| invalid("Fetch returned no usable page content"))
}

#[cfg(test)]
mod tests;
