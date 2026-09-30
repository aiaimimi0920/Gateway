//! Validated Redis route revisions, key namespaces and storage error contracts.

use crate::console::{
    document::validate_route_document, revision::RevisionMetadata, validate_redis_namespace,
};
use crate::redis::keys;
use crate::routing::config::{ActiveConfigSource, RouteConfigStore, RouteConfigYaml};
use serde::{Deserialize, Serialize};
use std::fmt;

#[derive(Clone, Debug, Eq, PartialEq, thiserror::Error)]
#[error("{message}")]
pub struct RouteConfigRedisStoreError {
    code: &'static str,
    message: String,
}

impl RouteConfigRedisStoreError {
    pub(crate) fn from_persistence(error: crate::console::PersistenceError) -> Self {
        Self::new(error.code(), error.to_string())
    }

    fn new(code: &'static str, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
        }
    }

    pub(super) fn unavailable(message: impl Into<String>) -> Self {
        Self::new("console_redis_unavailable", message)
    }

    pub(super) fn invalid_state(message: impl Into<String>) -> Self {
        Self::new("console_redis_invalid_state", message)
    }

    pub(super) fn unsupported_cluster(message: impl Into<String>) -> Self {
        Self::new("console_redis_cluster_unsupported", message)
    }

    fn from_route_config_error(error: impl std::error::Error) -> Self {
        Self::invalid_state(error.to_string())
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

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RouteConfigRedisRevision {
    metadata: RevisionMetadata,
    document: RouteConfigYaml,
}

impl RouteConfigRedisRevision {
    pub fn new(
        metadata: RevisionMetadata,
        document: RouteConfigYaml,
    ) -> Result<Self, RouteConfigRedisStoreError> {
        metadata
            .validate()
            .map_err(RouteConfigRedisStoreError::from_route_config_error)?;
        let validated = validate_route_document(document).map_err(|diagnostics| {
            RouteConfigRedisStoreError::invalid_state(diagnostics.to_string())
        })?;
        if metadata.document_digest() != validated.document_digest()
            || metadata.yaml_digest() != validated.yaml_digest()
        {
            return Err(RouteConfigRedisStoreError::invalid_state(
                "Redis route revision metadata does not match the validated document digests",
            ));
        }
        Ok(Self {
            metadata,
            document: validated.document().clone(),
        })
    }

    pub fn from_json(raw: &str) -> Result<Self, RouteConfigRedisStoreError> {
        let wire: Self = serde_json::from_str(raw).map_err(|error| {
            RouteConfigRedisStoreError::invalid_state(format!(
                "Redis route revision JSON is malformed: {error}"
            ))
        })?;
        Self::new(wire.metadata, wire.document)
    }

    pub fn to_json(&self) -> Result<String, RouteConfigRedisStoreError> {
        serialize_json_canonically(self, "Redis route revision JSON serialization failed")
    }

    pub fn metadata(&self) -> &RevisionMetadata {
        &self.metadata
    }

    pub fn document(&self) -> &RouteConfigYaml {
        &self.document
    }

    pub fn into_store(
        self,
        source: ActiveConfigSource,
    ) -> Result<RouteConfigStore, RouteConfigRedisStoreError> {
        let validated = validate_route_document(self.document).map_err(|diagnostics| {
            RouteConfigRedisStoreError::invalid_state(diagnostics.to_string())
        })?;
        RouteConfigStore::from_external_validated(validated, self.metadata, source)
            .map_err(RouteConfigRedisStoreError::from_route_config_error)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RouteConfigRedisActivationOutcome {
    Activated,
    AlreadyActive,
    RevisionConflict { actual: Option<String> },
    PreparedTransactionMismatch,
    RevisionPayloadMismatch,
    Indeterminate,
    Unavailable,
}

pub(super) fn serialize_json_canonically<T: Serialize>(
    value: &T,
    context: &str,
) -> Result<String, RouteConfigRedisStoreError> {
    let value = serde_json::to_value(value).map_err(|error| {
        RouteConfigRedisStoreError::invalid_state(format!("{context}: {error}"))
    })?;
    serde_json::to_string(&value)
        .map_err(|error| RouteConfigRedisStoreError::invalid_state(format!("{context}: {error}")))
}
