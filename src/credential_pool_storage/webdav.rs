//! Bounded, authenticated WebDAV I/O confined to one provider's namespace.
use super::capabilities::{for_connection, StorageCapabilities};
use super::{validate_object_key, StorageNotFound, MAX_OBJECT_BYTES, NETWORK_DEADLINE};
use crate::routing::config::CredentialStorageConnection;
use anyhow::{anyhow, bail, Result};
use futures::StreamExt;
use rquest::{Client, Method, RequestBuilder, Response};
use std::future::Future;
use url::Url;

#[path = "webdav_path.rs"]
mod path;
#[path = "webdav_xml.rs"]
mod xml;

pub(super) struct WebdavStorage {
    client: Client,
    namespace: Url,
    collections: Vec<Url>,
    username: Option<String>,
    password: Option<String>,
    capabilities: StorageCapabilities,
}

impl WebdavStorage {
    pub(super) fn new(connection: &CredentialStorageConnection, namespace: &str) -> Result<Self> {
        let CredentialStorageConnection::Webdav {
            endpoint,
            username,
            password,
            allow_insecure_http,
            ..
        } = connection
        else {
            bail!("invalid WebDAV storage connection");
        };
        if password.is_some() && username.as_deref().is_none_or(str::is_empty)
            || username
                .as_ref()
                .is_some_and(|name| name.contains(':') || name.chars().any(char::is_control))
        {
            bail!("invalid WebDAV authentication configuration");
        }
        let (namespace, collections) =
            path::namespace_urls(endpoint, namespace, *allow_insecure_http)?;
        let client = Client::builder()
            .redirect(rquest::redirect::Policy::none())
            .timeout(NETWORK_DEADLINE)
            .build()
            .map_err(|_| anyhow!("failed to initialize WebDAV transport"))?;
        Ok(Self {
            client,
            namespace,
            collections,
            username: username.clone().filter(|value| !value.is_empty()),
            password: password.clone(),
            capabilities: for_connection(connection),
        })
    }

    #[cfg(test)]
    pub(super) fn allow_fixture_capabilities(&mut self) {
        self.capabilities = StorageCapabilities {
            conditional_create: true,
            conditional_delete: true,
        };
    }

    fn request(&self, method: Method, url: Url) -> RequestBuilder {
        let request = self.client.request(method, url.as_str());
        match self.username.as_ref() {
            Some(username) => request.basic_auth(username, self.password.as_ref()),
            None => request,
        }
    }

    fn object_url(&self, key: &str) -> Result<Url> {
        validate_object_key(key)?;
        if key.contains('/') {
            bail!("WebDAV object must be an immediate JSON file");
        }
        let mut url = self.namespace.clone();
        url.path_segments_mut()
            .map_err(|_| anyhow!("invalid WebDAV namespace"))?
            .pop_if_empty()
            .push(key);
        Ok(url)
    }

    pub(super) async fn list(&self) -> Result<Vec<String>> {
        deadline(async {
            let response = send(
                self.request(Method::from_bytes(b"PROPFIND")?, self.namespace.clone())
                    .header("Depth", "1")
                    .header("Content-Type", "application/xml; charset=utf-8")
                    .body("<?xml version=\"1.0\"?><d:propfind xmlns:d=\"DAV:\"><d:prop><d:resourcetype/></d:prop></d:propfind>"),
            )
            .await?;
            require_status(&response, &[207])?;
            xml::list_keys(&read_bounded(response).await?, &self.namespace)
        })
        .await
    }

    pub(super) async fn get(&self, key: &str) -> Result<Vec<u8>> {
        let url = self.object_url(key)?;
        deadline(async {
            let response = send(self.request(Method::GET, url)).await?;
            require_status(&response, &[200])?;
            read_bounded(response).await
        })
        .await
    }

    pub(super) async fn get_version(&self, key: &str) -> Result<(Vec<u8>, Option<String>)> {
        let url = self.object_url(key)?;
        deadline(async {
            let response = send(self.request(Method::GET, url)).await?;
            require_status(&response, &[200])?;
            let etag = response
                .headers()
                .get("etag")
                .and_then(|v| v.to_str().ok())
                .map(str::to_owned);
            Ok((read_bounded(response).await?, etag))
        })
        .await
    }

