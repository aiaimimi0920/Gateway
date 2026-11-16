//! Explicit manual transfer of encrypted backups; never reuse inference storage credentials.
mod github;
mod s3;
#[cfg(test)]
mod tests;
mod webdav;

use serde::{Deserialize, Serialize};
use std::time::Duration;

pub(super) const LIMIT: usize = super::MAX_BACKUP_BYTES + 52;
pub(super) const DEADLINE: Duration = Duration::from_secs(60);

// Deliberately no Debug: destinations carry user-supplied authentication secrets.
#[derive(Clone, Deserialize, Serialize)]
#[serde(tag = "kind", rename_all = "camelCase", deny_unknown_fields)]
pub enum Destination {
    Webdav {
        url: String,
        username: String,
        password: String,
    },
    S3 {
        endpoint: String,
        region: String,
        bucket: String,
        key: String,
        #[serde(rename = "accessKeyId")]
        access_key_id: String,
        #[serde(rename = "secretAccessKey")]
        secret_access_key: String,
        #[serde(
            rename = "sessionToken",
            default,
            skip_serializing_if = "Option::is_none"
        )]
        session_token: Option<String>,
    },
    Github {
        owner: String,
        repository: String,
        branch: String,
        path: String,
        token: String,
    },
}

pub struct Download {
    pub bytes: Vec<u8>,
    /// Opaque remote revision used only for a later explicit conditional upload.
    pub revision: String,
}

/// No background retry or merge: a conflict must be resolved explicitly by the operator.
pub async fn upload(
    destination: &Destination,
    bytes: Vec<u8>,
    expected: Option<&str>,
) -> Result<(), String> {
    validate_envelope(&bytes)?;
    destination.validate()?;
    tokio::time::timeout(DEADLINE, async {
        match destination {
            Destination::Webdav { .. } => webdav::upload(destination, bytes, expected).await,
            Destination::S3 { .. } => s3::upload(destination, bytes, expected).await,
            Destination::Github { .. } => github::upload(destination, bytes, expected).await,
        }
    })
    .await
    .map_err(|_| "Backup upload timed out; check remote state before retrying".to_owned())?
}

pub async fn download(destination: &Destination) -> Result<Download, String> {
    destination.validate()?;
    let result = tokio::time::timeout(DEADLINE, async {
        match destination {
            Destination::Webdav { .. } => webdav::download(destination).await,
            Destination::S3 { .. } => s3::download(destination).await,
            Destination::Github { .. } => github::download(destination).await,
        }
    })
    .await
    .map_err(|_| "Backup download timed out".to_owned())??;
    validate_envelope(&result.bytes)?;
    Ok(result)
}

impl Destination {
    pub fn validate(&self) -> Result<(), String> {
        match self {
            Self::Webdav {
                url,
                username,
                password,
            } => {
                secure_url(url)?;
                bounded(username, 1024)?;
                bounded(password, 8192)?;
            }
            Self::S3 {
                endpoint,
                region,
                bucket,
                key,
                access_key_id,
                secret_access_key,
                session_token,
            } => {
                secure_url(endpoint)?;
                for value in [region, bucket, access_key_id, secret_access_key] {
                    nonempty(value, 1024)?;
                }
                object_path(key)?;
                if let Some(token) = session_token {
                    nonempty(token, 8192)?;
                }
            }
            Self::Github {
                owner,
                repository,
                branch,
                path,
                token,
            } => {
                for value in [owner, repository] {
                    nonempty(value, 100)?;
                    if !value
                        .bytes()
                        .all(|b| b.is_ascii_alphanumeric() || b"-_.".contains(&b))
                        || value == "."
                        || value == ".."
                    {
                        return Err("Invalid GitHub repository".into());
                    }
                }
                nonempty(branch, 255)?;
                object_path(path)?;
                nonempty(token, 8192)?;
            }
        }
        Ok(())
    }
}

fn bounded(value: &str, limit: usize) -> Result<(), String> {
    if value.len() > limit || value.chars().any(char::is_control) {
        return Err("Invalid backup destination field".into());
    }
    Ok(())
}
fn nonempty(value: &str, limit: usize) -> Result<(), String> {
    bounded(value, limit)?;
    if value.trim().is_empty() {
        return Err("Missing backup destination field".into());
    }
    Ok(())
}
fn secure_url(value: &str) -> Result<(), String> {
    nonempty(value, 4096)?;
    let url = url::Url::parse(value).map_err(|_| "Invalid backup URL")?;
    if url.scheme() != "https"
        || url.host_str().is_none()
        || !url.username().is_empty()
        || url.password().is_some()
        || url.fragment().is_some()
        || url.query().is_some()
    {
        return Err(
            "Backup URL must use HTTPS without embedded credentials, query or fragment".into(),
        );
    }
    Ok(())
}
fn object_path(path: &str) -> Result<(), String> {
    nonempty(path, 1024)?;
    if path.contains('\\')
        || path
            .split('/')
            .any(|part| part.is_empty() || part == "." || part == "..")
    {
        return Err("Invalid backup object path".into());
    }
    Ok(())
}
fn validate_envelope(bytes: &[u8]) -> Result<(), String> {
    if bytes.len() < 52 || bytes.len() > LIMIT || &bytes[..8] != b"NGBK\0\0\0\x01" {
        return Err("Only encrypted Gateway backups can be transferred".into());
    }
    Ok(())
}

pub(super) fn client() -> Result<reqwest::Client, String> {
    reqwest::Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .connect_timeout(Duration::from_secs(10))
        .timeout(DEADLINE)
        .user_agent("Neuro-Gateway-Backup")
        .build()
        .map_err(|_| "Cannot initialize backup transport".into())
}

pub(super) async fn read(mut response: reqwest::Response, limit: usize) -> Result<Vec<u8>, String> {
    if !response.status().is_success() {
        return Err(format!(
            "Backup service returned HTTP {}",
            response.status().as_u16()
        ));
    }
    if response.content_length().is_some_and(|n| n > limit as u64) {
        return Err("Remote backup exceeds size limit".into());
    }
    let mut bytes = Vec::new();
    while let Some(chunk) = response
        .chunk()
        .await
        .map_err(|_| "Cannot read remote backup")?
    {
        if chunk.len() > limit.saturating_sub(bytes.len()) {
            return Err("Remote backup exceeds size limit".into());
        }
        bytes.extend_from_slice(&chunk);
    }
    Ok(bytes)
}

pub(super) fn revision(response: &reqwest::Response) -> Result<String, String> {
    response
        .headers()
        .get(reqwest::header::ETAG)
        .and_then(|v| v.to_str().ok())
        .filter(|value| !value.is_empty() && !value.starts_with("W/"))
        .map(str::to_owned)
        .ok_or_else(|| "Backup service did not provide a strong revision".into())
}
