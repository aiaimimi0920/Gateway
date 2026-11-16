//! Only an exact accepted result may advance a permanent send intent; published history is retained.
use super::{invalid, GrantMaterial, GrantReceipt, GrantRequest, MAX_GRANT_BYTES};
use crate::console::{PersistenceError, RouteConfigRedisRevision};
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum GrantPhase {
    Sending,
    Result,
    Published,
}

#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct GrantResult {
    pub(crate) material: GrantMaterial,
    pub(crate) expires_in: Option<u64>,
    pub(crate) expires_at: Option<String>,
}

impl GrantResult {
    pub(crate) fn validate(&self, original: &GrantMaterial) -> Result<(), PersistenceError> {
        let lifetime_valid = if original.luma_oauth.is_some() {
            self.expires_in.is_none()
        } else {
            match (&self.expires_in, &self.expires_at) {
                (None, None) => original.lobsterai.is_some(),
                (Some(seconds), Some(expiry)) => {
                    original.lobsterai.is_none()
                        && *seconds > 0
                        && expiry.len() <= 64
                        && time::OffsetDateTime::parse(
                            expiry,
                            &time::format_description::well_known::Rfc3339,
                        )
                        .is_ok()
                }
                _ => false,
            }
        };
        if !lifetime_valid
            || self.material.access.trim().is_empty()
            || self.material.expires_at != self.expires_at
            || !original.permits(&self.material)
        {
            return Err(invalid());
        }
        Ok(())
    }
}

#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct GrantRecord {
    version: u32,
    attempt: String,
    provider: String,
    credential: String,
    revision: String,
    original: GrantMaterial,
    phase: GrantPhase,
    result: Option<GrantResult>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    receipt: Option<GrantReceipt>,
}

#[derive(Debug, PartialEq, Eq)]
pub enum GrantReservation {
    Reserved(GrantRecord),
    Existing(GrantRecord),
    Blocked,
    Stale,
    Contended,
}

impl GrantRecord {
    pub(crate) fn identity_digest(&self) -> Result<String, PersistenceError> {
        use sha2::{Digest, Sha256};
        self.validate()?;
        let mut identity = self.clone();
        identity.phase = GrantPhase::Sending;
        identity.result = None;
        identity.receipt = None;
        Ok(hex::encode(Sha256::digest(identity.to_json()?.as_bytes())))
    }

    pub(crate) fn original(&self) -> &GrantMaterial {
        &self.original
    }

    pub(crate) fn credential_id(&self) -> &str {
        &self.credential
    }

    pub(crate) fn provider_id(&self) -> &str {
        &self.provider
    }

    pub(crate) fn matches_publication(
        &self,
        parent: &RouteConfigRedisRevision,
        next: &RouteConfigRedisRevision,
        provider: &str,
        credential: &str,
    ) -> bool {
        self.provider == provider
            && self.credential == credential
            && GrantRequest::from_revision(parent, provider, credential)
                .is_ok_and(|request| self.original.equivalent(&request.material))
            && self.published(next).is_ok()
    }

    pub(crate) fn sending(request: &GrantRequest) -> Self {
        Self {
            version: 1,
            attempt: uuid::Uuid::new_v4().to_string(),
            provider: request.provider.clone(),
            credential: request.credential.clone(),
            revision: request.revision.clone(),
            original: request.material.clone(),
            phase: GrantPhase::Sending,
            result: None,
            receipt: None,
        }
    }

    pub fn phase(&self) -> GrantPhase {
        self.phase
    }
    pub fn result(&self) -> Option<&GrantResult> {
        self.result.as_ref()
    }
    pub fn receipt(&self) -> Option<&GrantReceipt> {
        self.receipt.as_ref()
    }
    /// A receipt may be recovered for validation only; it never authorizes replay as Result.
    pub(crate) fn receipt_for(&self, request: &GrantRequest) -> Option<&GrantReceipt> {
        (self.phase == GrantPhase::Sending
            && self.provider == request.provider
            && self.credential == request.credential
            && self.revision == request.revision
            && self.original == request.material)
            .then_some(self.receipt.as_ref())
            .flatten()
    }
    /// A restored revision/material is not a continuation of a deleted/re-added authorization.
    pub(crate) fn replay_for(&self, request: &GrantRequest) -> Option<&GrantResult> {
        (self.phase == GrantPhase::Result
            && self.provider == request.provider
            && self.credential == request.credential
            && self.revision == request.revision
            && self.original == request.material)
            .then_some(self.result.as_ref())
            .flatten()
    }
    pub fn family_id(&self) -> String {
        self.original.family_id()
    }
    pub fn material_id(&self) -> String {
        self.original.material_id()
    }

    pub fn from_json(raw: &str) -> Result<Self, PersistenceError> {
        if raw.len() > MAX_GRANT_BYTES {
            return Err(invalid());
        }
        let record: Self = serde_json::from_str(raw).map_err(|_| invalid())?;
        record.validate()?;
        Ok(record)
    }

    pub fn to_json(&self) -> Result<String, PersistenceError> {
        self.validate()?;
        let raw = serde_json::to_string(self).map_err(|_| invalid())?;
        if raw.len() > MAX_GRANT_BYTES {
            return Err(invalid());
        }
        Ok(raw)
    }

