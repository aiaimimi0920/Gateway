//! Durable public-client receipts: an accepted unknown result is never automatically replayed.
use super::{
    file_io::read_json_limited, PersistenceError, RouteConfigPersistence, WriterLockGuard,
};
use serde::{Deserialize, Serialize};
use std::{collections::HashSet, fs, io};

#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct LumaRegistrationRecord {
    pub id: String,
    pub owner: String,
    pub provider_id: String,
    pub expected_revision: String,
    pub redirect_uri: String,
    // None is durable uncertainty, including a crash before the remote request was sent.
    pub client_id: Option<String>,
    #[serde(default, skip_serializing_if = "is_false")]
    pub offline_access: bool,
}
fn is_false(value: &bool) -> bool {
    !value
}
impl LumaRegistrationRecord {
    fn validate(&self) -> Result<(), PersistenceError> {
        let valid_id = uuid::Uuid::parse_str(&self.id).is_ok_and(|id| id.to_string() == self.id);
        if !valid_id
            || crate::console::luma_oauth::Binding::new(
                &self.owner,
                &self.provider_id,
                &self.expected_revision,
                self.client_id.as_deref().unwrap_or("registration-pending"),
                &self.redirect_uri,
            )
            .is_err()
        {
            return Err(PersistenceError::corruption(
                "Invalid Luma registration receipt",
            ));
        }
        Ok(())
    }
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Records {
    version: u32,
    registrations: Vec<LumaRegistrationRecord>,
}
impl RouteConfigPersistence {
    pub(crate) fn load_luma_registrations_locked(
        &self,
        guard: &WriterLockGuard,
    ) -> Result<Vec<LumaRegistrationRecord>, PersistenceError> {
        self.ensure_guard(guard)?;
        let path = self.state_root().join("luma-client-registrations.json");
        match fs::symlink_metadata(&path) {
            Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(Vec::new()),
            Err(error) => return Err(PersistenceError::io("reading Luma receipt metadata", error)),
            Ok(_) => {}
        }
        let records: Records = read_json_limited(&path)?;
        if records.version != 1 || records.registrations.len() > 16 {
            return Err(PersistenceError::corruption(
                "Unsupported Luma receipt store",
            ));
        }
        let mut ids = HashSet::new();
        let mut owners = HashSet::new();
        for record in &records.registrations {
            record.validate()?;
            if !ids.insert(&record.id)
                || !owners.insert((&record.owner, &record.provider_id, record.offline_access))
            {
                return Err(PersistenceError::corruption(
                    "Duplicate Luma registration receipt",
                ));
            }
        }
        Ok(records.registrations)
    }

    pub(crate) fn claim_luma_registration_locked(
        &self,
        guard: &WriterLockGuard,
        record: &LumaRegistrationRecord,
    ) -> Result<bool, PersistenceError> {
        self.ensure_writable()?;
        record.validate()?;
        if record.client_id.is_some() {
            return Err(PersistenceError::corruption(
                "Registration claim already contains a result",
            ));
        }
        // Read and decide under the same OS writer lock, including between separate runtimes.
        let mut records = self.load_luma_registrations_locked(guard)?;
        if records.iter().any(|r| {
            r.owner == record.owner
                && r.provider_id == record.provider_id
                && r.offline_access == record.offline_access
        }) {
            return Ok(false);
        }
        if records.len() >= 16 || records.iter().any(|r| r.id == record.id) {
            return Err(PersistenceError::recovery_required(
                "Luma registration receipt capacity exhausted",
            ));
        }
        records.push(record.clone());
        self.write_luma_registrations(guard, records)?;
        Ok(true)
    }

    pub(crate) fn finish_luma_registration_locked(
        &self,
        guard: &WriterLockGuard,
        accepted: &LumaRegistrationRecord,
        client_id: String,
    ) -> Result<(), PersistenceError> {
        self.ensure_writable()?;
        accepted.validate()?;
        let mut desired = accepted.clone();
        desired.client_id = Some(client_id);
        desired.validate()?;
        let mut records = self.load_luma_registrations_locked(guard)?;
        let record = records
            .iter_mut()
            .find(|r| r.id == accepted.id)
            .ok_or_else(|| PersistenceError::corruption("Missing accepted Luma registration"))?;
        if *record == desired {
            return Ok(());
        }
        if *record != *accepted || accepted.client_id.is_some() {
            return Err(PersistenceError::corruption(
                "Luma registration receipt binding changed",
            ));
        }
        *record = desired;
        self.write_luma_registrations(guard, records)
    }

    fn write_luma_registrations(
        &self,
        guard: &WriterLockGuard,
        registrations: Vec<LumaRegistrationRecord>,
    ) -> Result<(), PersistenceError> {
        self.atomic_write_json_locked(
            guard,
            &self.state_root().join("luma-client-registrations.json"),
            &Records {
                version: 1,
                registrations,
            },
        )
    }
}
