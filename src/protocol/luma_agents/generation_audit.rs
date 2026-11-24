//! Request-owned supplier observations survive cancellation without extending upstream I/O.
use parking_lot::Mutex;
use serde_json::{json, Number, Value};
use std::{future::Future, sync::Arc};

#[derive(Clone, Default)]
pub(crate) struct GenerationAudit {
    facts: Arc<Mutex<Option<Observation>>>,
    checkpoint: Option<crate::luma_reconciliation::Checkpoint>,
}

#[derive(PartialEq)]
struct Observation {
    id: String,
    model: String,
    kind: &'static str,
    state: &'static str,
    credits: Option<Number>,
    media_verified: bool,
}

tokio::task_local! { static CURRENT: Option<GenerationAudit>; }

/// The send future stays in the caller task. Each attempt/probe installs its own scope;
/// the existing audit owner retains the handle when that future is dropped.
pub(crate) async fn within<T>(
    audit: Option<GenerationAudit>,
    future: impl Future<Output = T>,
) -> T {
    CURRENT.scope(audit, future).await
}

impl GenerationAudit {
    pub(crate) fn durable(checkpoint: Option<crate::luma_reconciliation::Checkpoint>) -> Self {
        Self {
            checkpoint,
            ..Default::default()
        }
    }
    pub(crate) fn record(&self, trace: &mut Value) -> Option<String> {
        let slot = self.facts.lock();
        let facts = slot.as_ref()?;
        trace["lumaAgentsGeneration"] = json!({"id":facts.id,"model":facts.model,
            "type":facts.kind,"state":facts.state,"usage":{"credits_charged":facts.credits},
            "mediaVerified":facts.media_verified});
        // Cancellation may interrupt the checkpoint await; the existing finalizer still owns these facts.
        if let Some(checkpoint) = &self.checkpoint {
            trace["lumaReconciliation"] = checkpoint.journal();
        }
        Some(facts.id.clone())
    }
}

/// Only the product sender publishes, after UUID/model/type/state/usage validation.
/// No raw upstream body, input, credentials or asset addresses enter this slot.
#[cfg(feature = "line-lumalabs-agents-official-api")]
pub(crate) async fn observe(
    id: &str,
    model: &str,
    kind: &'static str,
    state: &'static str,
    credits: Option<Number>,
    media_verified: bool,
) -> Result<(), crate::error::GatewayError> {
    let checkpoint = CURRENT
        .try_with(|current| {
            let audit = current.as_ref()?;
            let mut slot = audit.facts.lock();
            // A later missing usage must not erase a previously observed actual charge.
            let credits = credits.or_else(|| {
                slot.as_ref()
                    .filter(|previous| previous.id == id)
                    .and_then(|previous| previous.credits.clone())
            });
            let next = Observation {
                id: id.into(),
                model: model.into(),
                kind,
                state,
                credits,
                media_verified,
            };
            if slot.as_ref() == Some(&next) {
                return None;
            }
            *slot = Some(next);
            drop(slot);
            let checkpoint = audit.checkpoint.clone()?;
            let mut trace = json!({});
            audit.record(&mut trace);
            Some((checkpoint, trace["lumaAgentsGeneration"].take()))
        })
        .ok()
        .flatten();
    if let Some((checkpoint, facts)) = checkpoint {
        checkpoint.persist(facts).await?;
    }
    Ok(())
}

/// Settlement already checked the same UUID and terminal state. Update only its charge;
/// asset verification and the original task facts remain owned by the generation flow.
#[cfg(feature = "line-lumalabs-agents-official-api")]
pub(crate) fn observe_credits(id: &str, credits: Option<Number>) {
    let _ = CURRENT.try_with(|current| {
        let Some(audit) = current else { return };
        if let Some(facts) = audit.facts.lock().as_mut().filter(|facts| facts.id == id) {
            if credits.is_some() {
                facts.credits = credits;
            }
        }
    });
}
