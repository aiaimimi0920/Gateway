use std::path::Path;

use tokio::io::{AsyncRead, AsyncReadExt};

use crate::error::GatewayError;

pub(crate) const STANDARD_OBJECT_LIMIT_BYTES: usize = 32 * 1024 * 1024;
// Export datasets contain many bounded rows, so they receive a larger but still
// finite budget instead of weakening the limit for credentials and state JSON.
const DATASET_OBJECT_LIMIT_BYTES: usize = 256 * 1024 * 1024;
const INITIAL_RESERVE_LIMIT_BYTES: usize = 1024 * 1024;
const READ_CHUNK_BYTES: usize = 16 * 1024;

pub(super) fn limit_for_object_key(object_key: &str) -> usize {
    let mut segments = object_key.split('/');
    let is_gateway_dataset = matches!(
        (
            segments.next(),
            segments.next(),
            segments.next(),
            segments.next(),
            segments.next(),
        ),
        (
            Some("ai-gateway"),
            Some(
                "analysis-exports"
                    | "conversation-archive-exports"
                    | "conversation-dataset-exports"
            ),
            Some(export_id),
            Some("dataset.jsonl"),
            None,
        ) if !export_id.is_empty()
    );
    if is_gateway_dataset {
        DATASET_OBJECT_LIMIT_BYTES
    } else {
        STANDARD_OBJECT_LIMIT_BYTES
    }
}

pub(super) fn ensure_object_size(size: u64, limit: usize) -> Result<(), GatewayError> {
    if size <= limit as u64 {
        return Ok(());
    }

    Err(
        GatewayError::service_unavailable(format!("object payload exceeds {limit} byte limit"))
            .with_code("object_storage_object_too_large"),
    )
}

pub(super) async fn read_local_file(path: &Path, limit: usize) -> Result<Vec<u8>, GatewayError> {
    let file = tokio::fs::File::open(path).await.map_err(|error| {
        GatewayError::service_unavailable(format!("read object payload: {error}"))
    })?;
    let length = file.metadata().await.map_err(|error| {
        GatewayError::service_unavailable(format!("inspect object payload length: {error}"))
    })?;
    read_bounded(file, Some(length.len()), limit).await
}

pub(super) async fn read_bounded<R>(
    reader: R,
    known_length: Option<u64>,
    limit: usize,
) -> Result<Vec<u8>, GatewayError>
where
    R: AsyncRead,
{
    if let Some(length) = known_length {
        ensure_object_size(length, limit)?;
    }

    let initial_capacity = known_length
        .and_then(|length| usize::try_from(length).ok())
        .map(|length| length.min(INITIAL_RESERVE_LIMIT_BYTES))
        .unwrap_or(0);
    let mut bytes = Vec::new();
    bytes.try_reserve_exact(initial_capacity).map_err(|_| {
        GatewayError::service_unavailable("allocate object payload buffer")
            .with_code("object_storage_buffer_allocation_failed")
    })?;

    // The advertised length is only a fast rejection hint. Keep counting actual
    // bytes to handle stale local metadata and incorrect S3 Content-Length data.
    let mut reader = std::pin::pin!(reader);
    let mut chunk = [0_u8; READ_CHUNK_BYTES];
    loop {
        let read = reader.read(&mut chunk).await.map_err(|error| {
            GatewayError::service_unavailable(format!("read object payload: {error}"))
                .with_code("object_storage_read_failed")
        })?;
        if read == 0 {
            return Ok(bytes);
        }

        let next_length = bytes.len().checked_add(read).ok_or_else(|| {
            GatewayError::service_unavailable("object payload length overflow")
                .with_code("object_storage_object_too_large")
        })?;
        ensure_object_size(next_length as u64, limit)?;
        bytes.try_reserve(read).map_err(|_| {
            GatewayError::service_unavailable("grow object payload buffer")
                .with_code("object_storage_buffer_allocation_failed")
        })?;
        bytes.extend_from_slice(&chunk[..read]);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use uuid::Uuid;

    #[test]
    fn object_limits_are_generous_only_for_datasets() {
        assert_eq!(
            limit_for_object_key("ai-gateway/analysis-exports/id/dataset.jsonl"),
            DATASET_OBJECT_LIMIT_BYTES
        );
        assert_eq!(
            limit_for_object_key("ai-gateway/conversation-archive-exports/id/dataset.jsonl"),
            DATASET_OBJECT_LIMIT_BYTES
        );
        assert_eq!(
            limit_for_object_key("ai-gateway/conversation-dataset-exports/id/dataset.jsonl"),
            DATASET_OBJECT_LIMIT_BYTES
        );
        assert_eq!(
            limit_for_object_key("ai-gateway/conversation-archives/id/response.json"),
            STANDARD_OBJECT_LIMIT_BYTES
        );
        assert_eq!(
            limit_for_object_key("ai-gateway/dataset.jsonl.backup"),
            STANDARD_OBJECT_LIMIT_BYTES
        );
        assert_eq!(
            limit_for_object_key("untrusted/export/dataset.jsonl"),
            STANDARD_OBJECT_LIMIT_BYTES
        );
        assert_eq!(
            limit_for_object_key("ai-gateway/analysis-exports//dataset.jsonl"),
            STANDARD_OBJECT_LIMIT_BYTES
        );
    }

    #[test]
    fn object_size_check_accepts_exact_limit_and_rejects_excess() {
        ensure_object_size(8, 8).expect("exact limit must be accepted");
        let error = ensure_object_size(9, 8).expect_err("excess must be rejected");

        assert_eq!(
            error.code.as_deref(),
            Some("object_storage_object_too_large")
        );
    }

    #[tokio::test]
    async fn bounded_reader_accepts_payload_at_limit() {
        let payload = b"12345678";
        let bytes = read_bounded(&payload[..], Some(payload.len() as u64), payload.len())
            .await
            .expect("payload at limit must be accepted");

        assert_eq!(bytes, payload);
    }

    #[tokio::test]
    async fn bounded_reader_rejects_known_oversized_payload() {
        let error = read_bounded(&b"ignored"[..], Some(9), 8)
            .await
            .expect_err("known oversized payload must be rejected");

        assert_eq!(
            error.code.as_deref(),
            Some("object_storage_object_too_large")
        );
    }

    #[tokio::test]
    async fn bounded_reader_rechecks_bytes_while_streaming() {
        let error = read_bounded(&b"123456789"[..], None, 8)
            .await
            .expect_err("streamed oversized payload must be rejected");

        assert_eq!(
            error.code.as_deref(),
            Some("object_storage_object_too_large")
        );
    }

    #[tokio::test]
    async fn local_reader_rejects_oversized_file_from_metadata() {
        let path =
            std::env::temp_dir().join(format!("neuro-gateway-object-limit-{}", Uuid::new_v4()));
        tokio::fs::write(&path, b"123456789")
            .await
            .expect("write oversized local object");

        let error = read_local_file(&path, 8)
            .await
            .expect_err("oversized local object must be rejected");

        assert_eq!(
            error.code.as_deref(),
            Some("object_storage_object_too_large")
        );
        tokio::fs::remove_file(path)
            .await
            .expect("remove oversized local object");
    }
}
