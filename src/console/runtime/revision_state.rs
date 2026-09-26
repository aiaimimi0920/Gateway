//! Revision and YAML state projections preserve the coordinator's reconciliation decisions.

use super::RouteConfigRuntimeError;
use crate::console::document::validate_route_document;
use crate::console::persistence::{RouteConfigPersistence, TransactionRecord};
use crate::console::redis_store::RouteConfigRedisRevision;
use crate::console::revision::RevisionMetadata;
use std::path::PathBuf;

pub(super) fn receipt_from_record(
    record: &TransactionRecord,
) -> crate::console::YamlReplaceReceipt {
    crate::console::YamlReplaceReceipt::from_record_parts(
        record.previous_yaml_present(),
        record.previous_yaml_digest(),
        record.new_yaml_digest(),
        record.same_dir_backup_leaf(),
    )
}

pub(super) fn current_matches_record_previous(
    current: &(bool, Option<String>),
    record: &TransactionRecord,
) -> bool {
    current.0 == record.previous_yaml_present()
        && current.1.as_deref() == record.previous_yaml_digest()
}

pub(super) fn current_matches_record_new(
    current: &(bool, Option<String>),
    record: &TransactionRecord,
) -> bool {
    current.0 && current.1.as_deref() == Some(record.new_yaml_digest())
}

pub(super) fn redis_kept_previous_revision(
    active_revision: Option<&RouteConfigRedisRevision>,
    record: &TransactionRecord,
) -> bool {
    match active_revision {
        None => true,
        Some(active_revision) => {
            active_revision.metadata().id() == record.old_revision().unwrap_or_default()
        }
    }
}

pub(super) fn redis_revision_matches_record(
    active_revision: Option<&RouteConfigRedisRevision>,
    record: &TransactionRecord,
) -> Result<bool, RouteConfigRuntimeError> {
    let Some(active_revision) = active_revision else {
        return Ok(false);
    };
    if active_revision.metadata().id() != record.new_revision() {
        return Ok(false);
    }
    if active_revision.metadata().yaml_digest() != record.new_yaml_digest() {
        return Err(RouteConfigRuntimeError::new(
            "console_recovery_required",
            format!(
                "Gateway console Redis active revision '{}' does not match the local transaction digest",
                active_revision.metadata().id()
            ),
        ));
    }
    Ok(true)
}

pub(super) fn validated_and_metadata_from_active(
    active_revision: &RouteConfigRedisRevision,
    record: &TransactionRecord,
) -> Result<
    (
        crate::console::document::ValidatedRouteDocument,
        RevisionMetadata,
    ),
    RouteConfigRuntimeError,
> {
    if active_revision.metadata().id() != record.new_revision()
        || active_revision.metadata().yaml_digest() != record.new_yaml_digest()
    {
        return Err(RouteConfigRuntimeError::new(
            "console_recovery_required",
            "Gateway console Redis active revision does not match the local transaction record",
        ));
    }
    let validated =
        validate_route_document(active_revision.document().clone()).map_err(|diagnostics| {
            RouteConfigRuntimeError::new("console_route_validation_failed", diagnostics.to_string())
        })?;
    Ok((validated, active_revision.metadata().clone()))
}

pub(super) fn archive_path(
    persistence: &RouteConfigPersistence,
    record: &TransactionRecord,
) -> PathBuf {
    persistence.state_root().join(
        record
            .archive_relative_path()
            .replace('/', std::path::MAIN_SEPARATOR_STR),
    )
}
