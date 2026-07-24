use std::fmt;

use deadpool_redis::Pool;
use redis::AsyncCommands;
use serde::{Deserialize, Serialize};

use crate::console::document::validate_route_document;
use crate::console::revision::RevisionMetadata;
use crate::redis::keys;
use crate::routing::config::{ActiveConfigSource, RouteConfigStore, RouteConfigYaml};

use super::{validate_redis_namespace, TransactionPhase, TransactionRecord};

const ACTIVATE_SENTINEL_NONE: &str = "__NONE__";
const ACTIVATE_LUA: &str = r#"
local current = redis.call('GET', KEYS[1])
if not current then current = '' end
if ARGV[1] == '__NONE__' then
  if current ~= '' then
    if current == ARGV[2] then
      return {'already_active'}
    end
    return {'revision_conflict', current}
  end
elseif current ~= ARGV[1] then
  if current == ARGV[2] then
    return {'already_active'}
  end
  return {'revision_conflict', current}
end
local stored_revision = redis.call('GET', KEYS[3])
if stored_revision ~= ARGV[6] then
  return {'revision_payload_mismatch'}
end
local stored_transaction = redis.call('GET', KEYS[4])
if stored_transaction == ARGV[5] and current == ARGV[2] then
  return {'already_active'}
end
if stored_transaction ~= ARGV[4] then
  return {'prepared_transaction_mismatch'}
end
redis.call('SET', KEYS[2], ARGV[3])
if KEYS[5] ~= '' then
  redis.call('SET', KEYS[5], ARGV[3])
