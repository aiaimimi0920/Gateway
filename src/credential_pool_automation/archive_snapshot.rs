//! Freeze provider-owned local archive files before a durable route-CAS purge barrier.
use super::{archive::checked_archive_reader, storage_paths::reject_linked_components};
use sha2::{Digest, Sha256};
use std::{
    fs,
    io::Read,
    path::{Path, PathBuf},
};

pub(crate) struct LocalArchiveSnapshot {
    directory: PathBuf,
    records: Vec<(PathBuf, [u8; 32])>,
}
impl LocalArchiveSnapshot {
    pub(crate) fn capture(directory: &Path, provider_id: &str) -> anyhow::Result<Self> {
        reject_linked_components(directory)?;
        let mut result = Self {
            directory: directory.into(),
            records: Vec::new(),
        };
        let entries = match fs::read_dir(directory) {
            Ok(entries) => entries,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(result),
            Err(error) => return Err(error.into()),
        };
        let mut metadata_bytes = 0usize;
        for (scanned, entry) in entries.enumerate() {
            if scanned >= 1024 {
                anyhow::bail!("Archive snapshot scan limit exceeded");
            }
            let path = entry?.path();
            if path.extension().and_then(|v| v.to_str()) != Some("json") {
                continue;
            }
            let metadata = fs::symlink_metadata(&path)?;
            if !metadata.is_file() || metadata.file_type().is_symlink() {
                continue;
            }
            let bytes = read_bounded(&path)?;
            if !checked_archive_reader(bytes.as_slice(), provider_id)? {
                continue;
            }
            metadata_bytes = metadata_bytes.saturating_add(path.as_os_str().len() + 32);
            if metadata_bytes > 2 * 1024 * 1024 {
                anyhow::bail!("Archive snapshot metadata limit exceeded");
            }
            result.records.push((path, Sha256::digest(&bytes).into()));
        }
        Ok(result)
    }
    pub(crate) fn is_empty(&self) -> bool {
        self.records.is_empty()
    }
    pub(crate) fn execute(
        self,
        current_revision: &(dyn Fn() -> bool + Send + Sync),
    ) -> anyhow::Result<usize> {
        let mut removed = 0;
        for (path, expected_digest) in self.records {
            let result = (|| -> anyhow::Result<()> {
                reject_linked_components(&self.directory)?;
                reject_linked_components(&path)?;
                if !fs::symlink_metadata(&path)?.is_file() {
                    anyhow::bail!("Archive file was replaced");
                }
                let digest: [u8; 32] = Sha256::digest(read_bounded(&path)?).into();
                if digest != expected_digest {
                    anyhow::bail!("Archive changed after purge snapshot");
                }
                if !current_revision() {
                    anyhow::bail!("Route revision changed during purge");
                }
                fs::remove_file(path)?;
                Ok(())
            })();
            result.map_err(|_| anyhow::anyhow!("Archive purge stopped after {removed} confirmed deletions; the purge barrier remains committed"))?;
            removed += 1;
        }
        Ok(removed)
    }
}
fn read_bounded(path: &Path) -> anyhow::Result<Vec<u8>> {
    const LIMIT: usize = 4 * 1024 * 1024;
    let mut bytes = Vec::new();
    fs::File::open(path)?
        .take((LIMIT + 1) as u64)
        .read_to_end(&mut bytes)?;
    if bytes.len() > LIMIT {
        anyhow::bail!("Archive record exceeds read limit");
    }
    Ok(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn local_purge_does_not_expand_its_frozen_uuid_file_set() {
        let root = std::env::temp_dir().join(format!(
            "gateway-local-archive-snapshot-{}",
            uuid::Uuid::new_v4()
        ));
        fs::create_dir(&root).unwrap();
        let bytes = serde_json::to_vec(&serde_json::json!({"schemaVersion":1,"providerId":"p","credentialId":"a","archivedAt":"2026-10-02T00:00:00Z","reason":"permanent_driver_rejection","credential":{"id":"a","api_key":"fixture"}})).unwrap();
        let old = root.join(format!("{}.json", uuid::Uuid::new_v4()));
        fs::write(&old, &bytes).unwrap();
        let plan = LocalArchiveSnapshot::capture(&root, "p").unwrap();
        let new = root.join(format!("{}.json", uuid::Uuid::new_v4()));
        fs::write(&new, &bytes).unwrap();
        assert_eq!(plan.execute(&|| true).unwrap(), 1);
        assert!(!old.exists());
        assert_eq!(fs::read(new).unwrap(), bytes);
        fs::remove_dir_all(root).unwrap();
    }
}
