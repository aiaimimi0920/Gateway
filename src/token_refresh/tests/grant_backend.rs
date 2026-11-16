//! Synthetic backend implements the same permanent-head CAS contract, not a real Redis interpreter.
use super::*;
use crate::console::refresh_grants::*;

fn invalid() -> RouteConfigRedisStoreError {
    RouteConfigRedisStoreError::from_persistence(crate::console::refresh_grants::invalid())
}

impl Backend {
    pub(super) fn gate_load(
        &self,
        locator: &GrantLocator,
    ) -> Result<Option<GrantRecord>, RouteConfigRedisStoreError> {
        let state = self.state.lock().unwrap();
        let record = state
            .grants
            .get(&(locator.family.clone(), locator.material.clone()))
            .cloned();
        if record
            .as_ref()
            .is_some_and(|record| !locator.matches(record))
        {
            return Err(invalid());
        }
        Ok(record)
    }

    pub(super) async fn gate_repair(
        &self,
        revision: &RouteConfigRedisRevision,
        record: &GrantRecord,
        transaction: &TransactionRecord,
    ) -> Result<(), RouteConfigRedisStoreError> {
        {
            let state = self.state.lock().unwrap();
            if state.active.to_json()? != revision.to_json()?
                || state
                    .grants
                    .get(&(record.family_id(), record.material_id()))
                    != Some(record)
                || state
                    .transactions
                    .get(transaction.tx_id())
                    .is_none_or(|stored| {
                        stored.phase() != crate::console::TransactionPhase::RedisActivated
                            || stored.prepared_identity().unwrap()
                                != transaction.prepared_identity().unwrap()
                    })
            {
                return Err(invalid());
            }
        }
        let pool = self.cache.lock().unwrap().clone().ok_or_else(invalid)?;
        use redis::AsyncCommands;
        let store = revision
            .clone()
            .into_store(crate::routing::config::ActiveConfigSource::Recovered)?;
        let snapshot = store.snapshot();
        let (_, credential) = snapshot
            .oauth_refresh_target(record.provider_id(), record.credential_id())
            .ok_or_else(invalid)?;
        let mut connection = pool.get().await.map_err(|_| invalid())?;
        let key = crate::redis::keys::credential_key(record.credential_id());
        let raw: String = connection.get(&key).await.map_err(|_| invalid())?;
        let Some((updated, ttl)) =
            crate::token_refresh::server_cache::patch_from_grant(&raw, record, &credential.payload)
                .map_err(|_| invalid())?
        else {
            return Ok(());
        };
        let applied: i32 = redis::Script::new(crate::token_refresh::server_cache::CAS)
            .key(key)
            .arg(raw)
            .arg(updated)
            .arg(ttl)
            .invoke_async(&mut connection)
            .await
            .map_err(|_| invalid())?;
        if applied == 1 {
            Ok(())
        } else {
            Err(invalid())
        }
    }

    pub(super) fn gate_reserve(
        &self,
        revision: &RouteConfigRedisRevision,
        request: &GrantRequest,
    ) -> Result<GrantReservation, RouteConfigRedisStoreError> {
        let mut state = self.state.lock().unwrap();
        if self.absent.load(Ordering::SeqCst)
            || !request.matches(revision)
            || !crate::console::refresh_grants::same_authority_document(&state.active, revision)
                .map_err(RouteConfigRedisStoreError::from_persistence)?
        {
            return Ok(GrantReservation::Stale);
        }
        let family = request.family_id();
        let material = request.material_id();
        if let Some(record) = state.grants.get(&(family.clone(), material.clone())) {
            return Ok(GrantReservation::Existing(record.clone()));
        }
        if let Some(head) = state.grant_heads.get(&family) {
            let prior = state
                .grants
                .get(&(family.clone(), head.clone()))
                .ok_or_else(invalid)?;
            if !request.can_follow(prior) {
                return Ok(GrantReservation::Blocked);
            }
        }
        let record = GrantRecord::sending(request);
        state
            .grants
            .insert((family.clone(), material.clone()), record.clone());
        state.grant_heads.insert(family, material);
        Ok(GrantReservation::Reserved(record))
    }

    pub(super) fn gate_result(
        &self,
        record: &GrantRecord,
        result: &GrantResult,
    ) -> Result<GrantRecord, RouteConfigRedisStoreError> {
        let next = record
            .with_result(result)
            .map_err(RouteConfigRedisStoreError::from_persistence)?;
        transition(&mut self.state.lock().unwrap(), record, &next)
    }

    pub(super) fn gate_receipt(
        &self,
        record: &GrantRecord,
        receipt: &GrantReceipt,
    ) -> Result<GrantRecord, RouteConfigRedisStoreError> {
        let next = record
            .with_receipt(receipt)
            .map_err(RouteConfigRedisStoreError::from_persistence)?;
        transition(&mut self.state.lock().unwrap(), record, &next)
    }

    pub(super) fn gate_publish(
        &self,
        revision: &RouteConfigRedisRevision,
        record: &GrantRecord,
    ) -> Result<GrantRecord, RouteConfigRedisStoreError> {
        let mut state = self.state.lock().unwrap();
        if state.active.to_json()? != revision.to_json()? {
            return Err(invalid());
        }
        let next = record
            .published(revision)
            .map_err(RouteConfigRedisStoreError::from_persistence)?;
        transition(&mut state, record, &next)
    }
}

fn transition(
    state: &mut AuthorityState,
    expected: &GrantRecord,
    desired: &GrantRecord,
) -> Result<GrantRecord, RouteConfigRedisStoreError> {
    let key = (expected.family_id(), expected.material_id());
    let current = state.grants.get(&key).ok_or_else(invalid)?;
    if current == desired
        || current.acknowledges_receipt(desired)
        || (current.phase() == GrantPhase::Published
            && current.same_attempt(expected)
            && current.result() == desired.result())
    {
        return Ok(current.clone());
    }
    if current != expected || state.grant_heads.get(&key.0) != Some(&key.1) {
        return Err(invalid());
    }
    state.grants.insert(key, desired.clone());
    Ok(desired.clone())
}
