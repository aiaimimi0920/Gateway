//! Immutable revision archives own publication, replay, listing and integrity verification.

use super::file_io::{create_new_bytes, read_regular_file, sha256_hex, sync_directory};
use super::paths::reject_link_metadata;
use super::validation::{
    normalized_relative_string, validate_archive_relative_path, validate_canonical_yaml,
    validate_revision_id,
};
use super::{
    PersistenceError, RevisionArchive, RouteConfigPersistence, StoredRouteRevision,
    TransactionRecord, WriterLockGuard,
};
use crate::console::document::CanonicalRouteDocument;
use crate::console::revision::RevisionMetadata;
use crate::routing::config::RouteConfigYaml;
use std::path::{Path, PathBuf};
use std::{fs, io};

impl RevisionArchive {
    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn relative_path(&self) -> &Path {
        &self.relative_path
    }
}

impl StoredRouteRevision {
    pub fn archive(&self) -> &RevisionArchive {
        &self.archive
    }

    pub fn metadata(&self) -> &RevisionMetadata {
        &self.metadata
    }

    pub fn document(&self) -> &RouteConfigYaml {
        &self.document
    }
}

impl RouteConfigPersistence {
    pub fn archive_revision(
        &self,
        metadata: &RevisionMetadata,
        document: &CanonicalRouteDocument,
    ) -> Result<RevisionArchive, PersistenceError> {
        self.ensure_writable()?;
        let guard = self.try_writer_lock()?;
        self.archive_revision_locked(&guard, metadata, document)
    }

    pub fn load_revision(
        &self,
        revision_id: &str,
    ) -> Result<Option<StoredRouteRevision>, PersistenceError> {
        validate_revision_id(revision_id)?;
        let relative_path = PathBuf::from("revisions").join(revision_id);
        validate_archive_relative_path(&normalized_relative_string(&relative_path))?;
        let path = self.state_root.join(&relative_path);
        match fs::symlink_metadata(&path) {
            Ok(metadata) => {
                reject_link_metadata(&path, &metadata)?;
                if !metadata.is_dir() {
                    return Err(PersistenceError::corruption(
                        "Revision archive path is not a directory",
                    ));
                }
                self.read_revision_archive(RevisionArchive {
                    path,
                    relative_path,
                })
                .map(Some)
            }
            Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(None),
            Err(error) => Err(PersistenceError::io(
                "reading revision archive metadata",
                error,
            )),
        }
    }

    pub fn load_revisions(&self) -> Result<Vec<StoredRouteRevision>, PersistenceError> {
        let revisions_dir = self.revisions_dir();
        let mut archives = Vec::new();
        for entry in fs::read_dir(&revisions_dir)
            .map_err(|error| PersistenceError::io("listing revision archives", error))?
        {
            let entry = entry.map_err(|error| {
                PersistenceError::io("reading revision archive directory", error)
            })?;
            let path = entry.path();
            let metadata = fs::symlink_metadata(&path).map_err(|error| {
                PersistenceError::io("reading revision archive metadata", error)
            })?;
            reject_link_metadata(&path, &metadata)?;
            if !metadata.is_dir() {
                return Err(PersistenceError::corruption(
                    "Revision archives directory contains a non-directory entry",
                ));
            }
            let relative_path = PathBuf::from("revisions").join(entry.file_name());
            validate_archive_relative_path(&normalized_relative_string(&relative_path))?;
            archives.push(self.read_revision_archive(RevisionArchive {
                path,
                relative_path,
            })?);
        }
        archives.sort_by(|left, right| {
            right
                .metadata()
                .sequence()
                .cmp(&left.metadata().sequence())
                .then_with(|| right.metadata().id().cmp(left.metadata().id()))
        });
        Ok(archives)
    }

