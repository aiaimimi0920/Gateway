//! Bounded, owner-bound registration receipts share the existing runtime task/shutdown owner.
use super::{registration::Registration, registry::reap, Binding, LoginError, Registry};
use crate::{error::GatewayError, state::AppState};
use std::sync::Arc;

impl Registry {
    pub(crate) async fn prepare_registration(
        &self,
        state: &Arc<AppState>,
        owner: &str,
        provider: &str,
        revision: &str,
    ) -> Result<Arc<Registration>, GatewayError> {
        self.prepare_registration_with_offline(state, owner, provider, revision, false)
            .await
    }
    pub(crate) async fn prepare_registration_with_offline(
        &self,
        state: &Arc<AppState>,
        owner: &str,
        provider: &str,
        revision: &str,
        offline_access: bool,
    ) -> Result<Arc<Registration>, GatewayError> {
        let _creation = self
            .creations
            .try_read()
            .map_err(|_| super::login_error(LoginError::Stopped))?;
        if *self.stop.borrow() {
            return Err(super::login_error(LoginError::Stopped));
        }
        let _admission = self
            .admissions
            .try_acquire()
            .map_err(|_| super::login_error(LoginError::Capacity))?;
        let probe = Binding::new(
            owner,
            provider,
            revision,
            "registration-pending",
            "http://127.0.0.1:1/callback",
        )
        .map_err(super::login_error)?;
        super::validate_target(state, &probe)?;
        if let Some(record) = super::registration_storage::load(state)
            .await?
            .into_iter()
            .find(|r| {
                r.owner == owner && r.provider_id == provider && r.offline_access == offline_access
            })
        {
            let mut receipts = self.registrations.lock();
            if *self.stop.borrow() {
                return Err(super::login_error(LoginError::Stopped));
            }
            if let Some(receipt) = receipts.get(&record.id) {
                receipt.reconcile(&record).map_err(super::login_error)?;
                return Ok(receipt.clone());
            }
            // An old local candidate must not hide a different runtime's accepted receipt.
            receipts.retain(|_, receipt| {
                if receipt.binding.owner == owner
                    && receipt.binding.provider == provider
                    && receipt.binding.offline_access == offline_access
                {
                    receipt.cancel_prepared();
                    false
                } else {
                    true
                }
            });
            if receipts.len() >= 16 {
                return Err(super::login_error(LoginError::Capacity));
            }
            let receipt = Registration::restore(record);
            receipts.insert(receipt.id.clone(), receipt.clone());
            return Ok(receipt);
        }
        // No remote effect here. Reserve the actual callback before presenting the confirmation.
        let listener = tokio::net::TcpListener::bind((std::net::Ipv4Addr::LOCALHOST, 0))
            .await
            .map_err(|_| super::login_error(LoginError::Listener))?;
        let address = listener
            .local_addr()
            .map_err(|_| super::login_error(LoginError::Listener))?;
        let mut binding = Binding::new(
            owner,
            provider,
            revision,
            "registration-pending",
            &format!("http://{address}/callback"),
        )
        .map_err(super::login_error)?;
        binding.offline_access = offline_access;
        super::validate_target(state, &binding)?;
        let mut receipts = self.registrations.lock();
        if *self.stop.borrow() {
            return Err(super::login_error(LoginError::Stopped));
        }
        // Accepted remote attempts are never evicted. Only a never-started local reservation is replaceable.
        if let Some(receipt) = receipts
            .values()
            .find(|r| {
                r.binding.owner == owner
                    && r.binding.provider == provider
                    && r.binding.offline_access == offline_access
            })
            .cloned()
        {
            let status = receipt.view().status;
            let replaceable = matches!(
                status,
                super::RegistrationStatus::Expired | super::RegistrationStatus::Cancelled
            ) || (status == super::RegistrationStatus::Prepared
                && receipt.binding.revision != revision);
            if !replaceable {
                return Ok(receipt);
            }
            receipt.cancel_prepared();
            receipts.remove(&receipt.id);
        }
        if receipts.len() >= 16 {
            return Err(super::login_error(LoginError::Capacity));
        }
        let receipt = Registration::new(binding, listener);
        receipts.insert(receipt.id.clone(), receipt.clone());
        Ok(receipt)
    }
    pub(crate) fn registration_owned(
        &self,
        id: &str,
        owner: &str,
    ) -> Result<Arc<Registration>, GatewayError> {
        self.registrations
            .lock()
            .get(id)
            .filter(|receipt| receipt.binding.owner == owner)
            .cloned()
            .ok_or_else(|| super::login_error(LoginError::Unavailable))
    }
    pub(crate) async fn load_registration(
        &self,
        state: &AppState,
        id: &str,
        owner: &str,
    ) -> Result<Arc<Registration>, GatewayError> {
        let local = self.registration_owned(id, owner).ok();
        if let Some(receipt) = &local {
            if receipt.view().status != super::RegistrationStatus::Unconfirmed {
                return Ok(receipt.clone());
            }
        }
        if uuid::Uuid::parse_str(id).is_err() {
            return Err(super::login_error(LoginError::Unavailable));
        }
        let _admission = self
            .admissions
            .try_acquire()
            .map_err(|_| super::login_error(LoginError::Capacity))?;
        let stored = super::registration_storage::load(state)
            .await?
            .into_iter()
            .find(|r| r.id == id && r.owner == owner);
        match (local, stored) {
            (Some(receipt), record) => {
                if let Some(record) = record {
                    receipt.reconcile(&record).map_err(super::login_error)?;
                }
                Ok(receipt)
            }
            (None, Some(record)) => Ok(Registration::restore(record)),
            (None, None) => Err(super::login_error(LoginError::Unavailable)),
        }
    }
    pub(crate) fn start_registration(
        &self,
        state: &Arc<AppState>,
        id: &str,
        owner: &str,
        revision: &str,
        http: rquest::Client,
    ) -> Result<super::RegistrationView, GatewayError> {
        let receipts = self.registrations.lock();
        let receipt = receipts
            .get(id)
            .filter(|receipt| receipt.binding.owner == owner)
            .ok_or_else(|| super::login_error(LoginError::Unavailable))?;
        if receipt.binding.revision != revision {
            return Err(GatewayError::conflict(
                "Luma registration revision mismatch",
            ));
        }
        // Already accepted attempts are queried, never submitted again, even after target changes.
        if receipt.view().status != super::RegistrationStatus::Prepared {
            return Ok(receipt.view());
        }
        if *self.stop.borrow() {
            return Err(super::login_error(LoginError::Stopped));
        }
        super::validate_target(state, &receipt.binding)?;
        let store = super::registration_storage::persistence(state)?;
        let mut operations = self.operations.lock();
        reap(&mut operations);
        if operations.len() >= 32 {
            return Err(super::login_error(LoginError::Capacity));
        }
        if let Some(attempt) = receipt.begin() {
            let task = attempt.run(http, self.stop.subscribe(), store);
            #[cfg(test)]
            let task =
                crate::protocol::luma_oauth::test_support::inherit_registration_endpoint(task);
            operations.spawn(task);
        }
        Ok(receipt.view())
    }
}
