//! Public installed-client defaults with explicit runtime overrides; never user account tokens.
use serde::Deserialize;
use std::{ffi::OsString, path::PathBuf, time::Duration};
use tokio::io::AsyncReadExt;

const MAX_BYTES: u64 = 8192;
const INVALID: &str = "antigravity_oauth_client_invalid";
const MISSING: &str = "antigravity_oauth_client_missing";

#[derive(Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OAuthClient {
    client_id: String,
    client_secret: String,
}

enum Source {
    Values(OAuthClient),
    File(PathBuf),
}

impl OAuthClient {
    fn builtin() -> Self {
        Self {
            client_id: super::CLIENT_ID.into(),
            client_secret: super::CLIENT_SECRET.into(),
        }
    }
    pub fn client_id(&self) -> &str {
        &self.client_id
    }
    pub fn client_secret(&self) -> &str {
        &self.client_secret
    }
    /// A configured client may only authenticate its own refresh at the fixed Google endpoint.
    pub fn refresh_secret(&self, endpoint: &str, client_id: &str) -> Result<&str, &'static str> {
        if endpoint != super::TOKEN_ENDPOINT || client_id != self.client_id {
            return Err("antigravity_oauth_client_mismatch");
        }
        Ok(&self.client_secret)
    }
    fn validated(self) -> Result<Self, &'static str> {
        for (value, limit) in [(&self.client_id, 1024), (&self.client_secret, 4096)] {
            if value.is_empty()
                || value.len() > limit
                || !value.bytes().all(|c| c.is_ascii_graphic())
            {
                return Err(INVALID);
            }
        }
        Ok(self)
    }
    pub async fn load() -> Result<Self, &'static str> {
        let source = source(
            std::env::var_os("GATEWAY_ANTIGRAVITY_OAUTH_CLIENT_ID"),
            std::env::var_os("GATEWAY_ANTIGRAVITY_OAUTH_CLIENT_SECRET"),
            std::env::var_os("GATEWAY_ANTIGRAVITY_OAUTH_CLIENT_FILE"),
            Self::builtin,
        )?;
        source.load().await
    }
}

fn source(
    id: Option<OsString>,
    secret: Option<OsString>,
    file: Option<OsString>,
    default: impl FnOnce() -> OAuthClient,
) -> Result<Source, &'static str> {
    match (id, secret) {
        (Some(id), Some(secret)) => {
            return OAuthClient {
                client_id: id.into_string().map_err(|_| INVALID)?,
                client_secret: secret.into_string().map_err(|_| INVALID)?,
            }
            .validated()
            .map(Source::Values);
        }
        (None, None) => {}
        _ => return Err(INVALID), // A partial override must never silently fall back.
    }
    let path = match file {
        Some(file) => PathBuf::from(file),
        None => return default().validated().map(Source::Values),
    };
    if !path.is_absolute() {
        return Err(INVALID);
    }
    Ok(Source::File(path))
}

impl Source {
    async fn load(self) -> Result<OAuthClient, &'static str> {
        let path = match self {
            Self::Values(client) => return Ok(client),
            Self::File(path) => path,
        };
        // Bound both filesystem work and the actual read (not just a racy metadata size).
        tokio::time::timeout(Duration::from_secs(3), async {
            let file = tokio::fs::File::open(path).await.map_err(|e| {
                if e.kind() == std::io::ErrorKind::NotFound {
                    MISSING
                } else {
                    INVALID
                }
            })?;
            let metadata = file.metadata().await.map_err(|_| INVALID)?;
            if !metadata.is_file() || metadata.len() > MAX_BYTES {
                return Err(INVALID);
            }
            let mut bytes = Vec::with_capacity(metadata.len() as usize);
            file.take(MAX_BYTES + 1)
                .read_to_end(&mut bytes)
                .await
                .map_err(|_| INVALID)?;
            if bytes.len() as u64 > MAX_BYTES {
                return Err(INVALID);
            }
            serde_json::from_slice::<OAuthClient>(&bytes)
                .map_err(|_| INVALID)?
                .validated()
        })
        .await
        .map_err(|_| "antigravity_oauth_client_timeout")?
    }
}

#[cfg(test)]
#[path = "oauth_client_tests.rs"]
mod tests;
