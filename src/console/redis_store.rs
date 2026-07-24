use std::fmt;

use crate::redis::keys;

use super::validate_redis_namespace;

#[derive(Clone, Debug, Eq, PartialEq, thiserror::Error)]
#[error("{message}")]
pub struct RouteConfigRedisStoreError {
    code: &'static str,
    message: String,
}

impl RouteConfigRedisStoreError {
    fn new(code: &'static str, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
        }
    }

    pub fn code(&self) -> &'static str {
        self.code
    }
}

#[derive(Clone, Eq, PartialEq)]
pub struct RouteConfigRedisKeys {
    namespace: String,
}

impl fmt::Debug for RouteConfigRedisKeys {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("RouteConfigRedisKeys")
            .field("namespace", &self.namespace)
            .field("updates_legacy_mirror", &self.updates_legacy_mirror())
            .finish()
    }
}

impl RouteConfigRedisKeys {
    pub fn new(namespace: impl Into<String>) -> Result<Self, RouteConfigRedisStoreError> {
        let namespace = namespace.into();
        validate_redis_namespace(&namespace)
            .map_err(|error| RouteConfigRedisStoreError::new(error.code(), error.to_string()))?;
        Ok(Self { namespace })
    }

    pub fn namespace(&self) -> &str {
        &self.namespace
    }

    pub fn active_revision_key(&self) -> String {
        keys::console_route_config_active_revision_key(&self.namespace)
    }

    pub fn active_document_key(&self) -> String {
        keys::console_route_config_active_document_key(&self.namespace)
    }

    pub fn revision_key(&self, revision: &str) -> String {
        keys::console_route_config_revision_key(&self.namespace, revision)
    }

    pub fn transaction_key(&self, tx_id: &str) -> String {
        keys::console_route_config_transaction_key(&self.namespace, tx_id)
    }

    pub fn events_key(&self) -> String {
        keys::console_route_config_events_key(&self.namespace)
    }

    pub fn updates_legacy_mirror(&self) -> bool {
        self.namespace == "default"
    }

    pub fn legacy_document_key(&self) -> Option<&'static str> {
        self.updates_legacy_mirror()
            .then_some(keys::LEGACY_ROUTE_CONFIG_DOCUMENT_KEY)
    }
}
