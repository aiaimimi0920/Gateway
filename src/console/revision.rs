use serde::de::Error as _;
use serde::{Deserialize, Deserializer, Serialize};
use time::OffsetDateTime;

use super::document::{CanonicalRouteDocument, ValidatedRouteDocument};

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum RevisionActor {
    Bootstrap,
    ManagementToken,
    EnvironmentOverride,
    Recovery,
    CredentialPoolAutomation,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RevisionMetadata {
    id: String,
    sequence: u64,
    parent: Option<String>,
    actor: RevisionActor,
    #[serde(with = "time::serde::rfc3339")]
    timestamp: OffsetDateTime,
    document_digest: String,
    yaml_digest: String,
    message: Option<String>,
}

#[derive(Debug, thiserror::Error, Clone, Eq, PartialEq)]
pub enum RevisionMetadataValidationError {
    #[error("document_digest must be exactly 64 lowercase hexadecimal characters")]
    InvalidDocumentDigest,
    #[error("yaml_digest must be exactly 64 lowercase hexadecimal characters")]
    InvalidYamlDigest,
    #[error("revision id must match r<sequence>-<document digest prefix>")]
    InvalidId,
    #[error("parent revision id must match r<canonical-u64>-<12 lowercase hex>")]
    InvalidParent,
    #[error("parent revision sequence must be lower than the current revision sequence")]
    ParentSequenceNotEarlier,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RevisionMetadataWire {
    id: String,
    sequence: u64,
    parent: Option<String>,
    actor: RevisionActor,
    #[serde(with = "time::serde::rfc3339")]
    timestamp: OffsetDateTime,
    document_digest: String,
    yaml_digest: String,
    message: Option<String>,
}

impl<'de> Deserialize<'de> for RevisionMetadata {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let wire = RevisionMetadataWire::deserialize(deserializer)?;
        let metadata = Self {
            id: wire.id,
            sequence: wire.sequence,
            parent: wire.parent,
            actor: wire.actor,
            timestamp: wire.timestamp,
            document_digest: wire.document_digest,
            yaml_digest: wire.yaml_digest,
            message: wire.message,
        };
        metadata.validate().map_err(D::Error::custom)?;
        Ok(metadata)
    }
}

impl RevisionMetadata {
    pub fn validate(&self) -> Result<(), RevisionMetadataValidationError> {
        if !is_lowercase_hex(&self.document_digest, 64) {
            return Err(RevisionMetadataValidationError::InvalidDocumentDigest);
        }
        if !is_lowercase_hex(&self.yaml_digest, 64) {
            return Err(RevisionMetadataValidationError::InvalidYamlDigest);
        }
        let expected_id = format!("r{}-{}", self.sequence, &self.document_digest[..12]);
        if self.id != expected_id {
            return Err(RevisionMetadataValidationError::InvalidId);
        }
        if let Some(parent) = &self.parent {
            let parent_sequence = parse_parent_sequence(parent)
                .ok_or(RevisionMetadataValidationError::InvalidParent)?;
            if parent_sequence >= self.sequence {
                return Err(RevisionMetadataValidationError::ParentSequenceNotEarlier);
            }
        }
        Ok(())
    }

    pub fn id(&self) -> &str {
        &self.id
    }

    pub fn sequence(&self) -> u64 {
        self.sequence
    }

    pub fn parent(&self) -> Option<&str> {
        self.parent.as_deref()
    }

    pub fn actor(&self) -> RevisionActor {
        self.actor
    }

    pub fn timestamp(&self) -> OffsetDateTime {
        self.timestamp
    }

    pub fn document_digest(&self) -> &str {
        &self.document_digest
    }

    pub fn yaml_digest(&self) -> &str {
        &self.yaml_digest
    }

    pub fn message(&self) -> Option<&str> {
        self.message.as_deref()
    }

    pub fn from_validated(
        sequence: u64,
        parent: Option<String>,
        actor: RevisionActor,
        timestamp: OffsetDateTime,
        message: Option<String>,
        document: &ValidatedRouteDocument,
    ) -> Self {
        Self::from_digests(
            sequence,
            parent,
            actor,
            timestamp,
            message,
            document.document_digest(),
            document.yaml_digest(),
        )
    }

    pub fn from_canonical(
        sequence: u64,
        parent: Option<String>,
        actor: RevisionActor,
        timestamp: OffsetDateTime,
        message: Option<String>,
        document: &CanonicalRouteDocument,
    ) -> Self {
        Self::from_digests(
            sequence,
            parent,
            actor,
            timestamp,
            message,
            document.document_digest(),
            document.yaml_digest(),
        )
    }

    fn from_digests(
        sequence: u64,
        parent: Option<String>,
        actor: RevisionActor,
        timestamp: OffsetDateTime,
        message: Option<String>,
        document_digest: &str,
        yaml_digest: &str,
    ) -> Self {
        let digest_prefix = document_digest.get(..12).unwrap_or(document_digest);
        Self {
            id: format!("r{sequence}-{digest_prefix}"),
            sequence,
            parent,
            actor,
            timestamp,
            document_digest: document_digest.to_string(),
            yaml_digest: yaml_digest.to_string(),
            message,
        }
    }
}

fn is_lowercase_hex(value: &str, expected_len: usize) -> bool {
    value.len() == expected_len
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || matches!(byte, b'a'..=b'f'))
}

fn parse_parent_sequence(parent: &str) -> Option<u64> {
    let (sequence, digest) = parent.strip_prefix('r')?.split_once('-')?;
    if sequence.is_empty()
        || (sequence.len() > 1 && sequence.starts_with('0'))
        || !is_lowercase_hex(digest, 12)
    {
        return None;
    }
    let sequence = sequence.parse::<u64>().ok()?;
    (parent == format!("r{sequence}-{digest}")).then_some(sequence)
}