    pub(crate) fn verify_transaction_archive(
        &self,
        record: &TransactionRecord,
    ) -> Result<(), PersistenceError> {
        record.validate()?;
        let relative = Path::new(record.archive_relative_path());
        validate_archive_relative_path(record.archive_relative_path())?;
        let archive = self.state_root.join(relative);
        let metadata = fs::symlink_metadata(&archive).map_err(|error| {
            PersistenceError::corruption(format!(
                "Transaction revision archive is unavailable: {error}"
            ))
        })?;
        reject_link_metadata(&archive, &metadata)?;
        if !metadata.is_dir() {
            return Err(PersistenceError::corruption(
                "Transaction revision archive is not a directory",
            ));
        }
        let expected_files = vec![
            "document.json".to_string(),
            "metadata.json".to_string(),
            "routes.yaml".to_string(),
        ];
        let mut actual_files = Vec::new();
        for entry in fs::read_dir(&archive)
            .map_err(|error| PersistenceError::io("listing revision archive", error))?
        {
            let entry = entry
                .map_err(|error| PersistenceError::io("reading revision archive entry", error))?;
            let path = entry.path();
            let metadata = fs::symlink_metadata(&path)
                .map_err(|error| PersistenceError::io("checking revision archive entry", error))?;
            reject_link_metadata(&path, &metadata)?;
            if !metadata.is_file() {
                return Err(PersistenceError::corruption(
                    "Revision archive contains a non-file entry",
                ));
            }
            actual_files.push(entry.file_name().to_string_lossy().into_owned());
        }
        actual_files.sort();
        if actual_files != expected_files {
            return Err(PersistenceError::corruption(
                "Revision archive contains missing or unexpected files",
            ));
        }
        let document = read_regular_file(&archive.join("document.json"))?;
        let yaml = read_regular_file(&archive.join("routes.yaml"))?;
        let metadata_bytes = read_regular_file(&archive.join("metadata.json"))?;
        let metadata: RevisionMetadata = serde_json::from_slice(&metadata_bytes)
            .map_err(|_| PersistenceError::corruption("Revision metadata JSON is malformed"))?;
        let canonical_metadata = serde_json::to_vec(&metadata)
            .map_err(|error| PersistenceError::io("serializing revision metadata", error))?;
        if metadata.id() != record.new_revision()
            || metadata.parent() != record.old_revision()
            || metadata.yaml_digest() != record.new_yaml_digest()
            || sha256_hex(&yaml) != record.new_yaml_digest()
            || sha256_hex(&document) != metadata.document_digest()
            || metadata_bytes != canonical_metadata
        {
            return Err(PersistenceError::corruption(
                "Transaction revision archive digest or identity mismatch",
            ));
        }
        Ok(())
    }

    fn read_revision_archive(
        &self,
        archive: RevisionArchive,
    ) -> Result<StoredRouteRevision, PersistenceError> {
        let metadata = fs::symlink_metadata(archive.path())
            .map_err(|error| PersistenceError::io("reading revision archive metadata", error))?;
        reject_link_metadata(archive.path(), &metadata)?;
        if !metadata.is_dir() {
            return Err(PersistenceError::corruption(
                "Revision archive path is not a directory",
            ));
        }
        let expected_files = vec![
            "document.json".to_string(),
            "metadata.json".to_string(),
            "routes.yaml".to_string(),
        ];
        let mut actual_files = Vec::new();
        for entry in fs::read_dir(archive.path())
            .map_err(|error| PersistenceError::io("listing revision archive", error))?
        {
            let entry = entry
                .map_err(|error| PersistenceError::io("reading revision archive entry", error))?;
            let path = entry.path();
            let metadata = fs::symlink_metadata(&path)
                .map_err(|error| PersistenceError::io("checking revision archive entry", error))?;
            reject_link_metadata(&path, &metadata)?;
            if !metadata.is_file() {
                return Err(PersistenceError::corruption(
                    "Revision archive contains a non-file entry",
                ));
            }
            actual_files.push(entry.file_name().to_string_lossy().into_owned());
        }
        actual_files.sort();
        if actual_files != expected_files {
            return Err(PersistenceError::corruption(
                "Revision archive contains missing or unexpected files",
            ));
        }

        let document_bytes = read_regular_file(&archive.path().join("document.json"))?;
        let yaml_bytes = read_regular_file(&archive.path().join("routes.yaml"))?;
        let metadata_bytes = read_regular_file(&archive.path().join("metadata.json"))?;
        let metadata: RevisionMetadata = serde_json::from_slice(&metadata_bytes)
            .map_err(|_| PersistenceError::corruption("Revision metadata JSON is malformed"))?;
        metadata
            .validate()
            .map_err(|error| PersistenceError::corruption(error.to_string()))?;

        let expected_relative = PathBuf::from("revisions").join(metadata.id());
        if archive.relative_path() != expected_relative.as_path() {
            return Err(PersistenceError::corruption(
                "Revision archive path does not match its metadata identity",
            ));
        }
        if sha256_hex(&document_bytes) != metadata.document_digest()
            || sha256_hex(&yaml_bytes) != metadata.yaml_digest()
        {
            return Err(PersistenceError::corruption(
                "Revision archive digest mismatch",
            ));
        }
        let canonical_metadata = serde_json::to_vec(&metadata)
            .map_err(|error| PersistenceError::io("serializing revision metadata", error))?;
        if metadata_bytes != canonical_metadata {
            return Err(PersistenceError::corruption(
                "Revision metadata JSON is not in canonical form",
            ));
        }

        let document: RouteConfigYaml = serde_json::from_slice(&document_bytes)
            .map_err(|_| PersistenceError::corruption("Revision document JSON is malformed"))?;

        Ok(StoredRouteRevision {
            archive,
            metadata,
            document,
        })
    }

