//! Only non-secret lifecycle facts are returned. Cancelling never retracts an accepted commit.
use super::{Binding, Consent};
use parking_lot::Mutex;
use serde::Serialize;
use std::{sync::Arc, time::Duration};
use tokio::{sync::watch, time::Instant};
#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct View {
    pub id: String,
    pub provider_id: String,
    pub expected_revision: String,
    pub target_credential_id: String,
    pub status: &'static str,
    pub authorization_url: Option<String>,
    pub auth_received: bool,
    pub account_matched: bool,
    pub credential_persisted: bool,
    pub expires_at: Option<String>,
    pub saved_revision: Option<String>,
    pub error_code: Option<&'static str>,
    pub inference_status: &'static str,
}
pub(crate) struct Session {
    pub(super) binding: Binding,
    pub(super) deadline: Instant,
    pub(super) stop: watch::Sender<bool>,
    pub(super) renew: watch::Sender<Option<Arc<Consent>>>,
    pub(super) inner: Mutex<View>,
}
impl Session {
    pub(super) fn new(id: String, binding: Binding, url: Option<String>) -> Arc<Self> {
        let target = binding
            .credential
            .clone()
            .unwrap_or_else(|| format!("cursor-{id}"));
        Arc::new(Self {
            deadline: Instant::now() + Duration::from_secs(600),
            stop: watch::channel(false).0,
            renew: watch::channel(None).0,
            inner: Mutex::new(View {
                id,
                provider_id: binding.provider.clone(),
                expected_revision: binding.revision.clone(),
                target_credential_id: target,
                status: "waiting_user",
                authorization_url: url,
                auth_received: false,
                account_matched: false,
                credential_persisted: false,
                expires_at: None,
                saved_revision: None,
                error_code: None,
                inference_status: "not_checked",
            }),
            binding,
        })
    }
    pub fn view(&self) -> View {
        self.inner.lock().clone()
    }
    pub fn cancel(&self) {
        let mut view = self.inner.lock();
        if matches!(
            view.status,
            "waiting_user" | "verifying" | "awaiting_permission"
        ) {
            self.stop.send_replace(true);
            view.status = "cancelled";
            view.authorization_url = None;
        }
    }
    pub(super) fn fail(&self, code: &'static str) {
        let mut view = self.inner.lock();
        if view.status == "succeeded" || view.status == "cancelled" {
            return;
        }
        view.status = if view.status == "importing" {
            "save_unconfirmed"
        } else if code == "cursor_login_expired" {
            "expired"
        } else {
            "failed"
        };
        view.error_code = Some(code);
        view.authorization_url = None;
    }
}
