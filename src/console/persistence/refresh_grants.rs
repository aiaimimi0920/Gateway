//! Local grant consumption records share the real writer lock and authoritative active pointer.
use super::{file_io::read_json_limited, paths::ensure_secure_directory, *};
use crate::console::refresh_grants::{
    invalid, valid_digest, GrantReceipt, GrantRecord, GrantRequest, GrantReservation, GrantResult,
};
use crate::console::RouteConfigRedisRevision;

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Head {
    material: String,
}

impl RouteConfigPersistence {
    pub(crate) fn find_refresh_grant_locked(
        &self,
        guard: &WriterLockGuard,
        request: &GrantRequest,
    ) -> Result<Option<GrantRecord>, PersistenceError> {
        self.ensure_guard(guard)?;
        self.load_grant(&request.family_id(), &request.material_id())
    }

    pub(crate) fn load_refresh_grant_locked(
        &self,
        guard: &WriterLockGuard,
        locator: &crate::console::refresh_grants::GrantLocator,
    ) -> Result<Option<GrantRecord>, PersistenceError> {
        self.ensure_guard(guard)?;
        locator.validate()?;
        let record = self.load_grant(&locator.family, &locator.material)?;
        if record
            .as_ref()
            .is_some_and(|record| !locator.matches(record))
        {
            return Err(invalid());
        }
        Ok(record)
    }

    fn grant_path(
        &self,
        family: &str,
        material: Option<&str>,
    ) -> Result<PathBuf, PersistenceError> {
        if !valid_digest(family) || material.is_some_and(|id| !valid_digest(id)) {
            return Err(invalid());
        }
        let dir = self.state_root().join("refresh-grants").join(family);
        // A digest, not a token, is the only variable path component. Reject existing links on reads too.
        super::paths::reject_existing_links(&dir)?;
        let path = dir.join(
            material
                .map(|id| format!("{id}.json"))
                .unwrap_or_else(|| "head.json".into()),
        );
        self.validate_optional_managed_file(&path)?;
        Ok(path)
    }

    fn grant_authority_matches(
        &self,
        revision: &RouteConfigRedisRevision,
    ) -> Result<bool, PersistenceError> {
        if self.local_active_revision()?.as_deref() != Some(revision.metadata().id()) {
            return Ok(false);
        }
        let stored = self
            .load_revision(revision.metadata().id())?
            .ok_or_else(invalid)?;
        let actual =
            RouteConfigRedisRevision::new(stored.metadata().clone(), stored.document().clone())
                .map_err(|_| invalid())?;
        crate::console::refresh_grants::same_authority_document(&actual, revision)
    }

    fn load_grant(
        &self,
        family: &str,
        material: &str,
    ) -> Result<Option<GrantRecord>, PersistenceError> {
        let path = self.grant_path(family, Some(material))?;
        if !path
            .try_exists()
            .map_err(|e| PersistenceError::io("checking refresh grant", e))?
        {
            return Ok(None);
        }
        let raw: serde_json::Value = read_json_limited(&path)?;
        let record = GrantRecord::from_json(&raw.to_string())?;
        if record.family_id() != family || record.material_id() != material {
            return Err(invalid());
        }
        Ok(Some(record))
    }

    pub fn reserve_refresh_grant_locked(
        &self,
        guard: &WriterLockGuard,
        revision: &RouteConfigRedisRevision,
        request: &GrantRequest,
    ) -> Result<GrantReservation, PersistenceError> {
        self.ensure_guard(guard)?;
        self.ensure_writable()?;
        if !request.matches(revision) || !self.grant_authority_matches(revision)? {
            return Ok(GrantReservation::Stale);
        }
        let family = request.family_id();
        let material = request.material_id();
        if let Some(record) = self.load_grant(&family, &material)? {
            return Ok(GrantReservation::Existing(record));
        }
        let head_path = self.grant_path(&family, None)?;
        if head_path
            .try_exists()
            .map_err(|e| PersistenceError::io("checking refresh grant head", e))?
        {
            let head: Head = read_json_limited(&head_path)?;
            let prior = self
                .load_grant(&family, &head.material)?
                .ok_or_else(invalid)?;
            if !request.can_follow(&prior) {
                return Ok(GrantReservation::Blocked);
            }
        }
        let record = GrantRecord::sending(request);
        record.to_json()?;
        ensure_secure_directory(head_path.parent().ok_or_else(invalid)?)?;
        // Publish head first: a crash between these writes leaves a dangling head and fails closed.
        self.atomic_write_json_locked(
            guard,
            &head_path,
            &Head {
                material: material.clone(),
            },
        )?;
        self.atomic_write_json_locked(guard, &self.grant_path(&family, Some(&material))?, &record)?;
        Ok(GrantReservation::Reserved(record))
    }

    pub fn store_refresh_grant_receipt_locked(
        &self,
        guard: &WriterLockGuard,
        expected: &GrantRecord,
        receipt: &GrantReceipt,
    ) -> Result<GrantRecord, PersistenceError> {
        let desired = expected.with_receipt(receipt)?;
        self.transition_grant(guard, expected, &desired)
    }

    pub fn store_refresh_grant_result_locked(
        &self,
        guard: &WriterLockGuard,
        expected: &GrantRecord,
        result: &GrantResult,
    ) -> Result<GrantRecord, PersistenceError> {
        let desired = expected.with_result(result)?;
        self.transition_grant(guard, expected, &desired)
    }

    pub fn mark_refresh_grant_published_locked(
        &self,
        guard: &WriterLockGuard,
        revision: &RouteConfigRedisRevision,
        expected: &GrantRecord,
    ) -> Result<GrantRecord, PersistenceError> {
        self.ensure_guard(guard)?;
        if !self.grant_authority_matches(revision)? {
            return Err(invalid());
        }
        let desired = expected.published(revision)?;
        self.transition_grant(guard, expected, &desired)
    }

    fn transition_grant(
        &self,
        guard: &WriterLockGuard,
        expected: &GrantRecord,
        desired: &GrantRecord,
    ) -> Result<GrantRecord, PersistenceError> {
        self.ensure_guard(guard)?;
        self.ensure_writable()?;
        expected.validate()?;
        desired.to_json()?;
        let family = expected.family_id();
        let material = expected.material_id();
        let current = self.load_grant(&family, &material)?.ok_or_else(invalid)?;
        if current == *desired {
            return Ok(current);
        }
        if current.acknowledges_receipt(desired) {
            return Ok(current);
        }
        // A repeated result save after publication is idempotent, never a phase downgrade.
        if current.phase() == crate::console::refresh_grants::GrantPhase::Published
            && current.same_attempt(expected)
            && current.result() == desired.result()
            && current.receipt() == desired.receipt()
        {
            return Ok(current);
        }
        if current != *expected {
            return Err(invalid());
        }
        let head: Head = read_json_limited(&self.grant_path(&family, None)?)?;
        if head.material != material {
            return Err(invalid());
        }
        self.atomic_write_json_locked(guard, &self.grant_path(&family, Some(&material))?, desired)?;
        Ok(desired.clone())
    }
}
