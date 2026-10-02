//! Per-pool S3/R2 transport. The SDK signs and parses; a bounded connector owns I/O.
use std::collections::HashSet;
use std::time::Duration;

use anyhow::{anyhow, bail, Result};
use aws_sdk_s3::config::{
    retry::RetryConfig, timeout::TimeoutConfig, Credentials, Region, RequestChecksumCalculation,
    ResponseChecksumValidation,
};
use aws_sdk_s3::primitives::{ByteStream, SdkBody};
use aws_smithy_runtime_api::client::http::{
    http_client_fn, HttpConnector, HttpConnectorFuture, SharedHttpConnector,
};
use aws_smithy_runtime_api::client::orchestrator::{HttpRequest, HttpResponse};
use aws_smithy_runtime_api::client::result::ConnectorError;
use futures::StreamExt;
use tokio::time::timeout;

use super::capabilities::{for_connection, StorageCapabilities};
use super::{
    validate_object_key, StorageNotFound, MAX_LIST_ENTRIES, MAX_OBJECT_BYTES, NETWORK_DEADLINE,
};
use crate::routing::config::CredentialStorageConnection;

pub(super) struct S3Storage {
    client: aws_sdk_s3::Client,
    bucket: String,
    namespace: String,
    capabilities: StorageCapabilities,
    pub(super) deadline: Duration,
}

