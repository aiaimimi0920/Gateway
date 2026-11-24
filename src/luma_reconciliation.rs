//! Durable, bounded supplier-credit reads. Original request outcome and local billing never change.
mod hosted;
#[cfg(all(test, feature = "line-lumalabs-agents-official-api"))]
mod postgres_tests;
#[cfg(all(test, feature = "line-lumalabs-agents-official-api"))]
mod recovery_tests;
#[cfg(all(test, feature = "line-lumalabs-agents-official-api"))]
mod server_recovery_tests;
mod store;
#[cfg(all(test, feature = "line-lumalabs-agents-official-api"))]
mod tests;
mod worker;
use crate::routing::candidate::{ProviderAccountPayload, RouteCandidate};
use crate::{error::GatewayError, state::AppState};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
pub(crate) use worker::spawn;

#[derive(Clone)]
pub(crate) struct Checkpoint {
    store: store::Store,
    audit_id: String,
    binding: Binding,
}

#[derive(Clone)]
enum Binding {
    Route(RouteBinding),
    Hosted(hosted::Binding),
}

#[derive(Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct RouteBinding {
    provider_id: String,
    credential_id: String,
    revision: String,
}

impl Checkpoint {
    pub(crate) fn for_candidate(
        state: &AppState,
        audit_id: &str,
        candidate: &RouteCandidate,
        payload: &ProviderAccountPayload,
    ) -> Option<Self> {
        let provider = &candidate.provider_account_id;
        let credential = payload.credential_id.as_deref()?;
        match candidate.provider_credential_id.as_deref() {
            None => Self::new(state, audit_id, provider, credential, payload),
            Some(id) if id == credential => Some(Self {
                store: store::Store::Server(state.pg_pool.clone()?),
                audit_id: audit_id.into(),
                binding: Binding::Hosted(hosted::Binding::capture(
                    state, audit_id, provider, id, payload,
                )?),
            }),
            Some(_) => None,
        }
    }

    pub(crate) fn new(
        state: &AppState,
        audit_id: &str,
        provider: &str,
        credential: &str,
        payload: &ProviderAccountPayload,
    ) -> Option<Self> {
        let snapshot = state.route_config.snapshot();
        let target = snapshot
            .credential_probe_targets_for_provider(provider)?
            .into_iter()
            .find(|t| t.credential_id == credential && t.enabled)?;
        if target.payload.api_key != payload.api_key
            || target.payload.base_url != payload.base_url
            || target.payload.canonical_adapter() != crate::protocol::luma_agents::ADAPTER
        {
            return None;
        }
        Some(Self {
            store: store::Store::from_state(state)?,
            audit_id: audit_id.into(),
            binding: Binding::Route(RouteBinding {
                provider_id: provider.into(),
                credential_id: credential.into(),
                revision: snapshot.revision().id().into(),
            }),
        })
    }

    pub(crate) async fn persist(&self, facts: Value) -> Result<(), GatewayError> {
        self.store
            .checkpoint(&self.audit_id, &facts, &self.journal())
            .await
    }

    pub(crate) fn journal(&self) -> Value {
        // Generation has a hard 600 s limit. After 900 s, even a Server row left running
        // by a crashed process can be read without declaring its request outcome successful.
        let timestamp = now();
        let (version, binding) = match &self.binding {
            Binding::Route(value) => (1, json!(value)),
            Binding::Hosted(value) => (2, json!(value)),
        };
        json!({"version":version,"binding":binding,"state":"pending",
            "attempts":0,"nextAt":timestamp+60,"activeUntil":timestamp+900})
    }
}

fn now() -> i64 {
    time::OffsetDateTime::now_utc().unix_timestamp()
}

/// Finalizers own the outcome; preserve only the durable reconciliation marker across trace replacement.
pub(crate) fn preserve(existing: Option<&Value>, replacement: &mut Option<Value>) {
    if let (Some(journal), Some(trace)) = (
        existing.and_then(|trace| trace.get("lumaReconciliation")),
        replacement.as_mut(),
    ) {
        trace["lumaReconciliation"] = journal.clone();
        // A late finalizer can race crash reconciliation; retain settled credits for the same task only.
        if journal["state"] == "settled" {
            if let Some(facts) = existing.and_then(|value| value.get("lumaAgentsGeneration")) {
                if facts["id"] == trace["lumaAgentsGeneration"]["id"]
                    && facts["usage"]["credits_charged"].is_number()
                {
                    trace["lumaAgentsGeneration"]["usage"]["credits_charged"] =
                        facts["usage"]["credits_charged"].clone();
                }
            }
        }
    }
}

fn storage_error(error: sqlx::Error) -> GatewayError {
    tracing::error!(error = %error, "Luma reconciliation storage failed");
    crate::protocol::luma_agents::failure(
        crate::error::ErrorKind::ServerError,
        500,
        "luma_reconciliation_storage",
        "Luma task checkpoint could not be persisted.",
    )
}