    pub(crate) fn validate(&self) -> Result<(), PersistenceError> {
        if self.version != 1
            || uuid::Uuid::parse_str(&self.attempt).is_err()
            || [&self.provider, &self.credential, &self.revision]
                .iter()
                .any(|s| s.is_empty() || s.len() > 512)
            || (self.phase == GrantPhase::Sending) != self.result.is_none()
        {
            return Err(invalid());
        }
        self.original.validate()?;
        if let Some(receipt) = &self.receipt {
            receipt.validate(&self.original)?;
        }
        if let Some(result) = &self.result {
            result.validate(&self.original)?;
            // Only newly typed Nous actors require receipts; legacy generic records keep their contract.
            if (self.original.nous_portal.is_some()
                || self.original.lobsterai.is_some()
                || self.original.luma_oauth.is_some())
                && self.receipt.is_none()
            {
                return Err(invalid());
            }
            if let Some(receipt) = &self.receipt {
                receipt.validates_result(&self.original, result)?;
            }
        }
        Ok(())
    }

    pub(crate) fn with_result(&self, result: &GrantResult) -> Result<Self, PersistenceError> {
        self.validate()?;
        result.validate(&self.original)?;
        if let Some(saved) = &self.result {
            return if saved == result {
                Ok(self.clone())
            } else {
                Err(invalid())
            };
        }
        let mut record = self.clone();
        record.result = Some(result.clone());
        record.phase = GrantPhase::Result;
        record.to_json()?;
        Ok(record)
    }

    pub(crate) fn with_receipt(&self, receipt: &GrantReceipt) -> Result<Self, PersistenceError> {
        self.validate()?;
        receipt.validate(&self.original)?;
        if let Some(saved) = &self.receipt {
            return if saved == receipt {
                Ok(self.clone())
            } else {
                Err(invalid())
            };
        }
        if self.phase != GrantPhase::Sending {
            return Err(invalid());
        }
        let mut record = self.clone();
        record.receipt = Some(receipt.clone());
        record.to_json()?;
        Ok(record)
    }

    /// A delayed identical receipt acknowledgement cannot downgrade accepted or published state.
    pub(crate) fn acknowledges_receipt(&self, desired: &Self) -> bool {
        self.phase != GrantPhase::Sending
            && desired.phase == GrantPhase::Sending
            && self.same_attempt(desired)
            && desired.receipt.is_some()
            && self.receipt == desired.receipt
    }

    pub(crate) fn same_attempt(&self, other: &Self) -> bool {
        let mut left = self.clone();
        let mut right = other.clone();
        left.phase = GrantPhase::Sending;
        right.phase = GrantPhase::Sending;
        left.result = None;
        right.result = None;
        left.receipt = None;
        right.receipt = None;
        left == right
    }

    pub(crate) fn matches_request(&self, request: &GrantRequest) -> bool {
        self.provider == request.provider
            && self.credential == request.credential
            && self.revision == request.revision
            && self.original == request.material
    }

    /// The PostgreSQL owner supplies its locked parent and actual in-transaction successor.
    pub(crate) fn published_for_database(
        &self,
        parent: &GrantRequest,
        successor: &GrantRequest,
    ) -> Result<Self, PersistenceError> {
        self.validate()?;
        let result = self.result.as_ref().ok_or_else(invalid)?;
        if self.phase != GrantPhase::Result
            || !self.matches_request(parent)
            || successor.provider != self.provider
            || successor.credential != self.credential
            || !successor.material.equivalent(&result.material)
            || successor.material.expires_at != result.expires_at
        {
            return Err(invalid());
        }
        let mut published = self.clone();
        published.phase = GrantPhase::Published;
        Ok(published)
    }

    pub(crate) fn published(
        &self,
        revision: &RouteConfigRedisRevision,
    ) -> Result<Self, PersistenceError> {
        self.validate()?;
        let result = self.result.as_ref().ok_or_else(invalid)?;
        let request = GrantRequest::from_revision(revision, &self.provider, &self.credential)?;
        let credential = revision
            .document()
            .providers
            .iter()
            .find(|p| p.id == self.provider)
            .unwrap()
            .credentials
            .iter()
            .enumerate()
            .find(|(index, c)| {
                crate::routing::config::effective_credential_id(&self.provider, *index, c)
                    == self.credential
            })
            .map(|(_, c)| c)
            .ok_or_else(invalid)?;
        if !request.material.equivalent(&result.material)
            || if result.material.luma_oauth.is_some() {
                credential.expires_at.is_some() || credential.token_expires_in_secs.is_some()
            } else {
                credential.expires_at != result.expires_at
                    || credential.token_expires_in_secs != result.expires_in
            }
        {
            return Err(invalid());
        }
        let mut record = self.clone();
        record.phase = GrantPhase::Published;
        Ok(record)
    }
}

impl std::fmt::Debug for GrantRecord {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("GrantRecord")
            .field("phase", &self.phase)
            .field("material", &"<redacted>")
            .finish()
    }
}
impl std::fmt::Debug for GrantResult {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("GrantResult { material: <redacted> }")
    }
}