impl S3Storage {
    pub(super) fn new(connection: &CredentialStorageConnection, namespace: &str) -> Result<Self> {
        let CredentialStorageConnection::S3 {
            endpoint,
            bucket,
            region,
            access_key_id,
            secret_access_key,
            session_token,
            allow_insecure_http,
            ..
        } = connection
        else {
            bail!("invalid S3 storage connection");
        };
        let endpoint = url::Url::parse(endpoint).map_err(|_| anyhow!("invalid S3 endpoint"))?;
        if endpoint.host_str().is_none()
            || !endpoint.username().is_empty()
            || endpoint.password().is_some()
            || endpoint.query().is_some()
            || endpoint.fragment().is_some()
            || !(endpoint.scheme() == "https"
                || (*allow_insecure_http && endpoint.scheme() == "http"))
        {
            bail!("invalid S3 endpoint");
        }
        let namespace = namespace.trim_end_matches('/');
        if namespace.is_empty()
            || namespace.len() > 1024
            || namespace.split('/').any(|part| {
                part.is_empty()
                    || matches!(part, "." | "..")
                    || part.contains(['%', '\\', ':', '?', '#'])
                    || part.chars().any(char::is_control)
            })
            || bucket.is_empty()
            || bucket.len() > 63
            || !bucket
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b"-_.".contains(&b))
            || matches!(bucket.as_str(), "." | "..")
        {
            bail!("invalid S3 storage namespace");
        }
        let access_key = access_key_id.as_deref().filter(|value| !value.is_empty());
        let secret_key = secret_access_key
            .as_deref()
            .filter(|value| !value.is_empty());
        let (Some(access_key), Some(secret_key)) = (access_key, secret_key) else {
            bail!("S3 storage credentials are unavailable");
        };
        let http = crate::http_client::builder()
            .redirect(rquest::redirect::Policy::none())
            .connect_timeout(Duration::from_secs(5))
            .timeout(NETWORK_DEADLINE)
            .build()
            .map_err(|_| anyhow!("S3 transport initialization failed"))?;
        let connector = SharedHttpConnector::new(BoundedConnector {
            http,
            origin: endpoint.origin(),
        });
        let config = aws_sdk_s3::config::Builder::new()
            .behavior_version_latest()
            .region(Region::new(region.clone()))
            .endpoint_url(endpoint.as_str())
            .credentials_provider(Credentials::new(
                access_key,
                secret_key,
                session_token.clone(),
                None,
                "credential-pool",
            ))
            .force_path_style(true)
            // A failed write may have reached the server. The owner reconciles it by reading back.
            .retry_config(RetryConfig::disabled())
            .request_checksum_calculation(RequestChecksumCalculation::WhenRequired)
            .response_checksum_validation(ResponseChecksumValidation::WhenRequired)
            .timeout_config(
                TimeoutConfig::builder()
                    .operation_timeout(NETWORK_DEADLINE)
                    .operation_attempt_timeout(NETWORK_DEADLINE)
                    .build(),
            )
            .http_client(http_client_fn(move |_, _| connector.clone()))
            .build();
        Ok(Self {
            client: aws_sdk_s3::Client::from_conf(config),
            bucket: bucket.clone(),
            namespace: format!("{namespace}/"),
            capabilities: for_connection(connection),
            deadline: NETWORK_DEADLINE,
        })
    }

    #[cfg(test)]
    pub(super) fn allow_fixture_capabilities(&mut self) {
        self.capabilities.conditional_create = true;
        self.capabilities.conditional_delete = true;
    }

    fn object_key(&self, key: &str) -> Result<String> {
        validate_object_key(key)?;
        let full_key = format!("{}{key}", self.namespace);
        if full_key.len() > 1024 {
            bail!("S3 storage object key exceeded the size limit");
        }
        Ok(full_key)
    }

    pub(super) async fn list(&self) -> Result<Vec<String>> {
        timeout(self.deadline, async {
            let mut keys = HashSet::new();
            let mut tokens = HashSet::new();
            let mut token = None;
            let mut entries = 0usize;
            for _ in 0..32 {
                let response = self
                    .client
                    .list_objects_v2()
                    .bucket(&self.bucket)
                    .prefix(&self.namespace)
                    .delimiter("/")
                    .max_keys(256)
                    .set_continuation_token(token)
                    .send()
                    .await
                    .map_err(|_| anyhow!("S3 storage listing failed"))?;
                entries = entries
                    .saturating_add(response.contents().len())
                    .saturating_add(response.common_prefixes().len());
                if entries > MAX_LIST_ENTRIES {
                    bail!("S3 storage listing exceeded the entry limit");
                }
                for object in response.contents() {
                    let key = object
                        .key()
                        .ok_or_else(|| anyhow!("invalid S3 storage listing"))?;
                    let relative = key
                        .strip_prefix(&self.namespace)
                        .ok_or_else(|| anyhow!("S3 listing escaped the pool namespace"))?;
                    if !relative.is_empty()
                        && !relative.contains('/')
                        && relative.ends_with(".json")
                    {
                        validate_object_key(relative)?;
                        keys.insert(relative.to_owned());
                    }
                }
                if !response.is_truncated().unwrap_or(false) {
                    let mut keys: Vec<_> = keys.into_iter().collect();
                    keys.sort();
                    return Ok(keys);
                }
                let next = response
                    .next_continuation_token()
                    .filter(|next| !next.is_empty() && next.len() <= 4096)
                    .ok_or_else(|| anyhow!("invalid S3 storage pagination"))?;
                if !tokens.insert(next.to_owned()) {
                    bail!("S3 storage pagination stalled");
                }
                token = Some(next.to_owned());
            }
            bail!("S3 storage listing exceeded the page limit")
        })
        .await
        .map_err(|_| anyhow!("S3 storage listing timed out"))?
    }

    pub(super) async fn get(&self, key: &str) -> Result<Vec<u8>> {
        Ok(self.get_version(key).await?.0)
    }

    pub(super) async fn get_version(&self, key: &str) -> Result<(Vec<u8>, Option<String>)> {
        let key = self.object_key(key)?;
        timeout(self.deadline, async {
            let response = self
                .client
                .get_object()
                .bucket(&self.bucket)
                .key(key)
                .send()
                .await
                .map_err(|error| {
                    if error
                        .raw_response()
                        .is_some_and(|response| response.status().as_u16() == 404)
                    {
                        anyhow!(StorageNotFound)
                    } else {
                        anyhow!("S3 storage read failed")
                    }
                })?;
            // The connector already admits and bounds all response bytes before SDK parsing.
            let etag = response.e_tag().map(str::to_owned);
            let bytes = response
                .body
                .collect()
                .await
                .map_err(|_| anyhow!("S3 storage read failed"))?;
            Ok((bytes.into_bytes().to_vec(), etag))
        })
        .await
        .map_err(|_| anyhow!("S3 storage read timed out"))?
    }

    pub(super) async fn put(&self, key: &str, bytes: &[u8]) -> Result<()> {
        if !self.capabilities.conditional_create {
            bail!("Conditional S3 archive creation is unavailable for this endpoint");
        }
        let key = self.object_key(key)?;
        if bytes.len() > MAX_OBJECT_BYTES {
            bail!("S3 storage object exceeded the size limit");
        }
        timeout(
            self.deadline,
            self.client
                .put_object()
                .bucket(&self.bucket)
                .key(key)
                .content_type("application/json")
                .if_none_match("*")
                .body(ByteStream::from(bytes.to_vec()))
                .send(),
        )
        .await
        .map_err(|_| anyhow!("S3 storage write timed out"))?
        .map_err(|_| anyhow!("S3 storage write failed"))?;
        Ok(())
    }

    pub(super) async fn delete_verified(
        &self,
        key: &str,
        expected_bytes: &[u8],
        expected_etag: &str,
        current_revision: &(dyn Fn() -> bool + Send + Sync),
    ) -> Result<()> {
        if !self.capabilities.conditional_delete {
            bail!("Conditional S3 archive deletion is unavailable for this endpoint");
        }
        let object_key = self.object_key(key)?;
        timeout(self.deadline, async {
            let (bytes, etag) = self.get_version(key).await?;
            if bytes != expected_bytes {
                bail!("S3 storage object changed before deletion");
            }
            let etag = etag
                .filter(|value| {
                    value.len() >= 2
                        && value.len() <= 1024
                        && value.starts_with('"')
                        && value.ends_with('"')
                        && value[1..value.len() - 1]
                            .bytes()
                            .all(|b| b >= 0x21 && b != b'"' && b != 0x7f)
                })
                .ok_or_else(|| anyhow!("S3 storage deletion requires a strong ETag"))?;
            if etag != expected_etag {
                bail!("S3 archive version changed since purge was prepared");
            }
            if !current_revision() {
                bail!(
                    "Route revision changed during purge; completed deletions are not rolled back"
                );
            }
            self.client
                .delete_object()
                .bucket(&self.bucket)
                .key(object_key)
                .if_match(etag)
                .send()
                .await
                .map_err(|_| anyhow!("S3 conditional storage delete failed"))?;
            Ok(())
        })
        .await
        .map_err(|_| anyhow!("S3 storage delete timed out"))?
    }

    #[cfg(test)]
    pub(super) async fn delete(&self, key: &str) -> Result<()> {
        let key = self.object_key(key)?;
        timeout(
            self.deadline,
            self.client
                .delete_object()
                .bucket(&self.bucket)
                .key(key)
                .send(),
        )
        .await
        .map_err(|_| anyhow!("S3 storage delete timed out"))?
        .map_err(|_| anyhow!("S3 storage delete failed"))?;
        Ok(())
    }
}

