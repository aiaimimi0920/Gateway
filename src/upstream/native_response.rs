//! Native transport response limits and retry evidence are identical to the buffered policy.
use crate::error::{
    classify_native_network_error, classify_upstream_error, retry_after_from_headers, GatewayError,
};
use crate::protocol::upstream_body::MAX_ACCUMULATED_UPSTREAM_BODY_BYTES;
use futures::StreamExt;

pub(super) async fn read(
    response: reqwest::Response,
    provider: &str,
) -> Result<Vec<u8>, GatewayError> {
    let limit = MAX_ACCUMULATED_UPSTREAM_BODY_BYTES;
    let too_large = || {
        GatewayError::server_error(format!(
            "Native upstream body exceeded the {limit}-byte accumulation limit."
        ))
        .with_code("upstream_body_too_large")
        .with_provider(provider)
    };
    if response
        .content_length()
        .is_some_and(|len| len > limit as u64)
    {
        return Err(too_large());
    }
    let mut stream = response.bytes_stream();
    let mut body = Vec::new();
    while let Some(chunk) = stream.next().await {
        let chunk = chunk.map_err(|e| classify_native_network_error(e, provider))?;
        if chunk.len() > limit.saturating_sub(body.len()) {
            return Err(too_large());
        }
        body.try_reserve(chunk.len()).map_err(|_| {
            GatewayError::server_error("Native upstream buffer allocation failed")
                .with_code("upstream_body_buffer_allocation_failed")
                .with_provider(provider)
        })?;
        body.extend_from_slice(&chunk);
    }
    Ok(body)
}

pub(super) async fn success(
    response: reqwest::Response,
    provider: &str,
) -> Result<reqwest::Response, GatewayError> {
    if response.status().is_success() {
        return Ok(response);
    }
    let status = response.status().as_u16();
    let delay = retry_after_from_headers(response.headers(), std::time::SystemTime::now());
    let body = read(response, provider).await?;
    Err(
        classify_upstream_error(status, &String::from_utf8_lossy(&body), Some(provider))
            .with_retry_after_ms(delay),
    )
}
