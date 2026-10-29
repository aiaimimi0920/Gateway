//! Constant-space attribution at the actual send-start seam, separate from winner state.
use std::sync::Arc;

use parking_lot::Mutex;

use super::PipelineContext;
use crate::routing::candidate::RouteCandidate;

#[derive(Clone)]
pub(super) struct AttemptAttribution {
    pub provider_id: String,
    model: String,
    credential_ref: Option<String>,
}

pub(super) type LastAttempt = Arc<Mutex<Option<AttemptAttribution>>>;

#[derive(Clone, Default)]
pub(super) struct AttemptAudit {
    latest: LastAttempt,
    candidate: Option<AttemptAttribution>,
}

impl AttemptAudit {
    pub fn new(latest: LastAttempt) -> Self {
        Self {
            latest,
            candidate: None,
        }
    }

    pub fn set_candidate(&mut self, candidate: &RouteCandidate, model: &str) {
        self.candidate = Some(AttemptAttribution {
            provider_id: candidate.provider_account_id.clone(),
            model: model.to_string(),
            credential_ref: candidate.payload.credential_id.clone(),
        });
    }

    // Call only after admission and request-budget begin succeed. No winner or secrets are stored.
    pub fn record(&self) {
        if let Some(candidate) = &self.candidate {
            *self.latest.lock() = Some(candidate.clone());
        }
    }
}

impl PipelineContext {
    pub fn audited_resolved_model(&self) -> Option<String> {
        self.resolved_model.clone().or_else(|| {
            self.last_provider_attempt
                .lock()
                .as_ref()
                .map(|attempt| attempt.model.clone())
        })
    }

    pub fn audited_real_credential_ref(&self) -> Option<String> {
        if self.selected_provider_id.is_some() {
            return self.selected_real_credential_ref.clone();
        }
        self.last_provider_attempt
            .lock()
            .as_ref()
            .and_then(|attempt| attempt.credential_ref.clone())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn latest_actual_attempt_is_not_a_winner_and_revisiting_a_provider_updates_attribution() {
        let request = crate::protocol::search_api::normalize_search(serde_json::json!({
            "model":"search", "query":"synthetic"
        }))
        .unwrap();
        let mut ctx = PipelineContext::new(request, None);
        let mut audit = AttemptAudit::new(ctx.last_provider_attempt.clone());
        for (provider, credential) in [("a", "a1"), ("b", "b1"), ("a", "a2")] {
            audit.candidate = Some(AttemptAttribution {
                provider_id: provider.into(),
                model: format!("{provider}-search"),
                credential_ref: Some(credential.into()),
            });
            assert_ne!(
                ctx.audited_real_credential_ref().as_deref(),
                Some(credential)
            );
            audit.record();
            assert_eq!(ctx.audited_provider_id().as_deref(), Some(provider));
            assert_eq!(
                ctx.audited_resolved_model(),
                Some(format!("{provider}-search"))
            );
            assert_eq!(
                ctx.audited_real_credential_ref().as_deref(),
                Some(credential)
            );
            assert!(ctx.selected_provider_id.is_none());
        }
        ctx.selected_provider_id = Some("winner".into());
        ctx.resolved_model = Some("winner-search".into());
        assert_eq!(ctx.audited_provider_id().as_deref(), Some("winner"));
        assert_eq!(
            ctx.audited_resolved_model().as_deref(),
            Some("winner-search")
        );
        assert!(
            ctx.audited_real_credential_ref().is_none(),
            "never borrow another account for a provider-level winner"
        );
    }
}
