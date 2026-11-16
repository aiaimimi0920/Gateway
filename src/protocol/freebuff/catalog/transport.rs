//! Dedicated fixed-target transport: no default secret headers/cookies, retries or redirects.
use super::{account::Binding, CatalogError, DeviceRegistration};
use std::time::Duration;
pub(super) const CATALOG_PATH: &str = "/api/v1/freebuff/models";
const DEVICE_PATH: &str = "/api/v1/freebuff/device-keys";
pub(super) const CATALOG_BODY_LIMIT: usize = 1_048_576;

pub(super) struct CatalogTransport(pub(super) rquest::Client);
impl CatalogTransport {
    pub fn new() -> Result<Self, CatalogError> {
        let builder = crate::http_client::builder().retry(rquest::retry::Policy::never());
        #[cfg(test)]
        let builder = builder.no_proxy();
        builder
            .build()
            .map(Self)
            .map_err(|_| CatalogError::Transport)
    }
}
pub(super) enum Request<'a> {
    Catalog {
        device: Option<&'a DeviceRegistration>,
    },
    Register {
        public_key: String,
    },
}
pub(super) struct Reply {
    pub status: u16,
    pub bytes: Vec<u8>,
}
impl Reply {
    pub fn success(&self) -> Result<(), CatalogError> {
        match self.status {
            200..=299 => return Ok(()),
            408 | 429 | 500..=599 => return Err(CatalogError::Retryable),
            _ => {}
        }
        // Codes, never error prose or secret-bearing upstream bodies, leave this boundary.
        let unknown = serde_json::from_slice::<serde_json::Value>(&self.bytes)
            .ok()
            .and_then(|v| v.get("error").and_then(|s| s.as_str()).map(str::to_owned))
            .is_some_and(|code| {
                let code = code.to_ascii_lowercase().replace(['_', '-'], "");
                code.contains("devicekey")
                    && ["unknown", "notfound", "invalid", "unregistered"]
                        .iter()
                        .any(|s| code.contains(s))
            });
        Err(if unknown {
            CatalogError::DeviceUnknown
        } else {
            match self.status {
                401 | 403 => CatalogError::Unauthorized,
                404 | 405 => CatalogError::Unsupported,
                _ => CatalogError::Rejected,
            }
        })
    }
}
#[cfg(feature = "line-freebuff-web-reverse-api")]
pub(super) async fn exchange(
    http: &CatalogTransport,
    binding: &Binding,
    request: Request<'_>,
    budget: Duration,
) -> Result<Reply, CatalogError> {
    use futures::StreamExt;
    if budget.is_zero() {
        return Err(CatalogError::InvalidInput);
    }
    let budget = budget.min(Duration::from_secs(10));
    tokio::time::timeout(budget, async {
        let origin = binding.transport_origin();
        let (request, limit) = match request {
            Request::Catalog { device } => {
                let mut request = http
                    .0
                    .get(format!("{origin}{CATALOG_PATH}"))
                    .header("x-freebuff-catalog-protocol", "1")
                    .header("x-freebuff-client", binding.client.as_str());
                if let Some(device) = device {
                    let now = std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .map_err(|_| CatalogError::InvalidInput)?
                        .as_millis();
                    let timestamp = u64::try_from(now).map_err(|_| CatalogError::InvalidInput)?;
                    request = request.headers(device.catalog_headers(binding, timestamp)?);
                }
                (request, CATALOG_BODY_LIMIT)
            }
            Request::Register { public_key } => {
                let body = serde_json::to_vec(&serde_json::json!({
                    "publicKey": public_key, "client": binding.client.as_str()
                }))
                .expect("serializable local registration");
                (
                    http.0
                        .post(format!("{origin}{DEVICE_PATH}"))
                        .header("content-type", "application/json")
                        .body(body),
                    65_536,
                )
            }
        };
        let response = request
            .default_headers(false)
            .redirect(rquest::redirect::Policy::none())
            .timeout(budget)
            .header("accept", "application/json")
            .header("authorization", binding.authorization.clone())
            .send()
            .await
            .map_err(classify)?;
        if response.status().is_redirection() {
            return Err(CatalogError::RedirectRejected);
        }
        if response.content_length().is_some_and(|n| n > limit as u64) {
            return Err(CatalogError::ResponseTooLarge);
        }
        let status = response.status().as_u16();
        let mut bytes = Vec::new();
        let mut stream = response.bytes_stream();
        #[cfg(test)]
        let _ = super::TEST_BODY_STARTED.try_with(|started| started.notify_one());
        while let Some(chunk) = stream.next().await {
            let chunk = chunk.map_err(classify)?;
            if bytes.len().saturating_add(chunk.len()) > limit {
                return Err(CatalogError::ResponseTooLarge);
            }
            bytes
                .try_reserve(chunk.len())
                .map_err(|_| CatalogError::ResponseTooLarge)?;
            bytes.extend_from_slice(&chunk);
        }
        Ok(Reply { status, bytes })
    })
    .await
    .map_err(|_| CatalogError::Timeout)?
}
#[cfg(feature = "line-freebuff-web-reverse-api")]
fn classify(error: rquest::Error) -> CatalogError {
    if error.is_timeout() {
        CatalogError::Timeout
    } else {
        CatalogError::Transport
    }
}
#[cfg(not(feature = "line-freebuff-web-reverse-api"))]
pub(super) async fn exchange(
    http: &CatalogTransport,
    binding: &Binding,
    request: Request<'_>,
    budget: Duration,
) -> Result<Reply, CatalogError> {
    let _ = (http, binding, request, budget);
    Err(CatalogError::FeatureDisabled)
}
