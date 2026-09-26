//! The complete commit future retains one writer guard and the original Redis/YAML await order.

use super::{RouteConfigCoordinator, RouteConfigRuntimeError};
use crate::console::document::{canonicalize_route_document, validate_route_document};
use crate::console::persistence::{TransactionPhase, TransactionRecord};
use crate::console::redis_store::{RouteConfigRedisActivationOutcome, RouteConfigRedisRevision};
use crate::console::revision::{RevisionActor, RevisionMetadata};
use crate::routing::config::{RouteConfigSnapshot, RouteConfigYaml};
use std::sync::Arc;
use time::OffsetDateTime;
use uuid::Uuid;

impl RouteConfigCoordinator {
    pub async fn commit_document(
        &self,
        expected_revision: &str,
        document: RouteConfigYaml,
        message: Option<String>,
    ) -> Result<Arc<RouteConfigSnapshot>, RouteConfigRuntimeError> {
        self.commit_document_as(
            expected_revision,
            document,
            message,
            RevisionActor::ManagementToken,
        )
        .await
    }

    pub(super) async fn commit_document_as(
        &self,
        expected_revision: &str,
        document: RouteConfigYaml,
        message: Option<String>,
        actor: RevisionActor,
    ) -> Result<Arc<RouteConfigSnapshot>, RouteConfigRuntimeError> {
        let validated = validate_route_document(document).map_err(|diagnostics| {
            RouteConfigRuntimeError::new("console_route_validation_failed", diagnostics.to_string())
        })?;
        let current = self.route_config.snapshot();
        if current.revision().id() != expected_revision {
            return Err(RouteConfigRuntimeError::revision_conflict(format!(
                "Gateway console expected revision '{}' but active revision is '{}'",
                expected_revision,
                current.revision().id()
            )));
        }
        let proposed_revision = RevisionMetadata::from_validated(
            current.revision().sequence().saturating_add(1),
            Some(current.revision().id().to_string()),
            actor,
            OffsetDateTime::now_utc(),
            message,
            &validated,
        );
        let canonical = canonicalize_route_document(validated.document()).map_err(|error| {
            RouteConfigRuntimeError::new("console_route_compilation_failed", error.to_string())
        })?;

        let guard = self
            .persistence
            .try_writer_lock()
            .map_err(RouteConfigRuntimeError::from_persistence)?;
        let locked_current = self.route_config.snapshot();
        if locked_current.revision().id() != expected_revision {
            return Err(RouteConfigRuntimeError::revision_conflict(format!(
                "Gateway console expected revision '{}' but active revision is '{}'",
                expected_revision,
                locked_current.revision().id()
            )));
        }

        // A YAML-only development runtime has a valid local revision before
        // Redis has been initialized.  Treat an absent Redis active key as a
        // bootstrap state and let the Redis CAS script atomically claim it;
        // if Redis already has an active revision, enforce the normal
        // optimistic-concurrency check before touching the local journal/YAML.
        let expected_redis_revision = match self
            .redis
            .load_active_revision()
            .await
            .map_err(RouteConfigRuntimeError::from_redis)?
        {
            Some(active) if active.metadata().id() == expected_revision => Some(expected_revision),
            Some(active) => {
                return Err(RouteConfigRuntimeError::revision_conflict(format!(
                    "Gateway console Redis active revision conflict: expected '{}', actual '{}'",
                    expected_revision,
                    active.metadata().id()
                )));
            }
            None => None,
        };

        // Revision IDs intentionally derive from sequence + document digest.
        // A failed transaction can therefore leave an immutable archive (and
        // Redis revision payload) that a safe retry must reuse instead of
        // recreating with a different timestamp under the same ID. The audit
        // message remains part of the retry intent and must match exactly.
        let revision = match self
            .persistence
            .load_revision(proposed_revision.id())
            .map_err(RouteConfigRuntimeError::from_persistence)?
        {
            Some(stored)
                if stored.metadata().sequence() == proposed_revision.sequence()
                    && stored.metadata().parent() == proposed_revision.parent()
                    && stored.metadata().actor() == proposed_revision.actor()
                    && stored.metadata().message() == proposed_revision.message()
                    && stored.metadata().document_digest()
                        == proposed_revision.document_digest()
                    && stored.metadata().yaml_digest() == proposed_revision.yaml_digest() =>
            {
                stored.metadata().clone()
            }
            Some(_) => {
                return Err(RouteConfigRuntimeError::new(
                    "console_revision_collision",
                    format!(
                        "Existing immutable revision '{}' does not match the retry candidate",
                        proposed_revision.id()
                    ),
                ));
            }
            None => proposed_revision,
        };
        let redis_revision =
            RouteConfigRedisRevision::new(revision.clone(), validated.document().clone())
                .map_err(RouteConfigRuntimeError::from_redis)?;

        self.persistence
            .archive_revision_locked(&guard, &revision, &canonical)
            .map_err(RouteConfigRuntimeError::from_persistence)?;
        let (previous_yaml_present, previous_yaml_digest) = self
            .persistence
            .current_routes_state_locked(&guard)
            .map_err(RouteConfigRuntimeError::from_persistence)?;
        let tx_id = format!("tx-{}", Uuid::new_v4());
        let tx_path = format!("revisions/{}", revision.id());
        let prepared = TransactionRecord::new_prepared(
            tx_id,
            Some(expected_revision),
            revision.id(),
            tx_path,
            previous_yaml_present,
            previous_yaml_digest.as_deref(),
            validated.yaml_digest(),
            OffsetDateTime::now_utc(),
        )
        .map_err(RouteConfigRuntimeError::from_persistence)?;
        self.journal
            .persist_locked(&guard, &prepared)
            .map_err(RouteConfigRuntimeError::from_persistence)?;

        if let Err(error) = self.redis.store_revision(&redis_revision).await {
            self.abort_without_yaml_change(
                &guard,
                prepared.tx_id(),
                "redis_revision_store_failed",
            )?;
            return Err(RouteConfigRuntimeError::from_redis(error));
        }
        if let Err(error) = self.redis.store_prepared_transaction(&prepared).await {
            self.abort_without_yaml_change(&guard, prepared.tx_id(), "redis_prepare_store_failed")?;
            return Err(RouteConfigRuntimeError::from_redis(error));
        }

        let receipt = self
            .persistence
            .replace_routes_yaml_for_transaction_locked(
                &guard,
                &prepared,
                validated.canonical_yaml(),
            )
            .map_err(RouteConfigRuntimeError::from_persistence)?;
        let yaml_replaced = self
            .journal
            .record_yaml_replaced_locked(
                &guard,
                prepared.tx_id(),
                &receipt,
                OffsetDateTime::now_utc(),
            )
            .map_err(RouteConfigRuntimeError::from_persistence)?;
        let mut activated = yaml_replaced.clone();
        activated
            .transition(
                TransactionPhase::RedisActivated,
                OffsetDateTime::now_utc(),
                None,
            )
            .map_err(RouteConfigRuntimeError::from_persistence)?;

        match self
            .redis
            .activate_revision(
                expected_redis_revision,
                &redis_revision,
                &prepared,
                &activated,
            )
            .await
            .map_err(RouteConfigRuntimeError::from_redis)?
        {
            RouteConfigRedisActivationOutcome::Activated
            | RouteConfigRedisActivationOutcome::AlreadyActive => {
                self.finish_commit(&guard, prepared.tx_id(), validated, revision)
            }
            RouteConfigRedisActivationOutcome::RevisionConflict { actual } => {
                self.rollback_yaml_and_abort(
                    &guard,
                    prepared.tx_id(),
                    &receipt,
                    "redis_revision_conflict",
                )?;
                Err(RouteConfigRuntimeError::revision_conflict(format!(
                    "Gateway console Redis active revision conflict: expected '{}', actual '{}'",
                    expected_revision,
                    actual.unwrap_or_else(|| "<none>".to_string())
                )))
            }
            RouteConfigRedisActivationOutcome::PreparedTransactionMismatch => {
                self.rollback_yaml_and_abort(
                    &guard,
                    prepared.tx_id(),
                    &receipt,
                    "redis_prepared_transaction_mismatch",
                )?;
                Err(RouteConfigRuntimeError::new(
                    "console_redis_invalid_state",
                    "Gateway console Redis prepared transaction did not match the local transaction record",
                ))
            }
            RouteConfigRedisActivationOutcome::RevisionPayloadMismatch => {
                self.rollback_yaml_and_abort(
                    &guard,
                    prepared.tx_id(),
                    &receipt,
                    "redis_revision_payload_mismatch",
                )?;
                Err(RouteConfigRuntimeError::new(
                    "console_redis_invalid_state",
                    "Gateway console Redis immutable revision payload did not match the candidate revision",
                ))
            }
            RouteConfigRedisActivationOutcome::Unavailable => {
                self.rollback_yaml_and_abort(
                    &guard,
                    prepared.tx_id(),
                    &receipt,
                    "redis_unavailable",
                )?;
                Err(RouteConfigRuntimeError::new(
                    "console_redis_unavailable",
                    "Gateway console Redis CAS could not start because Redis was unavailable",
                ))
            }
            RouteConfigRedisActivationOutcome::Indeterminate => {
                self.resolve_indeterminate_activation(
                    &guard,
                    prepared.tx_id(),
                    &receipt,
                    expected_revision,
                    &revision,
                    validated,
                )
                .await
            }
        }
    }
}
