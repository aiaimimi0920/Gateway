//! Shared normalized source-path keys for one import snapshot.

use super::layout::normalize_source_path_key;
use super::limits::{source_path_retention_limit, SOURCE_PATH_AGGREGATE_RETAINED_BYTES};
use crate::db;
use std::collections::{HashMap, HashSet};
use std::sync::Arc;

pub(in crate::provider_credential_folder_sync) type SourcePathKey = Arc<str>;
pub(in crate::provider_credential_folder_sync) type SourcePathSet = HashSet<SourcePathKey>;

const PATH_KEY_OVERHEAD_BYTES: usize = 384;

pub(in crate::provider_credential_folder_sync) struct SourcePathInterner {
    keys: SourcePathSet,
    retained_bytes: usize,
    max_retained_bytes: usize,
}

pub(in crate::provider_credential_folder_sync) struct IndexedCredential {
    pub(in crate::provider_credential_folder_sync) credential: db::ProviderCredentialImportMetadata,
    pub(in crate::provider_credential_folder_sync) source_path: Option<SourcePathKey>,
}

pub(in crate::provider_credential_folder_sync) struct CredentialPathIndex {
    entries: Vec<IndexedCredential>,
    by_path: HashMap<SourcePathKey, usize>,
}

impl SourcePathInterner {
    pub(in crate::provider_credential_folder_sync) fn new() -> Self {
        Self::with_limit(SOURCE_PATH_AGGREGATE_RETAINED_BYTES)
    }

    fn with_limit(max_retained_bytes: usize) -> Self {
        Self {
            keys: SourcePathSet::new(),
            retained_bytes: 0,
            max_retained_bytes,
        }
    }

    pub(in crate::provider_credential_folder_sync) fn intern_source_path(
        &mut self,
        source_path: &str,
    ) -> Result<Option<SourcePathKey>, crate::error::GatewayError> {
        normalize_source_path_key(source_path)
            .map(|path| self.intern_normalized(path))
            .transpose()
    }

    pub(in crate::provider_credential_folder_sync) fn intern_normalized(
        &mut self,
        path: String,
    ) -> Result<SourcePathKey, crate::error::GatewayError> {
        if let Some(existing) = self.keys.get(path.as_str()) {
            return Ok(existing.clone());
        }
        let retained_bytes = path.capacity().saturating_add(PATH_KEY_OVERHEAD_BYTES);
        if self
            .retained_bytes
            .checked_add(retained_bytes)
            .is_none_or(|bytes| bytes > self.max_retained_bytes)
        {
            return Err(source_path_retention_limit(self.max_retained_bytes));
        }
        let path = SourcePathKey::from(path);
        self.keys.insert(path.clone());
        self.retained_bytes = self.retained_bytes.saturating_add(retained_bytes);
        Ok(path)
    }
}

impl CredentialPathIndex {
    pub(in crate::provider_credential_folder_sync) fn new(
        credentials: Vec<db::ProviderCredentialImportMetadata>,
        interner: &mut SourcePathInterner,
    ) -> Result<Self, crate::error::GatewayError> {
        let mut by_path = HashMap::new();
        let mut entries = Vec::with_capacity(credentials.len());
        for (index, credential) in credentials.into_iter().enumerate() {
            let source_path = match credential.source_path.as_deref() {
                Some(path) => interner.intern_source_path(path)?,
                None => None,
            };
            if let Some(source_path) = &source_path {
                by_path.insert(source_path.clone(), index);
            }
            // The shared key is the deletion/index source of truth; release the row's
            // duplicate allocation before retaining the snapshot for later phases.
            let mut credential = credential;
            credential.source_path = None;
            entries.push(IndexedCredential {
                credential,
                source_path,
            });
        }
        Ok(Self { entries, by_path })
    }

    pub(in crate::provider_credential_folder_sync) fn get(
        &self,
        path: &str,
    ) -> Option<&db::ProviderCredentialImportMetadata> {
        self.by_path
            .get(path)
            .and_then(|index| self.entries.get(*index))
            .map(|entry| &entry.credential)
    }

    pub(in crate::provider_credential_folder_sync) fn iter(
        &self,
    ) -> impl Iterator<Item = &IndexedCredential> {
        self.entries.iter()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn credential(id: &str, path: Option<&str>) -> db::ProviderCredentialImportMetadata {
        db::ProviderCredentialImportMetadata {
            id: id.to_string(),
            label: id.to_string(),
            status: "active".to_string(),
            source_kind: "folder_sync_import".to_string(),
            source_path: path.map(str::to_string),
            source_hash: None,
            sync_mode: "folder_sync".to_string(),
            sync_state: "imported".to_string(),
            archived_at: None,
        }
    }

    #[test]
    fn interner_reuses_equal_normalized_paths() {
        let mut interner = SourcePathInterner::new();
        let first = interner
            .intern_source_path(r"qwen\fixture.json")
            .unwrap()
            .unwrap();
        let second = interner
            .intern_source_path("qwen/fixture.json")
            .unwrap()
            .unwrap();
        assert!(Arc::ptr_eq(&first, &second));
    }

    #[test]
    fn index_preserves_created_order_and_last_duplicate_wins() {
        let credentials = vec![
            credential("older", Some("qwen\\fixture.json")),
            credential("newer", Some("qwen/fixture.json")),
        ];
        let mut interner = SourcePathInterner::new();
        let index = CredentialPathIndex::new(credentials, &mut interner).unwrap();
        assert_eq!(index.get("qwen/fixture.json").unwrap().id, "newer");
        assert_eq!(
            index
                .iter()
                .map(|item| item.credential.id.as_str())
                .collect::<Vec<_>>(),
            ["older", "newer"]
        );
    }

    #[test]
    fn duplicate_paths_do_not_consume_the_budget_twice() {
        let path = "qwen/fixture.json".to_string();
        let mut interner =
            SourcePathInterner::with_limit(path.capacity().saturating_add(PATH_KEY_OVERHEAD_BYTES));
        interner.intern_normalized(path.clone()).unwrap();
        let retained_bytes = interner.retained_bytes;
        interner.intern_normalized(path).unwrap();
        assert_eq!(interner.retained_bytes, retained_bytes);
    }

    #[test]
    fn aggregate_path_budget_rejects_without_retaining_a_partial_key() {
        let path = "qwen/fixture.json".to_string();
        let mut interner = SourcePathInterner::with_limit(path.capacity());
        let error = interner
            .intern_normalized(path)
            .expect_err("path overhead must be charged");
        assert_eq!(
            error.code.as_deref(),
            Some("provider_credential_folder_sync_path_retention_limit")
        );
        assert!(interner.keys.is_empty());
        assert_eq!(interner.retained_bytes, 0);
    }

    #[test]
    fn index_releases_duplicate_metadata_path_storage_after_interning() {
        let credentials = vec![credential("one", Some("qwen/fixture.json"))];
        let mut interner = SourcePathInterner::new();
        let index = CredentialPathIndex::new(credentials, &mut interner).unwrap();
        let entry = index.iter().next().expect("indexed credential");
        assert!(entry.credential.source_path.is_none());
        assert_eq!(entry.source_path.as_deref(), Some("qwen/fixture.json"));
    }
}
