//! Stable object names make retries idempotent without storing passwords in names.
use crate::routing::config::ProviderCredentialYaml;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(super) struct ArchiveRecord {
    pub schema_version: u8,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_revision: Option<String>,
    pub provider_id: String,
    pub credential_id: String,
    pub archived_at: String,
    pub reason: String,
    pub credential: ProviderCredentialYaml,
}

pub(super) fn object_key(
    credential_id: &str,
    credential: &ProviderCredentialYaml,
) -> anyhow::Result<String> {
    let mut value = serde_json::to_value(credential)?;
    sort_objects(&mut value);
    let mut digest = Sha256::new();
    digest.update(credential_id.as_bytes());
    digest.update([0]);
    digest.update(serde_json::to_vec(&value)?);
    Ok(format!("{:x}.json", digest.finalize()))
}

/// Different source revisions must never reuse an archive being purged.
pub(super) fn revision_object_key(
    credential_id: &str,
    credential: &ProviderCredentialYaml,
    revision: &str,
) -> anyhow::Result<String> {
    let Some((sequence, digest)) = revision.strip_prefix('r').and_then(|v| v.split_once('-'))
    else {
        anyhow::bail!("Archive source revision is invalid");
    };
    let number: u64 = sequence
        .parse()
        .map_err(|_| anyhow::anyhow!("Archive source revision is invalid"))?;
    if number.to_string() != sequence
        || digest.len() != 12
        || !digest
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
    {
        anyhow::bail!("Archive source revision is invalid");
    }
    let payload_key = object_key(credential_id, credential)?;
    Ok(format!(
        "v2-{:x}.json",
        Sha256::digest(format!("{revision}\0{payload_key}"))
    ))
}

fn sort_objects(value: &mut Value) {
    match value {
        Value::Object(map) => {
            map.sort_keys();
            for value in map.values_mut() {
                sort_objects(value);
            }
        }
        Value::Array(values) => {
            for value in values {
                sort_objects(value);
            }
        }
        _ => {}
    }
}

pub(super) fn owned_record(bytes: &[u8], key: &str, provider_id: &str) -> Option<ArchiveRecord> {
    let record: ArchiveRecord = serde_json::from_slice(bytes).ok()?;
    let expected_key = match (record.schema_version, record.source_revision.as_deref()) {
        (1, None) => object_key(&record.credential_id, &record.credential).ok(),
        (2, Some(revision)) => {
            revision_object_key(&record.credential_id, &record.credential, revision).ok()
        }
        _ => None,
    };
    if expected_key.as_deref() != Some(key)
        || record.provider_id != provider_id
        || record.credential_id.trim().is_empty()
        || record.reason != "permanent_driver_rejection"
        || record
            .credential
            .id
            .as_deref()
            .is_some_and(|id| id != record.credential_id)
        || time::OffsetDateTime::parse(
            &record.archived_at,
            &time::format_description::well_known::Rfc3339,
        )
        .is_err()
    {
        return None;
    }
    Some(record)
}
