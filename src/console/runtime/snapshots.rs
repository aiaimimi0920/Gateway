//! Recovered snapshots and YAML mirrors are installed only after revision identity checks.

use super::revision_state::{
    archive_path, current_matches_record_new, current_matches_record_previous,
    validated_and_metadata_from_active,
};
use super::{RouteConfigCoordinator, RouteConfigRuntimeError};
use crate::console::document::validate_route_document;
use crate::console::persistence::TransactionRecord;
use crate::console::redis_store::RouteConfigRedisRevision;
use crate::console::revision::RevisionMetadata;
use crate::routing::config::{ActiveConfigSource, RouteConfigYaml};
use std::fs;
use time::OffsetDateTime;

impl RouteConfigCoordinator {
    pub(super) fn ensure_store_matches_revision(
        &self,
        active_revision: &RouteConfigRedisRevision,
    ) -> Result<(), RouteConfigRuntimeError> {
        let (validated, metadata) = validated_and_metadata_from_active(
            active_revision,
            &TransactionRecord::new_prepared(
                "tx-active-probe",
                active_revision.metadata().parent(),
                active_revision.metadata().id(),
                format!("revisions/{}", active_revision.metadata().id()),
                true,
                Some(active_revision.metadata().yaml_digest()),
                active_revision.metadata().yaml_digest(),
                OffsetDateTime::now_utc(),
            )
            .map_err(RouteConfigRuntimeError::from_persistence)?,
        )?;
        self.install_snapshot_if_needed(metadata, validated)
    }

    pub(super) fn ensure_yaml_matches_revision(
        &self,
        guard: &crate::console::WriterLockGuard,
        record: &TransactionRecord,
        canonical_yaml: &[u8],
    ) -> Result<(), RouteConfigRuntimeError> {
        let current = self
            .persistence
            .current_routes_state_locked(guard)
            .map_err(RouteConfigRuntimeError::from_persistence)?;
        if current_matches_record_new(&current, record) {
            return Ok(());
        }
        if !current_matches_record_previous(&current, record) {
            return Err(RouteConfigRuntimeError::new(
                "console_recovery_required",
                format!(
                    "Gateway console YAML mirror for transaction '{}' is neither the previous nor the authoritative new revision",
                    record.tx_id()
                ),
            ));
        }
        self.persistence
            .install_routes_yaml_recovered_locked(guard, canonical_yaml)
            .map_err(RouteConfigRuntimeError::from_persistence)
    }

    pub(super) fn install_snapshot_if_needed(
        &self,
        metadata: RevisionMetadata,
        validated: crate::console::document::ValidatedRouteDocument,
    ) -> Result<(), RouteConfigRuntimeError> {
        if self.route_config.snapshot().revision().id() == metadata.id() {
            return Ok(());
        }
        self.route_config
            .install_external_validated(validated, metadata, ActiveConfigSource::Recovered)
            .map(|_| ())
            .map_err(RouteConfigRuntimeError::from_replace)
    }

    pub(super) fn validated_and_metadata_from_archive(
        &self,
        record: &TransactionRecord,
    ) -> Result<
        (
            crate::console::document::ValidatedRouteDocument,
            RevisionMetadata,
        ),
        RouteConfigRuntimeError,
    > {
        let archive = archive_path(&self.persistence, record);
        let metadata_bytes = fs::read(archive.join("metadata.json")).map_err(|error| {
            RouteConfigRuntimeError::new("console_recovery_required", error.to_string())
        })?;
        let document_bytes = fs::read(archive.join("document.json")).map_err(|error| {
            RouteConfigRuntimeError::new("console_recovery_required", error.to_string())
        })?;
        let metadata: RevisionMetadata =
            serde_json::from_slice(&metadata_bytes).map_err(|error| {
                RouteConfigRuntimeError::new("console_recovery_required", error.to_string())
            })?;
        let document: RouteConfigYaml =
            serde_json::from_slice(&document_bytes).map_err(|error| {
                RouteConfigRuntimeError::new("console_recovery_required", error.to_string())
            })?;
        let validated = validate_route_document(document).map_err(|diagnostics| {
            RouteConfigRuntimeError::new("console_route_validation_failed", diagnostics.to_string())
        })?;
        if metadata.id() != record.new_revision()
            || metadata.document_digest() != validated.document_digest()
            || metadata.yaml_digest() != validated.yaml_digest()
        {
            return Err(RouteConfigRuntimeError::new(
                "console_recovery_required",
                "Gateway console archive metadata no longer matches its validated document",
            ));
        }
        Ok((validated, metadata))
    }
}
