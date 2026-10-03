//! Additive per-provider storage connection contracts. Debug output never contains auth.
use serde::{Deserialize, Serialize};
use std::fmt;

#[derive(Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum CredentialStorageConnection {
    Local {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        path: Option<String>,
    },
    Webdav {
        endpoint: String,
        #[serde(default)]
        directory: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        username: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        password: Option<String>,
        #[serde(default)]
        allow_insecure_http: bool,
    },
    S3 {
        endpoint: String,
        bucket: String,
        region: String,
        #[serde(default)]
        prefix: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        access_key_id: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        secret_access_key: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        session_token: Option<String>,
        #[serde(default)]
        allow_insecure_http: bool,
    },
}

impl CredentialStorageConnection {
    pub fn is_remote(&self) -> bool {
        !matches!(self, Self::Local { .. })
    }

    pub(crate) fn secret_fields(&self) -> Vec<(&'static str, Option<&str>)> {
        match self {
            Self::Local { .. } => Vec::new(),
            Self::Webdav { password, .. } => vec![("password", password.as_deref())],
            Self::S3 {
                access_key_id,
                secret_access_key,
                session_token,
                ..
            } => vec![
                ("access_key_id", access_key_id.as_deref()),
                ("secret_access_key", secret_access_key.as_deref()),
                ("session_token", session_token.as_deref()),
            ],
        }
    }

    pub(crate) fn clear_secrets(&mut self) {
        match self {
            Self::Local { .. } => {}
            Self::Webdav { password, .. } => *password = None,
            Self::S3 {
                access_key_id,
                secret_access_key,
                session_token,
                ..
            } => {
                *access_key_id = None;
                *secret_access_key = None;
                *session_token = None;
            }
        }
    }
}

impl fmt::Debug for CredentialStorageConnection {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let kind = match self {
            Self::Local { .. } => "local",
            Self::Webdav { .. } => "webdav",
            Self::S3 { .. } => "s3",
        };
        f.debug_struct("CredentialStorageConnection")
            .field("type", &kind)
            .finish_non_exhaustive()
    }
}
