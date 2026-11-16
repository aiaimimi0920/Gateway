//! Prepared registration receipts precede remote effects; accepted attempts are never replayed.
use super::{session::wait_stopped, Binding};
use crate::console::persistence::{LumaRegistrationRecord, RouteConfigPersistence};
use crate::protocol::luma_oauth::{RegisteredClient, RegistrationRequest};
use parking_lot::Mutex;
use serde::Serialize;
use std::{sync::Arc, time::Duration};
use tokio::{net::TcpListener, sync::watch, time::Instant};

#[derive(Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RegistrationStatus {
    Prepared,
    Registering,
    Registered,
    Unconfirmed,
    Cancelled,
    Expired,
}

struct Inner {
    status: RegistrationStatus,
    listener: Option<TcpListener>,
    client: Option<RegisteredClient>,
}
pub struct Registration {
    pub(super) id: String,
    pub(super) binding: Binding,
    deadline: Instant,
    inner: Mutex<Inner>,
}
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RegistrationView {
    pub id: String,
    pub provider_id: String,
    pub expected_revision: String,
    pub redirect_uri: String,
    pub status: RegistrationStatus,
    pub client_id: Option<String>,
    pub offline_access: bool,
}
impl Registration {
    pub(super) fn reconcile(
        &self,
        record: &LumaRegistrationRecord,
    ) -> Result<(), super::LoginError> {
        if self.id != record.id
            || self.binding.owner != record.owner
            || self.binding.provider != record.provider_id
            || self.binding.revision != record.expected_revision
            || self.binding.redirect != record.redirect_uri
            || self.binding.offline_access != record.offline_access
        {
            return Err(super::LoginError::Unavailable);
        }
        let mut inner = self.inner.lock();
        // A peer runtime may finish after this runtime restored an unknown receipt.
        // Never replace an active attempt or downgrade an already published success.
        if inner.status == RegistrationStatus::Unconfirmed {
            if let Some(client_id) = &record.client_id {
                inner.client = Some(RegisteredClient {
                    client_id: client_id.clone(),
                    redirect_uri: record.redirect_uri.clone(),
                });
                inner.status = RegistrationStatus::Registered;
            }
        }
        Ok(())
    }

    pub(super) fn restore(record: LumaRegistrationRecord) -> Arc<Self> {
        let mut binding = Binding::new(
            &record.owner,
            &record.provider_id,
            &record.expected_revision,
            record
                .client_id
                .as_deref()
                .unwrap_or("registration-pending"),
            &record.redirect_uri,
        )
        .expect("durable receipt validated by storage reader");
        binding.offline_access = record.offline_access;
        let client = record.client_id.map(|client_id| RegisteredClient {
            client_id,
            redirect_uri: record.redirect_uri,
        });
        Arc::new(Self {
            id: record.id,
            binding,
            deadline: Instant::now(),
            inner: Mutex::new(Inner {
                status: if client.is_some() {
                    RegistrationStatus::Registered
                } else {
                    RegistrationStatus::Unconfirmed
                },
                listener: None,
                client,
            }),
        })
    }

    pub(super) fn new(binding: Binding, listener: TcpListener) -> Arc<Self> {
        Arc::new(Self {
            id: uuid::Uuid::new_v4().to_string(),
            binding,
            deadline: Instant::now() + Duration::from_secs(1200),
            inner: Mutex::new(Inner {
                status: RegistrationStatus::Prepared,
                listener: Some(listener),
                client: None,
            }),
        })
    }
    pub fn view(&self) -> RegistrationView {
        let mut inner = self.inner.lock();
        if inner.status == RegistrationStatus::Prepared && Instant::now() >= self.deadline {
            inner.status = RegistrationStatus::Expired;
            inner.listener = None;
        }
        RegistrationView {
            id: self.id.clone(),
            provider_id: self.binding.provider.clone(),
            expected_revision: self.binding.revision.clone(),
            redirect_uri: self.binding.redirect.clone(),
            status: inner.status,
            client_id: inner.client.as_ref().map(|client| client.client_id.clone()),
            offline_access: self.binding.offline_access,
        }
    }
    pub(super) fn begin(self: &Arc<Self>) -> Option<Attempt> {
        self.view();
        let mut inner = self.inner.lock();
        if inner.status != RegistrationStatus::Prepared {
            return None;
        }
        let listener = inner
            .listener
            .take()
            .expect("prepared registration owns callback port");
        inner.status = RegistrationStatus::Registering;
        Some(Attempt {
            receipt: self.clone(),
            listener: Some(listener),
        })
    }
    pub(super) fn cancel_prepared(&self) {
        let mut inner = self.inner.lock();
        if inner.status == RegistrationStatus::Prepared {
            inner.listener = None;
            inner.status = RegistrationStatus::Cancelled;
        }
    }
}

// Owned by the registry JoinSet, not the HTTP response. Drop records uncertainty, not failure/retry.
pub(super) struct Attempt {
    receipt: Arc<Registration>,
    listener: Option<TcpListener>,
}
impl Attempt {
    pub async fn run(
        mut self,
        http: rquest::Client,
        mut stop: watch::Receiver<bool>,
        store: RouteConfigPersistence,
    ) {
        let record = LumaRegistrationRecord {
            id: self.receipt.id.clone(),
            owner: self.receipt.binding.owner.clone(),
            provider_id: self.receipt.binding.provider.clone(),
            expected_revision: self.receipt.binding.revision.clone(),
            redirect_uri: self.receipt.binding.redirect.clone(),
            client_id: None,
            offline_access: self.receipt.binding.offline_access,
        };
        if *stop.borrow() {
            return;
        }
        match super::registration_storage::claim(store.clone(), record.clone()).await {
            Ok(true) => {}
            Ok(false) => return, // Another accepted receipt is authoritative; never replay it.
            Err(error) => {
                tracing::warn!(
                    code = error.code(),
                    "Luma registration not sent: durable claim unavailable"
                );
                return;
            }
        }
        let mut request = RegistrationRequest::new(&self.receipt.binding.redirect)
            .expect("registration redirect validated at preparation");
        if self.receipt.binding.offline_access {
            request = request.with_offline_access();
        }
        let result = tokio::select! { biased;
            _ = wait_stopped(&mut stop) => return,
            result = request.register(&http) => result,
        };
        if let Ok(client) = result {
            self.listener = None;
            if let Err(error) =
                super::registration_storage::finish(store, record, client.client_id.clone()).await
            {
                tracing::warn!(
                    code = error.code(),
                    "Luma registration result persistence unconfirmed"
                );
                return;
            }
            let mut inner = self.receipt.inner.lock();
            inner.client = Some(client);
            inner.status = RegistrationStatus::Registered;
        }
        // Every error is conservatively unconfirmed: even an error response may follow a remote write.
    }
}
impl Drop for Attempt {
    fn drop(&mut self) {
        self.listener = None;
        let mut inner = self.receipt.inner.lock();
        if inner.status == RegistrationStatus::Registering {
            inner.status = RegistrationStatus::Unconfirmed;
        }
    }
}
