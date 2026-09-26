//! Boundary tests use real small-limit files/trees and adversarial stream lengths.
use super::fixture::Directory;
use crate::provider_credential_folder_sync::filesystem::{
    collect_json_files_with_limits, delete_stale_files_with_limits, read_bounded,
    read_material_bytes, serialize_export_payload, write_export_file_with_limit,
};
use crate::provider_credential_folder_sync::limits::ScanLimits;
use crate::provider_credential_folder_sync::paths::validated_relative_path;
use std::collections::HashSet;
use std::fs;
use std::io::{Cursor, Read};

fn limits() -> ScanLimits {
    ScanLimits {
        max_depth: 4,
        max_entries: 20,
        max_files: 10,
    }
}

#[test]
fn json_count_accepts_boundary_and_rejects_excess() {
    let directory = Directory::new();
    let budget = ScanLimits {
        max_files: 1,
        ..limits()
    };
    fs::write(directory.root.join("first.json"), b"{}").unwrap();
    assert_eq!(
        collect_json_files_with_limits(&directory.root, budget)
            .unwrap()
            .len(),
        1
    );
    fs::write(directory.root.join("second.json"), b"{}").unwrap();
    let error = collect_json_files_with_limits(&directory.root, budget).unwrap_err();
    assert_eq!(
        error.code.as_deref(),
        Some("provider_credential_folder_sync_scan_limit")
    );
}

#[test]
fn scan_entry_budget_counts_directories_and_non_json_files() {
    let directory = Directory::new();
    let budget = ScanLimits {
        max_entries: 2,
        ..limits()
    };
    fs::create_dir(directory.root.join("empty")).unwrap();
    fs::write(directory.root.join("notes.txt"), b"ignored").unwrap();
    assert!(collect_json_files_with_limits(&directory.root, budget)
        .unwrap()
        .is_empty());
    fs::write(directory.root.join("extra.txt"), b"ignored").unwrap();
    assert!(collect_json_files_with_limits(&directory.root, budget).is_err());
}

#[test]
fn scan_depth_budget_accepts_leaf_level_and_rejects_extra_directory() {
    let directory = Directory::new();
    let budget = ScanLimits {
        max_depth: 1,
        ..limits()
    };
    let child = directory.root.join("child");
    fs::create_dir(&child).unwrap();
    fs::write(child.join("item.json"), b"{}").unwrap();
    assert_eq!(
        collect_json_files_with_limits(&directory.root, budget)
            .unwrap()
            .len(),
        1
    );
    fs::create_dir(child.join("too-deep")).unwrap();
    assert!(collect_json_files_with_limits(&directory.root, budget).is_err());
}

#[test]
fn scan_limit_aborts_stale_cleanup_without_partial_deletion() {
    let directory = Directory::new();
    let victim = directory.root.join("local.json");
    fs::write(&victim, b"local fixture").unwrap();
    fs::create_dir(directory.root.join("child")).unwrap();
    let mut count = 0;
    let result = delete_stale_files_with_limits(
        &directory.root,
        &HashSet::new(),
        &mut count,
        ScanLimits {
            max_depth: 0,
            ..limits()
        },
    );
    assert!(victim.exists());
    assert_eq!(fs::read(victim).unwrap(), b"local fixture");
    assert_eq!(count, 0);
    assert!(result.is_err());
}

#[test]
fn bounded_reader_accepts_exact_limit_and_rejects_one_extra_byte() {
    assert_eq!(
        read_bounded(Cursor::new(b"1234"), None, 4).unwrap(),
        b"1234"
    );
    let error = read_bounded(Cursor::new(b"12345"), None, 4).unwrap_err();
    assert_eq!(
        error.code.as_deref(),
        Some("provider_credential_folder_sync_material_too_large")
    );
}

struct NeverRead;
impl Read for NeverRead {
    fn read(&mut self, _: &mut [u8]) -> std::io::Result<usize> {
        panic!("known excessive length must be rejected before reading");
    }
}

#[test]
fn oversized_metadata_is_rejected_before_io() {
    let error = read_bounded(NeverRead, Some(5), 4).unwrap_err();
    assert_eq!(
        error.code.as_deref(),
        Some("provider_credential_folder_sync_material_too_large")
    );
}

#[test]
fn actual_stream_length_overrides_stale_short_metadata() {
    assert!(read_bounded(Cursor::new(b"12345"), Some(1), 4).is_err());
}

#[test]
fn file_read_enforces_material_byte_limit() {
    let directory = Directory::new();
    let file = directory.root.join("item.json");
    fs::write(&file, b"{\"fixture\":true}").unwrap();
    let size = fs::metadata(&file).unwrap().len() as usize;
    assert_eq!(
        read_material_bytes(&file, size).unwrap(),
        b"{\"fixture\":true}"
    );
    assert!(read_material_bytes(&file, size - 1).is_err());
}

#[test]
fn export_size_rejection_happens_before_creating_directories() {
    let directory = Directory::new();
    let result =
        write_export_file_with_limit(&directory.root, "new/item.json", b"{\"fixture\":true}", 2);
    assert!(!directory.root.join("new").exists());
    assert!(result.is_err());
}

#[test]
fn oversized_existing_export_is_not_overwritten() {
    let directory = Directory::new();
    let file = directory.root.join("item.json");
    fs::write(&file, b"existing material").unwrap();
    let result = write_export_file_with_limit(&directory.root, "item.json", b"{}", 2);
    assert_eq!(fs::read(file).unwrap(), b"existing material");
    assert!(result.is_err());
}

#[test]
fn bounded_export_serializer_preserves_pretty_bytes_at_boundary() {
    let payload = serde_json::json!({ "escaped": "a\nb", "items": [1, 2, 3] });
    let expected = serde_json::to_vec_pretty(&payload).unwrap();
    assert_eq!(
        serialize_export_payload(&payload, expected.len()).unwrap(),
        expected
    );
    let error = serialize_export_payload(&payload, expected.len() - 1).unwrap_err();
    assert_eq!(
        error.code.as_deref(),
        Some("provider_credential_folder_sync_material_too_large")
    );
}

#[test]
fn source_path_limit_matches_database_unicode_character_capacity() {
    let segment = "\u{e9}".repeat(127);
    let relative = format!("a{segment}/{segment}/{segment}/{segment}");
    assert_eq!(relative.chars().count(), 512);
    assert!(validated_relative_path(&relative).unwrap().is_some());
    assert!(validated_relative_path(&(relative + "a")).is_err());
}

struct CountingReader {
    consumed: usize,
}
impl Read for CountingReader {
    fn read(&mut self, output: &mut [u8]) -> std::io::Result<usize> {
        let count = output.len().min(128 - self.consumed);
        output[..count].fill(b'x');
        self.consumed += count;
        Ok(count)
    }
}

#[test]
fn bounded_reader_stops_after_limit_plus_sentinel_byte() {
    let mut reader = CountingReader { consumed: 0 };
    let result = read_bounded(&mut reader, None, 4);
    assert!(result.is_err());
    assert_eq!(reader.consumed, 5);
}