#[derive(Clone, Debug)]
struct BoundedConnector {
    http: rquest::Client,
    origin: url::Origin,
}

fn transport_error() -> ConnectorError {
    ConnectorError::other(
        std::io::Error::other("S3 storage transport failed").into(),
        None,
    )
}

impl HttpConnector for BoundedConnector {
    fn call(&self, request: HttpRequest) -> HttpConnectorFuture {
        let connector = self.clone();
        HttpConnectorFuture::new(async move {
            let url = url::Url::parse(request.uri()).map_err(|_| transport_error())?;
            // Even SDK endpoint re-resolution must never forward signed headers to another origin.
            if url.origin() != connector.origin {
                return Err(transport_error());
            }
            let method = rquest::Method::from_bytes(request.method().as_bytes())
                .map_err(|_| transport_error())?;
            let body = request.body().bytes().ok_or_else(transport_error)?;
            if body.len() > MAX_OBJECT_BYTES {
                return Err(transport_error());
            }
            let mut outgoing = connector
                .http
                .request(method, request.uri())
                .body(body.to_vec());
            for (name, value) in request.headers().iter() {
                outgoing = outgoing.header(name, value);
            }
            let response = outgoing.send().await.map_err(|_| transport_error())?;
            let status = response
                .status()
                .as_u16()
                .try_into()
                .map_err(|_| transport_error())?;
            if !response.status().is_success() {
                // Do not retain, parse, log or follow untrusted error/redirect bodies and headers.
                return Ok(HttpResponse::new(status, SdkBody::empty()));
            }
            if response
                .content_length()
                .is_some_and(|length| length > MAX_OBJECT_BYTES as u64)
            {
                return Err(transport_error());
            }
            let mut result = HttpResponse::new(status, SdkBody::empty());
            for (name, value) in response.headers() {
                result
                    .headers_mut()
                    .try_append(
                        name.as_str().to_owned(),
                        value.to_str().map_err(|_| transport_error())?.to_owned(),
                    )
                    .map_err(|_| transport_error())?;
            }
            let stream = response.bytes_stream();
            futures::pin_mut!(stream);
            let mut bytes = Vec::new();
            while let Some(chunk) = stream.next().await {
                let chunk = chunk.map_err(|_| transport_error())?;
                if chunk.len() > MAX_OBJECT_BYTES.saturating_sub(bytes.len()) {
                    return Err(transport_error());
                }
                bytes
                    .try_reserve(chunk.len())
                    .map_err(|_| transport_error())?;
                bytes.extend_from_slice(&chunk);
            }
            *result.body_mut() = SdkBody::from(bytes);
            Ok(result)
        })
    }
}
