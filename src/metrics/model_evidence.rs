//! Management-only, bounded observations. Self-reported names are never identity proof.
use serde::Serialize;
use std::collections::VecDeque;
use std::sync::Mutex;

use super::request::GatewayMetrics;
use crate::protocol::model_declaration::{
    bounded_model_label, ModelDeclaration, ModelDeclarationSource,
};

const MAX_OBSERVATIONS: usize = 100;

#[derive(Debug, Default)]
pub(super) struct ModelObservationStore(Mutex<VecDeque<ModelObservation>>);

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelObservation {
    observed_at: i64,
    provider_id: Option<String>,
    requested_model: Option<String>,
    upstream_model: Option<String>,
    declared_model: Option<String>,
    declaration_source: Option<ModelDeclarationSource>,
    alias_applied: bool,
    relation: &'static str,
    verified: bool,
    stream: bool,
    observation_status: &'static str,
    observation_end: &'static str,
}

#[derive(Debug)]
pub(crate) struct ModelObservationMeta {
    provider_id: Option<String>,
    requested_model: Option<String>,
    upstream_model: Option<String>,
    stream: bool,
}

impl ModelObservationMeta {
    pub(crate) fn new(
        provider: &str,
        requested: Option<&str>,
        upstream: &str,
        stream: bool,
    ) -> Self {
        Self {
            provider_id: bounded_model_label(provider),
            requested_model: requested.and_then(bounded_model_label),
            upstream_model: bounded_model_label(upstream),
            stream,
        }
    }

    pub(crate) fn finish(
        self,
        declaration: Option<&ModelDeclaration>,
        status: &'static str,
        outcome: &'static str,
    ) -> ModelObservation {
        let declared_model = declaration.and_then(|d| bounded_model_label(&d.value));
        let relation = match (declared_model.as_deref(), self.upstream_model.as_deref()) {
            (Some(a), Some(b)) if a == b => "declared_matches",
            (Some(_), Some(_)) => "declared_differs",
            _ => "unknown",
        };
        ModelObservation {
            observed_at: time::OffsetDateTime::now_utc().unix_timestamp(),
            alias_applied: matches!((&self.requested_model, &self.upstream_model), (Some(a), Some(b)) if a != b),
            provider_id: self.provider_id,
            requested_model: self.requested_model,
            upstream_model: self.upstream_model,
            declaration_source: declared_model.as_ref().and(declaration.map(|d| d.source)),
            declared_model,
            relation,
            verified: false,
            stream: self.stream,
            observation_status: status,
            observation_end: outcome,
        }
    }
}

impl GatewayMetrics {
    pub(crate) fn observe_model_declaration(&self, observation: ModelObservation) {
        let mut rows = self
            .model_observations
            .0
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        if rows.len() == MAX_OBSERVATIONS {
            rows.pop_front();
        }
        rows.push_back(observation);
    }

    /// Not part of public Prometheus labels or billing/health/routing decisions.
    pub fn model_observations(&self) -> Vec<ModelObservation> {
        self.model_observations
            .0
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .iter()
            .cloned()
            .collect()
    }
}

#[cfg(test)]
mod tests;
