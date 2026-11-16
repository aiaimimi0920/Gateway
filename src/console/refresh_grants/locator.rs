//! A locator names one permanent attempt without copying its secret material.
use super::{invalid, valid_digest, GrantRecord};
use crate::console::PersistenceError;
use serde::{Deserialize, Serialize};

#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct GrantLocator {
    pub(crate) family: String,
    pub(crate) material: String,
    identity: String,
}

impl GrantLocator {
    pub(crate) fn from_record(record: &GrantRecord) -> Result<Self, PersistenceError> {
        Ok(Self {
            family: record.family_id(),
            material: record.material_id(),
            identity: record.identity_digest()?,
        })
    }

    pub(crate) fn validate(&self) -> Result<(), PersistenceError> {
        if ![&self.family, &self.material, &self.identity]
            .iter()
            .all(|value| valid_digest(value))
        {
            return Err(invalid());
        }
        Ok(())
    }

    pub(crate) fn matches(&self, record: &GrantRecord) -> bool {
        Self::from_record(record).is_ok_and(|actual| actual == *self)
    }
}