    pub(super) async fn put(&self, key: &str, bytes: &[u8]) -> Result<()> {
        if !self.capabilities.conditional_create {
            bail!("WebDAV conditional creation capability is unconfirmed");
        }
        let url = self.object_url(key)?;
        if bytes.len() > MAX_OBJECT_BYTES {
            bail!("WebDAV object exceeds the size limit");
        }
        deadline(async {
            let mut response = self.put_once(url.clone(), bytes).await?;
            if matches!(response.status().as_u16(), 404 | 409) {
                drop(response);
                // Never create the endpoint root or ascend above the configured directory.
                for collection in &self.collections {
                    let created =
                        send(self.request(Method::from_bytes(b"MKCOL")?, collection.clone()))
                            .await?;
                    require_status(&created, &[201, 405])?;
                }
                response = self.put_once(url, bytes).await?;
            }
            require_status(&response, &[200, 201, 204])
        })
        .await
    }

    async fn put_once(&self, url: Url, bytes: &[u8]) -> Result<Response> {
        send(
            self.request(Method::PUT, url)
                .header("Content-Type", "application/json")
                .header("If-None-Match", "*")
                .body(bytes.to_vec()),
        )
        .await
    }

    #[cfg(test)]
    pub(super) async fn delete(&self, key: &str) -> Result<()> {
        let url = self.object_url(key)?;
        deadline(async {
            let response = send(self.request(Method::DELETE, url)).await?;
            require_status(&response, &[200, 204])
        })
        .await
    }

    pub(super) async fn delete_verified(
        &self,
        key: &str,
        expected_bytes: &[u8],
        expected_etag: &str,
        current_revision: &(dyn Fn() -> bool + Send + Sync),
    ) -> Result<()> {
        if !self.capabilities.conditional_delete {
            bail!("WebDAV archive purge is disabled because atomic conditional deletion is unconfirmed");
        }
        let url = self.object_url(key)?;
        deadline(async {
            let response = send(self.request(Method::GET, url.clone())).await?;
            require_status(&response, &[200])?;
            let etag = response
                .headers()
                .get("etag")
                .filter(|value| value.to_str().ok().is_some_and(strong_etag))
                .cloned()
                .ok_or_else(|| anyhow!("WebDAV verified deletion requires a strong ETag"))?;
            if read_bounded(response).await? != expected_bytes {
                bail!("WebDAV object changed before verified deletion");
            }
            if etag.to_str().ok() != Some(expected_etag) {
                bail!("WebDAV archive version changed since purge was prepared");
            }
            if !current_revision() {
                bail!(
                    "Route revision changed during purge; completed deletions are not rolled back"
                );
            }
            let response = send(self.request(Method::DELETE, url).header("If-Match", etag)).await?;
            require_status(&response, &[200, 204])
        })
        .await
    }
}

fn strong_etag(value: &str) -> bool {
    value.len() >= 2
        && value.len() <= 1024
        && value.starts_with('"')
        && value.ends_with('"')
        && value.as_bytes()[1..value.len() - 1]
            .iter()
            .all(|byte| *byte > 0x20 && *byte != b'"' && *byte != 0x7f)
}

async fn deadline<T>(future: impl Future<Output = Result<T>>) -> Result<T> {
    tokio::time::timeout(NETWORK_DEADLINE, future)
        .await
        .map_err(|_| anyhow!("WebDAV operation timed out"))?
}

async fn send(request: RequestBuilder) -> Result<Response> {
    // HTTP client errors may contain URLs or authentication details; do not retain them.
    request.send().await.map_err(|error| {
        if error.is_timeout() {
            anyhow!("WebDAV operation timed out")
        } else {
            anyhow!("WebDAV request failed")
        }
    })
}

fn require_status(response: &Response, accepted: &[u16]) -> Result<()> {
    let status = response.status().as_u16();
    if status == 404 {
        return Err(StorageNotFound.into());
    }
    if !accepted.contains(&status) {
        bail!("WebDAV request returned HTTP {status}");
    }
    Ok(())
}

async fn read_bounded(response: Response) -> Result<Vec<u8>> {
    if response
        .content_length()
        .is_some_and(|size| size > MAX_OBJECT_BYTES as u64)
    {
        bail!("WebDAV response exceeds the size limit");
    }
    let stream = response.bytes_stream();
    futures::pin_mut!(stream);
    let mut bytes = Vec::new();
    while let Some(chunk) = stream.next().await {
        let chunk = chunk.map_err(|_| anyhow!("failed to read WebDAV response"))?;
        if chunk.len() > MAX_OBJECT_BYTES.saturating_sub(bytes.len()) {
            bail!("WebDAV response exceeds the size limit");
        }
        bytes
            .try_reserve(chunk.len())
            .map_err(|_| anyhow!("failed to read WebDAV response"))?;
        bytes.extend_from_slice(&chunk);
    }
    Ok(bytes)
}

#[cfg(test)]
#[path = "webdav_tests.rs"]
mod tests;
