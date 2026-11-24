//! Request-local, secret-free send facts shared with the cancellation audit owner.
use crate::db;
use std::sync::{
    atomic::{AtomicBool, AtomicU16, Ordering},
    Arc,
};

#[derive(Clone)]
pub(crate) struct ProbeEvidence {
    facts: Arc<Facts>,
    provider: String,
    credential: String,
    model: String,
}

#[derive(Default)]
struct Facts {
    sent: AtomicBool,
    status: AtomicU16,
    failure: parking_lot::Mutex<Option<serde_json::Value>>,
}

impl ProbeEvidence {
    pub(crate) fn new(provider: &str, credential: &str, model: &str) -> Self {
        Self {
            facts: Arc::new(Facts::default()),
            provider: provider.into(),
            credential: credential.into(),
            model: model.into(),
        }
    }

    pub(super) fn begin(&self) {
        assert!(
            !self.facts.sent.swap(true, Ordering::SeqCst),
            "SAP probe cannot replay"
        );
    }

    pub(super) fn receive(&self, status: u16) {
        self.facts.status.store(status, Ordering::SeqCst);
    }

    pub(crate) fn status(&self) -> Option<u16> {
        match self.facts.status.load(Ordering::SeqCst) {
            0 => None,
            status => Some(status),
        }
    }

    pub(crate) fn fail(&self, error: &crate::error::GatewayError) {
        // Only locally assigned codes and observed statuses are persisted. Never serialize
        // GatewayError itself: its message or future fields may contain upstream material.
        let code = match error.code.as_deref() {
            Some(code)
                if code.len() <= 80
                    && (code.starts_with("sap_aicore_") || code == "invalid_sap_aicore")
                    && code.bytes().all(|b| b.is_ascii_lowercase() || b == b'_') =>
            {
                code
            }
            _ => "sap_aicore_probe_failed",
        };
        let phase = if self.facts.sent.load(Ordering::SeqCst) {
            "inference"
        } else if code.starts_with("sap_aicore_token_") {
            "oauth"
        } else {
            "prepare"
        };
        let status = if code == "sap_aicore_token_rejected" {
            error.http_status
        } else {
            self.status()
        };
        *self.facts.failure.lock() = Some(serde_json::json!({
            "phase":phase,"code":code,"status":status,
            "retryAfterMs":error.upstream_retry_after_ms
        }));
    }

    pub(crate) fn snapshot(&self) -> serde_json::Value {
        serde_json::json!({
            "attempts":u32::from(self.facts.sent.load(Ordering::SeqCst)),
            "upstreamStatus":self.status(),"failure":self.facts.failure.lock().clone()
        })
    }

    pub(crate) fn project(&self, input: &mut db::FinalizeRequestAuditInput) {
        let sent = self.facts.sent.load(Ordering::SeqCst);
        input.provider_account_id = sent.then(|| self.provider.clone());
        input.resolved_model = sent.then(|| self.model.clone());
        input.route_attempt_count = u32::from(sent);
        input.upstream_status = self.status();
        let trace = input
            .route_trace
            .get_or_insert_with(|| serde_json::json!({}));
        trace["routeAttemptCount"] = u32::from(sent).into();
        trace["sapProbe"] = self.snapshot();
        trace["realCredentialRef"] = if sent {
            self.credential.clone().into()
        } else {
            serde_json::Value::Null
        };
    }
}
