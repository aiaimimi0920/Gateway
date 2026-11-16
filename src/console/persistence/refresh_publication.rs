//! Per-revision, per-transaction candidates survive same-revision retries without mutable selectors.
use super::file_io::read_json_limited;
use super::{refresh_intent::RefreshIntent, *};
use crate::console::refresh_grants::{invalid, GrantLocator};

#[derive(Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct RefreshPublication {
    version: u32,
    pub(crate) intent: RefreshIntent,
    pub(crate) grant: GrantLocator,
}

impl RefreshPublication {
    pub(crate) fn new(intent: RefreshIntent, grant: GrantLocator) -> Self {
        Self {
            version: 1,
            intent,
            grant,
        }
    }

    fn validate(&self, revision: &str, tx: &str) -> Result<(), PersistenceError> {
        let record = &self.intent.prepared;
        if self.version != 1
            || record.new_revision() != revision
            || record.tx_id() != tx
            || *record != record.prepared_identity()?
        {
            return Err(invalid());
        }
        self.grant.validate()
    }
}

impl RouteConfigPersistence {
    fn publication_dir(&self, revision: &str) -> Result<PathBuf, PersistenceError> {
        super::validation::validate_revision_id(revision)?;
        let path = self
            .state_root()
            .join("refresh-publications")
            .join(revision);
        super::paths::reject_existing_links(&path)?;
        Ok(path)
    }

    pub(crate) fn store_refresh_publication_locked(
        &self,
        guard: &WriterLockGuard,
        publication: &RefreshPublication,
    ) -> Result<(), PersistenceError> {
        self.ensure_guard(guard)?;
        self.ensure_writable()?;
        let prepared = &publication.intent.prepared;
        publication.validate(prepared.new_revision(), prepared.tx_id())?;
        self.transaction_path(prepared.tx_id())?;
        let directory = self.publication_dir(prepared.new_revision())?;
        let path = directory.join(format!("{}.json", prepared.tx_id()));
        self.validate_optional_managed_file(&path)?;
        if path
            .try_exists()
            .map_err(|e| PersistenceError::io("checking refresh publication", e))?
        {
            let existing: RefreshPublication = read_json_limited(&path)?;
            if existing != *publication {
                return Err(invalid());
            }
            return Ok(());
        }
        super::paths::ensure_secure_directory(&directory)?;
        self.atomic_write_json_locked(guard, &path, publication)
    }

    pub(crate) fn load_refresh_publications_locked(
        &self,
        guard: &WriterLockGuard,
        revision: &str,
    ) -> Result<Vec<RefreshPublication>, PersistenceError> {
        self.ensure_guard(guard)?;
        let directory = self.publication_dir(revision)?;
        if !directory
            .try_exists()
            .map_err(|e| PersistenceError::io("checking refresh publications", e))?
        {
            return Ok(Vec::new());
        }
        let mut publications = Vec::new();
        for entry in fs::read_dir(&directory)
            .map_err(|e| PersistenceError::io("listing refresh publications", e))?
        {
            // Bound externally persisted candidates; never scan unrelated historical revisions.
            if publications.len() == 128 {
                return Err(invalid());
            }
            let path = entry
                .map_err(|e| PersistenceError::io("reading refresh publication entry", e))?
                .path();
            let tx = path
                .file_stem()
                .and_then(|name| name.to_str())
                .ok_or_else(invalid)?;
            self.transaction_path(tx)?;
            if path.extension().and_then(|name| name.to_str()) != Some("json") {
                return Err(invalid());
            }
            self.validate_optional_managed_file(&path)?;
            let publication: RefreshPublication = read_json_limited(&path)?;
            publication.validate(revision, tx)?;
            publications.push(publication);
        }
        Ok(publications)
    }
}
