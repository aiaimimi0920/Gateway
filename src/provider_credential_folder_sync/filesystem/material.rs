//! Encoded material I/O and export serialization, separate from path containment.
use crate::error::GatewayError;
use crate::provider_credential_folder_sync::limits::{
    ensure_material_size, MATERIAL_ALLOCATION_FAILED,
};
use serde_json::Value;
use std::io::{Read, Write};
use std::path::Path;

pub(in crate::provider_credential_folder_sync) fn read_material_bytes(
    path: &Path,
    limit: usize,
) -> Result<Vec<u8>, GatewayError> {
    let file = std::fs::File::open(path).map_err(|error| {
        GatewayError::server_error(format!(
            "read provider credential file {}: {error}",
            path.display()
        ))
    })?;
    let length = file
        .metadata()
        .map_err(|error| {
            GatewayError::server_error(format!(
                "inspect provider credential file {}: {error}",
                path.display()
            ))
        })?
        .len();
    read_bounded(file, Some(length), limit)
}

pub(in crate::provider_credential_folder_sync) fn read_bounded<R: Read>(
    mut reader: R,
    known_length: Option<u64>,
    limit: usize,
) -> Result<Vec<u8>, GatewayError> {
    if let Some(length) = known_length {
        ensure_material_size(length, limit)?;
    }
    let mut bytes = Vec::new();
    let mut chunk = [0_u8; 16 * 1024];
    loop {
        // Read at most one excess byte, even when metadata is stale or absent.
        let requested = (limit - bytes.len()).saturating_add(1).min(chunk.len());
        let count = match reader.read(&mut chunk[..requested]) {
            Ok(count) => count,
            Err(error) if error.kind() == std::io::ErrorKind::Interrupted => continue,
            Err(error) => {
                return Err(GatewayError::server_error(format!(
                    "read provider credential material: {error}"
                )))
            }
        };
        if count == 0 {
            return Ok(bytes);
        }
        append(&mut bytes, &chunk[..count], limit)?;
    }
}

pub(in crate::provider_credential_folder_sync) fn serialize_export_payload(
    payload: &Value,
    limit: usize,
) -> Result<Vec<u8>, GatewayError> {
    let mut writer = MaterialWriter {
        bytes: Vec::new(),
        limit,
        failure: None,
    };
    let result = serde_json::to_writer_pretty(&mut writer, payload);
    if let Some(error) = writer.failure {
        return Err(error);
    }
    result.map_err(|error| {
        GatewayError::server_error(format!(
            "serialize provider credential for folder sync: {error}"
        ))
    })?;
    Ok(writer.bytes)
}

fn append(bytes: &mut Vec<u8>, data: &[u8], limit: usize) -> Result<(), GatewayError> {
    let length = bytes.len().checked_add(data.len()).ok_or_else(|| {
        GatewayError::bad_request("provider credential material length overflow")
            .with_code(super::super::limits::MATERIAL_TOO_LARGE)
    })?;
    ensure_material_size(length as u64, limit)?;
    if length > bytes.capacity() {
        // Grow geometrically without reserving beyond the material budget.
        let capacity = length
            .max(bytes.capacity().saturating_mul(2).max(4096))
            .min(limit);
        bytes
            .try_reserve_exact(capacity - bytes.len())
            .map_err(|_| {
                GatewayError::service_unavailable("allocate provider credential material buffer")
                    .with_code(MATERIAL_ALLOCATION_FAILED)
            })?;
    }
    bytes.extend_from_slice(data);
    Ok(())
}

struct MaterialWriter {
    bytes: Vec<u8>,
    limit: usize,
    failure: Option<GatewayError>,
}

impl Write for MaterialWriter {
    fn write(&mut self, data: &[u8]) -> std::io::Result<usize> {
        if let Err(error) = append(&mut self.bytes, data, self.limit) {
            self.failure = Some(error);
            return Err(std::io::Error::other(
                "provider credential material budget exceeded",
            ));
        }
        Ok(data.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}
