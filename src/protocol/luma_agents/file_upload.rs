//! Metadata for a bounded console-owned presigned upload; user_id is attribution, not ACL.
use super::files::{invalid, Purpose};
use crate::error::GatewayError;
use serde::{Deserialize, Serialize};
use time::{format_description::well_known::Rfc3339, OffsetDateTime};

pub const BUFFERED_UPLOAD: usize = 64 * 1024 * 1024;
pub const MAX_UPLOAD: usize = 5 * 1024 * 1024 * 1024;

/// A streaming upload keeps its producer in the request future; cancellation drops both sides.
pub struct UploadBody {
    pub body: rquest::Body,
    pub completion:
        std::pin::Pin<Box<dyn std::future::Future<Output = Result<(), GatewayError>> + Send>>,
    pub size: usize,
}

impl From<bytes::Bytes> for UploadBody {
    fn from(bytes: bytes::Bytes) -> Self {
        Self {
            size: bytes.len(),
            body: bytes.into(),
            completion: Box::pin(std::future::ready(Ok(()))),
        }
    }
}

pub fn upload_timeout(size: usize) -> std::time::Duration {
    std::time::Duration::from_secs(if size > BUFFERED_UPLOAD { 3600 } else { 180 })
}

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Upload {
    pub filename: String,
    pub mime_type: String,
    pub size_bytes: usize,
    pub purpose: Purpose,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expires_at: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub user_id: Option<String>,
}

impl Upload {
    pub fn validate(&self) -> Result<(), GatewayError> {
        if self.size_bytes == 0
            || self.size_bytes > MAX_UPLOAD
            || self.filename.is_empty()
            || self.filename.len() > 255
            || self
                .filename
                .chars()
                .any(|c| c.is_control() || matches!(c, '/' | '\\' | '"'))
            || self.mime_type.len() > 255
            || !self
                .mime_type
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'/' | b'-' | b'+' | b'.'))
            || !self.mime_type.split_once('/').is_some_and(|(kind, sub)| {
                matches!(kind, "image" | "video") && !sub.is_empty() && !sub.contains('/')
            })
            || self
                .user_id
                .as_ref()
                .is_some_and(|s| s.is_empty() || s.len() > 256 || s.chars().any(char::is_control))
            || self.expires_at.as_ref().is_some_and(|s| {
                s.len() > 64
                    || OffsetDateTime::parse(s, &Rfc3339)
                        .map_or(true, |t| t <= OffsetDateTime::now_utc())
            })
        {
            return Err(invalid());
        }
        Ok(())
    }
}