    pub(crate) fn archive_revision_locked(
        &self,
        guard: &WriterLockGuard,
        metadata: &RevisionMetadata,
        document: &CanonicalRouteDocument,
    ) -> Result<RevisionArchive, PersistenceError> {
        self.ensure_guard(guard)?;
        self.ensure_writable()?;
        metadata
            .validate()
            .map_err(|error| PersistenceError::corruption(error.to_string()))?;
        validate_canonical_yaml(document.canonical_yaml())?;
        if metadata.document_digest() != document.document_digest()
            || metadata.yaml_digest() != document.yaml_digest()
        {
            return Err(PersistenceError::corruption(
                "Revision metadata digests do not match the canonical document",
            ));
        }
        validate_revision_id(metadata.id())?;

        let relative_path = PathBuf::from("revisions").join(metadata.id());
        validate_archive_relative_path(&normalized_relative_string(&relative_path))?;
        let archive_path = self.state_root.join(&relative_path);
        let metadata_bytes = serde_json::to_vec(metadata)
            .map_err(|error| PersistenceError::io("serializing revision metadata", error))?;
        let expected = [
            ("document.json", document.canonical_json()),
            ("routes.yaml", document.canonical_yaml()),
            ("metadata.json", metadata_bytes.as_slice()),
        ];

        match fs::create_dir(&archive_path) {
            Ok(()) => {
                for (leaf, bytes) in expected {
                    create_new_bytes(&archive_path.join(leaf), bytes)?;
                }
                sync_directory(&archive_path)?;
                sync_directory(self.revisions_dir().as_path())?;
            }
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {
                let metadata = fs::symlink_metadata(&archive_path)
                    .map_err(|error| PersistenceError::io("reading revision archive", error))?;
                reject_link_metadata(&archive_path, &metadata)?;
                if !metadata.is_dir() {
                    return Err(PersistenceError::corruption(
                        "Revision archive path is not a directory",
                    ));
                }
                let mut actual_files = Vec::new();
                for entry in fs::read_dir(&archive_path)
                    .map_err(|error| PersistenceError::io("listing revision archive", error))?
                {
                    let entry = entry.map_err(|error| {
                        PersistenceError::io("reading revision archive entry", error)
                    })?;
                    let path = entry.path();
                    let metadata = fs::symlink_metadata(&path).map_err(|error| {
                        PersistenceError::io("checking revision archive entry", error)
                    })?;
                    reject_link_metadata(&path, &metadata)?;
                    if !metadata.is_file() {
                        return Err(PersistenceError::corruption(
                            "Existing revision archive contains a non-file entry",
                        ));
                    }
                    actual_files.push(entry.file_name().to_string_lossy().into_owned());
                }
                actual_files.sort();
                if actual_files
                    != [
                        "document.json".to_string(),
                        "metadata.json".to_string(),
                        "routes.yaml".to_string(),
                    ]
                {
                    return Err(PersistenceError::corruption(
                        "Existing revision archive contains missing or unexpected entries",
                    ));
                }
                for (leaf, bytes) in expected {
                    let existing = read_regular_file(&archive_path.join(leaf)).map_err(|_| {
                        PersistenceError::corruption(
                            "Existing revision archive is incomplete or unreadable",
                        )
                    })?;
                    if existing != bytes {
                        return Err(PersistenceError::corruption(
                            "Existing revision archive differs from the immutable revision",
                        ));
                    }
                }
            }
            Err(error) => {
                return Err(PersistenceError::io("creating revision archive", error));
            }
        }

        Ok(RevisionArchive {
            path: archive_path,
            relative_path,
        })
    }
}
