//! Durable journal append, bounded framing and incomplete-tail repair share one I/O owner.

use super::{
    JournalAppendBackend, JournalEntry, PersistenceError, TransactionJournal, TransactionRecord,
    WriterLockGuard,
};
use std::fs::OpenOptions;
use std::io::{self, BufRead, BufReader, Write};
use std::path::Path;

#[derive(Debug, Default)]
pub(super) struct FileJournalAppendBackend;

impl JournalAppendBackend for FileJournalAppendBackend {
    fn append_and_sync(&self, path: &Path, bytes: &[u8]) -> io::Result<()> {
        let mut file = OpenOptions::new().create(true).append(true).open(path)?;
        file.write_all(bytes)?;
        file.flush()?;
        file.sync_all()
    }
}

impl TransactionJournal {
    pub(super) fn serialize_entry(
        &self,
        record: &TransactionRecord,
    ) -> Result<Vec<u8>, PersistenceError> {
        let entry = JournalEntry::from_record(record);
        entry.validate()?;
        let mut bytes = serde_json::to_vec(&entry)
            .map_err(|error| PersistenceError::io("serializing journal entry", error))?;
        bytes.push(b'\n');
        if bytes.len() > self.max_line_bytes {
            return Err(PersistenceError::corruption(
                "Serialized journal record exceeds the configured line limit",
            ));
        }
        Ok(bytes)
    }

    pub(super) fn append_bytes_locked(
        &self,
        guard: &WriterLockGuard,
        bytes: &[u8],
    ) -> Result<(), PersistenceError> {
        self.persistence.ensure_guard(guard)?;
        let path = self.persistence.journal_path();
        self.persistence.validate_optional_managed_file(&path)?;
        self.append_backend
            .append_and_sync(&path, bytes)
            .map_err(|error| PersistenceError::io("syncing journal", error))?;
        Ok(())
    }

    pub(super) fn load_entries_inner(
        &self,
        repair_tail: bool,
    ) -> Result<Vec<JournalEntry>, PersistenceError> {
        let (entries, tail_offset, file_len, missing_final_lf) = self.scan_journal()?;
        if repair_tail && tail_offset < file_len {
            let file = OpenOptions::new()
                .write(true)
                .open(self.persistence.journal_path())
                .map_err(|error| PersistenceError::io("opening journal for tail repair", error))?;
            file.set_len(tail_offset)
                .and_then(|_| file.sync_all())
                .map_err(|error| PersistenceError::io("truncating journal tail", error))?;
        } else if repair_tail && missing_final_lf {
            let mut file = OpenOptions::new()
                .append(true)
                .open(self.persistence.journal_path())
                .map_err(|error| PersistenceError::io("opening journal for LF repair", error))?;
            file.write_all(b"\n")
                .and_then(|_| file.flush())
                .and_then(|_| file.sync_all())
                .map_err(|error| PersistenceError::io("repairing journal LF terminator", error))?;
        }
        Ok(entries)
    }

    fn scan_journal(&self) -> Result<(Vec<JournalEntry>, u64, u64, bool), PersistenceError> {
        let path = self.persistence.journal_path();
        self.persistence.validate_optional_managed_file(&path)?;
        let file = match OpenOptions::new().read(true).open(&path) {
            Ok(file) => file,
            Err(error) if error.kind() == io::ErrorKind::NotFound => {
                return Ok((Vec::new(), 0, 0, false));
            }
            Err(error) => return Err(PersistenceError::io("opening journal", error)),
        };
        let file_len = file
            .metadata()
            .map_err(|error| PersistenceError::io("reading journal length", error))?
            .len();
        let mut reader = BufReader::new(file);
        let mut offset = 0u64;
        let mut last_good_offset = 0u64;
        let mut entries = Vec::new();
        let mut missing_final_lf = false;
        loop {
            let Some(mut line) = self.read_bounded_line(&mut reader)? else {
                break;
            };
            offset += line.len() as u64;
            let terminated = line.last() == Some(&b'\n');
            if terminated {
                line.pop();
                if line.last() == Some(&b'\r') {
                    return Err(PersistenceError::corruption(
                        "Journal records must use LF-only line endings",
                    ));
                }
                let entry: JournalEntry = serde_json::from_slice(&line).map_err(|_| {
                    PersistenceError::corruption(
                        "Malformed newline-terminated console journal record",
                    )
                })?;
                entry.validate()?;
                entries.push(entry);
                if entries.len() > self.max_records {
                    return Err(PersistenceError::corruption(
                        "Journal record count exceeds the configured limit",
                    ));
                }
                last_good_offset = offset;
            } else {
                if line.last() == Some(&b'\r') {
                    return Err(PersistenceError::corruption(
                        "Journal records must use LF-only line endings",
                    ));
                }
                match serde_json::from_slice::<JournalEntry>(&line) {
                    Ok(entry) => {
                        entry.validate()?;
                        entries.push(entry);
                        if entries.len() > self.max_records {
                            return Err(PersistenceError::corruption(
                                "Journal record count exceeds the configured limit",
                            ));
                        }
                        last_good_offset = offset;
                        missing_final_lf = true;
                    }
                    Err(error) if error.is_eof() => {
                        break;
                    }
                    Err(_) => {
                        return Err(PersistenceError::corruption(
                            "Malformed incomplete console journal tail",
                        ));
                    }
                }
            }
        }
        Ok((entries, last_good_offset, file_len, missing_final_lf))
    }

    fn read_bounded_line<R: BufRead>(
        &self,
        reader: &mut R,
    ) -> Result<Option<Vec<u8>>, PersistenceError> {
        let mut line = Vec::with_capacity(self.max_line_bytes.min(8 * 1024));
        loop {
            let available = reader
                .fill_buf()
                .map_err(|error| PersistenceError::io("reading journal", error))?;
            if available.is_empty() {
                return Ok((!line.is_empty()).then_some(line));
            }
            let newline = available.iter().position(|byte| *byte == b'\n');
            let take = newline.map_or(available.len(), |index| index + 1);
            if line.len().saturating_add(take) > self.max_line_bytes {
                return Err(PersistenceError::corruption(
                    "Journal record exceeds the configured line limit",
                ));
            }
            line.extend_from_slice(&available[..take]);
            reader.consume(take);
            if newline.is_some() {
                return Ok(Some(line));
            }
        }
    }
}
