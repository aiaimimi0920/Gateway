//! One metadata part followed by one binary part, admitted before any body allocation.
mod ingress;
mod streaming;
use super::Input;
use crate::{
    error::GatewayError,
    protocol::luma_agents::{
        file_upload::{Upload, BUFFERED_UPLOAD},
        files::Operation,
    },
    state::AppState,
};
use axum::extract::{FromRequest, Multipart, Request};
use bytes::BytesMut;
use serde::Deserialize;
use std::sync::Arc;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Metadata {
    provider_id: String,
    credential_id: String,
    expected_revision: String,
    upload: Upload,
}

pub(super) async fn parse(request: Request, state: &Arc<AppState>) -> Result<Input, GatewayError> {
    let invalid = || GatewayError::bad_request("Invalid Luma upload multipart body.");
    let (request, budget) = ingress::bounded(request);
    let mut form = Multipart::from_request(request, state)
        .await
        .map_err(|_| invalid())?;
    let mut field = form
        .next_field()
        .await
        .map_err(|_| invalid())?
        .ok_or_else(invalid)?;
    if field.name() != Some("metadata") {
        return Err(invalid());
    }
    let mut json = BytesMut::new();
    while let Some(chunk) = field.chunk().await.map_err(|_| invalid())? {
        if json.len() + chunk.len() > 4096 {
            return Err(invalid());
        }
        json.extend_from_slice(&chunk);
    }
    // Consuming all chunks does not release multer's field lock; drop before next_field.
    drop(field);
    let metadata: Metadata = serde_json::from_slice(&json).map_err(|_| invalid())?;
    metadata.upload.validate()?;
    if metadata.upload.size_bytes > BUFFERED_UPLOAD {
        let bytes = streaming::body(form, metadata.upload.size_bytes, budget);
        return Ok(Input {
            provider_id: metadata.provider_id,
            credential_id: metadata.credential_id,
            expected_revision: metadata.expected_revision,
            operation: Operation::Presigned {
                metadata: metadata.upload,
                bytes,
            },
        });
    }
    let mut field = form
        .next_field()
        .await
        .map_err(|_| invalid())?
        .ok_or_else(invalid)?;
    if field.name() != Some("file") {
        return Err(invalid());
    }
    budget.file(metadata.upload.size_bytes);
    // Exact declared size bounds both allocation and subsequent PUT; no base64 expansion or disk spool.
    let mut bytes = BytesMut::with_capacity(metadata.upload.size_bytes);
    while let Some(chunk) = field.chunk().await.map_err(|_| invalid())? {
        if bytes.len() + chunk.len() > metadata.upload.size_bytes
            || bytes.len() + chunk.len() > BUFFERED_UPLOAD
        {
            return Err(invalid());
        }
        bytes.extend_from_slice(&chunk);
    }
    drop(field);
    if bytes.len() != metadata.upload.size_bytes
        || form.next_field().await.map_err(|_| invalid())?.is_some()
    {
        return Err(invalid());
    }
    Ok(Input {
        provider_id: metadata.provider_id,
        credential_id: metadata.credential_id,
        expected_revision: metadata.expected_revision,
        operation: Operation::Presigned {
            metadata: metadata.upload,
            bytes: bytes.freeze().into(),
        },
    })
}
