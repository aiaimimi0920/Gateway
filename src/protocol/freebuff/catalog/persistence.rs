//! Restricted restoration of authenticated local receipts, never deserialization of a live capability.
use super::{atom, CatalogError, CatalogSession, DeviceKey, DeviceRegistration};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Receipt {
    version: u8,
    scope: [u8; 32],
    public_key: String,
    key_id: String,
}

impl CatalogSession {
    pub(crate) fn registration_scope(&self) -> [u8; 32] {
        self.binding.scope
    }
    pub(crate) fn registration_receipt(
        &self,
        device: &DeviceRegistration,
    ) -> Result<Vec<u8>, CatalogError> {
        device.check_binding(&self.binding)?;
        serde_json::to_vec(&Receipt {
            version: 1,
            scope: device.scope,
            public_key: device.key.public_key(),
            key_id: device.key_id.clone(),
        })
        .map_err(|_| CatalogError::InvalidResponse)
    }
    pub(crate) fn restore_registration(
        &self,
        key: Arc<DeviceKey>,
        bytes: &[u8],
    ) -> Result<DeviceRegistration, CatalogError> {
        if bytes.len() > 8192 {
            return Err(CatalogError::InvalidResponse);
        }
        let receipt: Receipt =
            serde_json::from_slice(bytes).map_err(|_| CatalogError::InvalidResponse)?;
        if receipt.version != 1 || !atom(&receipt.key_id, 4096) {
            return Err(CatalogError::InvalidResponse);
        }
        if receipt.scope != self.binding.scope || receipt.public_key != key.public_key() {
            return Err(CatalogError::ScopeMismatch);
        }
        Ok(DeviceRegistration::new(&self.binding, key, receipt.key_id))
    }
}