end
redis.call('SET', KEYS[4], ARGV[5])
redis.call('SET', KEYS[1], ARGV[2])
return {'activated'}
"#;

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

    fn unavailable(message: impl Into<String>) -> Self {
        Self::new("console_redis_unavailable", message)
    }

    fn invalid_state(message: impl Into<String>) -> Self {
        Self::new("console_redis_invalid_state", message)
    }

    fn unsupported_cluster(message: impl Into<String>) -> Self {
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
        serde_json::to_string(self).map_err(|error| {
            RouteConfigRedisStoreError::invalid_state(format!(
                "Redis route revision JSON serialization failed: {error}"
            ))
        })
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

#[derive(Clone, Debug)]
pub struct RouteConfigRedisStore {
    keys: RouteConfigRedisKeys,
}

impl RouteConfigRedisStore {
    pub fn new(namespace: impl Into<String>) -> Result<Self, RouteConfigRedisStoreError> {
        Ok(Self {
            keys: RouteConfigRedisKeys::new(namespace)?,
        })
    }

    pub fn keys(&self) -> &RouteConfigRedisKeys {
        &self.keys
    }

    pub async fn store_revision(
        &self,
        pool: &Pool,
        revision: &RouteConfigRedisRevision,
    ) -> Result<(), RouteConfigRedisStoreError> {
        self.ensure_single_node(pool).await?;
        let mut connection = pool.get().await.map_err(|error| {
            RouteConfigRedisStoreError::unavailable(format!(
                "Gateway console Redis connection failed: {error}"
            ))
        })?;
        let key = self.keys.revision_key(revision.metadata().id());
        let value = revision.to_json()?;
        store_json_if_absent_or_identical(&mut connection, &key, &value).await
    }

    pub async fn store_prepared_transaction(
        &self,
        pool: &Pool,
        record: &TransactionRecord,
    ) -> Result<(), RouteConfigRedisStoreError> {
        if record.phase() != TransactionPhase::Prepared {
            return Err(RouteConfigRedisStoreError::invalid_state(
                "Gateway console Redis prepared transaction must be in Prepared phase",
            ));
        }
        self.ensure_single_node(pool).await?;
        let mut connection = pool.get().await.map_err(|error| {
            RouteConfigRedisStoreError::unavailable(format!(
                "Gateway console Redis connection failed: {error}"
            ))
        })?;
        let key = self.keys.transaction_key(record.tx_id());
        let value = serialize_transaction_record(record)?;
        store_json_if_absent_or_identical(&mut connection, &key, &value).await
    }

    pub async fn activate_revision(
        &self,
        pool: &Pool,
        expected_active_revision: Option<&str>,
        revision: &RouteConfigRedisRevision,
        prepared_record: &TransactionRecord,
        activated_record: &TransactionRecord,
    ) -> Result<RouteConfigRedisActivationOutcome, RouteConfigRedisStoreError> {
        if prepared_record.phase() != TransactionPhase::Prepared {
            return Err(RouteConfigRedisStoreError::invalid_state(
                "Gateway console Redis CAS expects a Prepared transaction record",
            ));
        }
        if activated_record.phase() != TransactionPhase::RedisActivated {
            return Err(RouteConfigRedisStoreError::invalid_state(
                "Gateway console Redis CAS expects a RedisActivated transaction record",
            ));
        }
        if prepared_record.tx_id() != activated_record.tx_id()
            || prepared_record.new_revision() != activated_record.new_revision()
        {
            return Err(RouteConfigRedisStoreError::invalid_state(
                "Gateway console CAS transaction identities do not match",
            ));
        }
        self.ensure_single_node(pool).await?;

        let revision_json = revision.to_json()?;
        let active_document_json = serde_json::to_string(revision.document()).map_err(|error| {
            RouteConfigRedisStoreError::invalid_state(format!(
                "Gateway console active document JSON serialization failed: {error}"
            ))
        })?;
        let prepared_json = serialize_transaction_record(prepared_record)?;
        let activated_json = serialize_transaction_record(activated_record)?;

        let mut connection = match pool.get().await {
            Ok(connection) => connection,
            Err(_) => return Ok(RouteConfigRedisActivationOutcome::Unavailable),
        };

        let script = redis::Script::new(ACTIVATE_LUA);
        let mut invocation = script.prepare_invoke();
        invocation
            .key(self.keys.active_revision_key())
            .key(self.keys.active_document_key())
            .key(self.keys.revision_key(revision.metadata().id()))
            .key(self.keys.transaction_key(prepared_record.tx_id()))
            .key(self.keys.legacy_document_key().unwrap_or(""))
            .arg(expected_active_revision.unwrap_or(ACTIVATE_SENTINEL_NONE))
            .arg(revision.metadata().id())
            .arg(&active_document_json)
            .arg(&prepared_json)
            .arg(&activated_json)
            .arg(&revision_json);
        let response = invocation
            .invoke_async::<Vec<String>>(&mut connection)
            .await;
        let outcome = match response {
            Ok(values) => parse_activation_response(&values)?,
            Err(_) => return Ok(RouteConfigRedisActivationOutcome::Indeterminate),
        };
        if outcome == RouteConfigRedisActivationOutcome::Activated {
            let _: i64 = redis::cmd("PUBLISH")
                .arg(self.keys.events_key())
                .arg(revision.metadata().id())
                .query_async(&mut connection)
                .await
                .map_err(|error| {
                    RouteConfigRedisStoreError::unavailable(format!(
                        "Gateway console revision publish failed: {error}"
                    ))
                })?;
        }
        Ok(outcome)
    }

    pub async fn load_active_revision(
        &self,
        pool: &Pool,
    ) -> Result<Option<RouteConfigRedisRevision>, RouteConfigRedisStoreError> {
        self.ensure_single_node(pool).await?;
        let mut connection = pool.get().await.map_err(|error| {
            RouteConfigRedisStoreError::unavailable(format!(
                "Gateway console Redis connection failed: {error}"
            ))
        })?;
        let active_revision: Option<String> = connection
            .get(self.keys.active_revision_key())
            .await
            .map_err(|error| {
                RouteConfigRedisStoreError::unavailable(format!(
                    "Gateway console active revision lookup failed: {error}"
                ))
            })?;
        let Some(active_revision) = active_revision else {
            return Ok(None);
        };
        let revision_payload: Option<String> = connection
            .get(self.keys.revision_key(&active_revision))
            .await
            .map_err(|error| {
                RouteConfigRedisStoreError::unavailable(format!(
                    "Gateway console revision payload lookup failed: {error}"
                ))
            })?;
        let active_document: Option<String> = connection
            .get(self.keys.active_document_key())
            .await
            .map_err(|error| {
                RouteConfigRedisStoreError::unavailable(format!(
                    "Gateway console active document lookup failed: {error}"
                ))
            })?;
        Self::decode_active_revision_state(
            &active_revision,
            revision_payload.as_deref(),
            active_document.as_deref(),
        )
        .map(Some)
    }

    pub async fn load_active_store(
        &self,
        pool: &Pool,
    ) -> Result<Option<RouteConfigStore>, RouteConfigRedisStoreError> {
        let Some(revision) = self.load_active_revision(pool).await? else {
            return Ok(None);
        };
        revision.into_store(ActiveConfigSource::Redis).map(Some)
    }

    pub fn decode_active_revision_state(
        active_revision: &str,
        revision_payload: Option<&str>,
        active_document: Option<&str>,
    ) -> Result<RouteConfigRedisRevision, RouteConfigRedisStoreError> {
        let revision_payload = revision_payload.ok_or_else(|| {
            RouteConfigRedisStoreError::invalid_state(
                "Gateway console active revision is missing its immutable Redis payload",
            )
        })?;
        let revision = RouteConfigRedisRevision::from_json(revision_payload)?;
        if revision.metadata().id() != active_revision {
            return Err(RouteConfigRedisStoreError::invalid_state(
                "Gateway console active revision key does not match the immutable revision payload",
            ));
        }
        if let Some(active_document) = active_document {
            let active_document: RouteConfigYaml =
                serde_json::from_str(active_document).map_err(|error| {
                    RouteConfigRedisStoreError::invalid_state(format!(
                        "Gateway console active document JSON is malformed: {error}"
                    ))
                })?;
            let validated = validate_route_document(active_document).map_err(|diagnostics| {
                RouteConfigRedisStoreError::invalid_state(format!(
                    "Gateway console active document is invalid: {diagnostics}"
                ))
            })?;
            if validated.document_digest() != revision.metadata().document_digest()
                || validated.yaml_digest() != revision.metadata().yaml_digest()
            {
                return Err(RouteConfigRedisStoreError::invalid_state(
                    "Gateway console active document does not match the immutable revision payload",
                ));
            }
        }
        Ok(revision)
    }

    async fn ensure_single_node(&self, pool: &Pool) -> Result<(), RouteConfigRedisStoreError> {
        let mut connection = pool.get().await.map_err(|error| {
            RouteConfigRedisStoreError::unavailable(format!(
                "Gateway console Redis capability check failed: {error}"
            ))
        })?;
        let info: String = redis::cmd("INFO")
            .arg("CLUSTER")
            .query_async(&mut connection)
            .await
            .map_err(|error| {
                RouteConfigRedisStoreError::unavailable(format!(
                    "Gateway console Redis cluster probe failed: {error}"
                ))
            })?;
        if cluster_enabled(&info) {
            return Err(RouteConfigRedisStoreError::unsupported_cluster(
                "Gateway console route transactions currently require a single-node Redis deployment",
            ));
        }
        Ok(())
    }
}

fn parse_activation_response(
    response: &[String],
) -> Result<RouteConfigRedisActivationOutcome, RouteConfigRedisStoreError> {
    match response.first().map(String::as_str) {
        Some("activated") => Ok(RouteConfigRedisActivationOutcome::Activated),
        Some("already_active") => Ok(RouteConfigRedisActivationOutcome::AlreadyActive),
        Some("revision_conflict") => Ok(RouteConfigRedisActivationOutcome::RevisionConflict {
            actual: response.get(1).cloned().filter(|value| !value.is_empty()),
        }),
        Some("prepared_transaction_mismatch") => {
            Ok(RouteConfigRedisActivationOutcome::PreparedTransactionMismatch)
        }
        Some("revision_payload_mismatch") => {
            Ok(RouteConfigRedisActivationOutcome::RevisionPayloadMismatch)
        }
        Some(other) => Err(RouteConfigRedisStoreError::invalid_state(format!(
            "Gateway console Redis CAS returned an unknown outcome: {other}"
        ))),
        None => Err(RouteConfigRedisStoreError::invalid_state(
            "Gateway console Redis CAS returned an empty response",
        )),
    }
}

async fn store_json_if_absent_or_identical(
    connection: &mut deadpool_redis::Connection,
    key: &str,
    value: &str,
) -> Result<(), RouteConfigRedisStoreError> {
    let existing: Option<String> = connection.get(key).await.map_err(|error| {
        RouteConfigRedisStoreError::unavailable(format!(
            "Gateway console Redis read failed for '{key}': {error}"
        ))
    })?;
    if let Some(existing) = existing {
        if existing == value {
            return Ok(());
        }
        return Err(RouteConfigRedisStoreError::invalid_state(format!(
            "Gateway console Redis key '{key}' already contains a different immutable value",
        )));
    }
    let result: Option<String> = redis::cmd("SET")
        .arg(key)
        .arg(value)
        .arg("NX")
        .query_async(connection)
        .await
        .map_err(|error| {
            RouteConfigRedisStoreError::unavailable(format!(
                "Gateway console Redis write failed for '{key}': {error}"
            ))
        })?;
    if result.as_deref() != Some("OK") {
        let existing: Option<String> = connection.get(key).await.map_err(|error| {
            RouteConfigRedisStoreError::unavailable(format!(
                "Gateway console Redis re-read failed for '{key}': {error}"
            ))
        })?;
        if existing.as_deref() == Some(value) {
            return Ok(());
        }
        return Err(RouteConfigRedisStoreError::invalid_state(format!(
            "Gateway console Redis key '{key}' changed before the immutable payload could be stored",
        )));
    }
    Ok(())
}

fn serialize_transaction_record(
    record: &TransactionRecord,
) -> Result<String, RouteConfigRedisStoreError> {
    serde_json::to_string(record).map_err(|error| {
        RouteConfigRedisStoreError::invalid_state(format!(
            "Gateway console transaction JSON serialization failed: {error}"
        ))
    })
}

fn cluster_enabled(info: &str) -> bool {
    info.lines()
        .map(str::trim)
        .any(|line| line == "cluster_enabled:1")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::console::document::validate_route_document;
    use crate::console::revision::{RevisionActor, RevisionMetadata};
    use time::OffsetDateTime;

    fn document(provider: &str, model: &str) -> RouteConfigYaml {
        serde_yaml::from_str(&format!(
            r#"
providers:
  - id: {provider}
    base_url: https://example.com/v1
    api_key: test-secret
    supported_models: [{model}]
model_routes:
  - pattern: {model}
    provider_ids: [{provider}]
aliases:
  answer: {model}
"#
        ))
        .unwrap()
    }

    fn revision(document: &RouteConfigYaml) -> RouteConfigRedisRevision {
        let validated = validate_route_document(document.clone()).unwrap();
        let metadata = RevisionMetadata::from_validated(
            4,
            Some("r3-aaaaaaaaaaaa".to_string()),
            RevisionActor::ManagementToken,
            OffsetDateTime::from_unix_timestamp(1_700_000_000).unwrap(),
            Some("test".to_string()),
            &validated,
        );
        RouteConfigRedisRevision::new(metadata, document.clone()).unwrap()
    }

    #[test]
    fn redis_revision_round_trips_through_json() {
        let revision = revision(&document("managed", "gpt-5.4"));
        let roundtrip = RouteConfigRedisRevision::from_json(&revision.to_json().unwrap()).unwrap();

        assert_eq!(roundtrip.metadata().id(), revision.metadata().id());
        assert_eq!(
            roundtrip.metadata().document_digest(),
            revision.metadata().document_digest()
        );
    }

    #[test]
    fn redis_revision_rejects_digest_mismatch() {
        let route_document = document("managed", "gpt-5.4");
        let validated = validate_route_document(document("other", "o3")).unwrap();
        let metadata = RevisionMetadata::from_validated(
            5,
            Some("r4-aaaaaaaaaaaa".to_string()),
            RevisionActor::ManagementToken,
            OffsetDateTime::from_unix_timestamp(1_700_000_010).unwrap(),
            None,
            &validated,
        );

        let error = RouteConfigRedisRevision::new(metadata, route_document).unwrap_err();

        assert_eq!(error.code(), "console_redis_invalid_state");
    }

    #[test]
    fn active_revision_decode_rejects_mismatched_active_document() {
        let revision = revision(&document("managed", "gpt-5.4"));
        let error = RouteConfigRedisStore::decode_active_revision_state(
            revision.metadata().id(),
            Some(&revision.to_json().unwrap()),
            Some(&serde_json::to_string(&document("other-provider", "other-model")).unwrap()),
        )
        .unwrap_err();

        assert_eq!(error.code(), "console_redis_invalid_state");
    }

    #[test]
    fn cluster_probe_detects_enabled_cluster_mode() {
        assert!(cluster_enabled("# Cluster\r\ncluster_enabled:1\r\n"));
        assert!(!cluster_enabled("# Cluster\r\ncluster_enabled:0\r\n"));
    }

    #[test]
    fn activation_response_parser_maps_conflicts_and_success() {
        assert_eq!(
            parse_activation_response(&["activated".to_string()]).unwrap(),
            RouteConfigRedisActivationOutcome::Activated
        );
        assert_eq!(
            parse_activation_response(&[
                "revision_conflict".to_string(),
                "r2-abcdefabcdef".to_string()
            ])
            .unwrap(),
            RouteConfigRedisActivationOutcome::RevisionConflict {
                actual: Some("r2-abcdefabcdef".to_string())
            }
        );
    }
}
