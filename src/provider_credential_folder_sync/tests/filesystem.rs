//! Real filesystem contracts for the same operations used by import and export.
mod fixture;
mod limits;

use super::super::filesystem::{
    collect_json_files, delete_stale_files, read_provider_credential_json_with_retry, sha256_hex,
    write_export_file,
};
use fixture::Directory;
use std::collections::HashSet;
use std::fs;

#[tokio::test]
async fn ordinary_material_roundtrips_and_unchanged_export_is_skipped() {
    let directory = Directory::new();
    let bytes = b"{\n  \"fixture\": true\n}";
    let (relative, hash, written) =
        write_export_file(&directory.root, "provider\\nested\\item.JSON", bytes).unwrap();
    assert_eq!(relative, "provider/nested/item.JSON");
    assert_eq!(hash, sha256_hex(bytes));
    assert!(written);
    let file = directory.root.join(&relative);
    assert_eq!(
        collect_json_files(&directory.root).unwrap(),
        vec![file.clone()]
    );
    let (read_hash, payload) = read_provider_credential_json_with_retry(&directory.root, &file)
        .await
        .unwrap();
    assert_eq!(read_hash, hash);
    assert_eq!(payload, serde_json::json!({ "fixture": true }));
    assert!(
        !write_export_file(&directory.root, &relative, bytes)
            .unwrap()
            .2
    );
}

#[test]
fn export_replacement_is_atomic_and_cleans_temporary_material() {
    let directory = Directory::new();
    let relative = "provider/item.json";
    write_export_file(&directory.root, relative, br#"{"version":1}"#).unwrap();
    write_export_file(&directory.root, relative, br#"{"version":2}"#).unwrap();

    assert_eq!(
        fs::read(directory.root.join(relative)).unwrap(),
        br#"{"version":2}"#
    );
    assert_eq!(
        fs::read_dir(directory.root.join("provider"))
            .unwrap()
            .count(),
        1,
        "atomic export must clean temporary material"
    );
}

#[test]
fn export_waits_for_the_root_operation_owner() {
    let directory = Directory::new();
    let operation_lock = super::super::paths::root_operation_lock(&directory.root);
    let guard = operation_lock.lock().unwrap();
    let root = directory.root.clone();
    let (started_tx, started_rx) = std::sync::mpsc::channel();
    let (finished_tx, finished_rx) = std::sync::mpsc::channel();
    let worker = std::thread::spawn(move || {
        started_tx.send(()).unwrap();
        let result = write_export_file(&root, "provider/item.json", b"{}");
        finished_tx.send(result.is_ok()).unwrap();
    });

    started_rx
        .recv_timeout(std::time::Duration::from_secs(1))
        .unwrap();
    assert!(
        finished_rx
            .recv_timeout(std::time::Duration::from_millis(100))
            .is_err(),
        "export must wait for the root operation owner"
    );
    drop(guard);
    assert!(finished_rx
        .recv_timeout(std::time::Duration::from_secs(1))
        .unwrap());
    worker.join().unwrap();
}

#[test]
fn traversal_export_preserves_outside_bytes() {
    for relative in [
        "../outside/victim.json",
        "..\\outside\\victim.json",
        "provider/../../outside/victim.json",
    ] {
        let directory = Directory::new();
        fs::create_dir(directory.root.join("provider")).unwrap();
        let victim = directory.outside.join("victim.json");
        fs::write(&victim, b"outside fixture").unwrap();
        let result = write_export_file(&directory.root, relative, b"{}");
        assert_eq!(fs::read(&victim).unwrap(), b"outside fixture", "{relative}");
        assert!(result.is_err(), "{relative}");
    }
}

#[test]
fn ambiguous_source_paths_are_rejected_before_normalization() {
    for relative in [
        "/provider/item.json",
        "\\provider\\item.json",
        "provider/./item.json",
        "provider /item.json",
        "provider/item.json.",
        "provider/item.json:stream",
        "NUL.json",
        "C:relative.json",
    ] {
        let directory = Directory::new();
        assert!(
            write_export_file(&directory.root, relative, b"{}").is_err(),
            "{relative}"
        );
        assert_eq!(
            fs::read_dir(&directory.root).unwrap().count(),
            0,
            "{relative}"
        );
    }
}

#[cfg(any(unix, windows))]
#[test]
fn discovery_rejects_external_directory_links() {
    let mut directory = Directory::new();
    fs::write(directory.outside.join("victim.json"), b"{}").unwrap();
    directory.link(directory.root.join("linked"), directory.outside.clone());
    assert!(collect_json_files(&directory.root).is_err());
}

#[cfg(any(unix, windows))]
#[test]
fn discovery_rejects_internal_aliases_without_silently_skipping() {
    let mut directory = Directory::new();
    let target = directory.root.join("target");
    fs::create_dir(&target).unwrap();
    fs::write(target.join("item.json"), b"{}").unwrap();
    directory.link(directory.root.join("alias"), target);
    assert!(collect_json_files(&directory.root).is_err());
}

#[cfg(any(unix, windows))]
#[test]
fn linked_export_cannot_overwrite_external_material() {
    let mut directory = Directory::new();
    let victim = directory.outside.join("item.json");
    fs::write(&victim, b"outside fixture").unwrap();
    directory.link(directory.root.join("linked"), directory.outside.clone());
    let result = write_export_file(&directory.root, "linked/item.json", b"{}");
    assert_eq!(fs::read(&victim).unwrap(), b"outside fixture");
    assert!(result.is_err());
}

#[cfg(any(unix, windows))]
#[test]
fn hard_linked_leaf_cannot_overwrite_external_material() {
    let directory = Directory::new();
    let victim = directory.outside.join("item.json");
    let alias = directory.root.join("item.json");
    fs::write(&victim, b"outside fixture").unwrap();
    fs::hard_link(&victim, &alias).unwrap();

    let result = write_export_file(&directory.root, "item.json", b"{}");

    assert_eq!(fs::read(&victim).unwrap(), b"outside fixture");
    assert!(result.is_err());
}

#[cfg(any(unix, windows))]
#[test]
fn linked_export_cannot_create_external_directories() {
    let mut directory = Directory::new();
    directory.link(directory.root.join("linked"), directory.outside.clone());
    let result = write_export_file(&directory.root, "linked/new/item.json", b"{}");
    assert!(!directory.outside.join("new").exists());
    assert!(result.is_err());
}

#[cfg(any(unix, windows))]
#[test]
fn stale_cleanup_rejects_linked_tree_before_any_deletion() {
    let mut directory = Directory::new();
    let victim = directory.outside.join("victim.json");
    let local = directory.root.join("local.json");
    fs::write(&victim, b"outside fixture").unwrap();
    fs::write(&local, b"local fixture").unwrap();
    directory.link(directory.root.join("linked"), directory.outside.clone());
    let mut count = 0;
    let result = delete_stale_files(&directory.root, &HashSet::new(), &mut count);
    assert_eq!(fs::read(&victim).unwrap(), b"outside fixture");
    assert_eq!(fs::read(&local).unwrap(), b"local fixture");
    assert_eq!(count, 0);
    assert!(result.is_err());
}

#[cfg(any(unix, windows))]
#[tokio::test]
async fn read_rechecks_ancestor_replaced_after_discovery() {
    let mut directory = Directory::new();
    let parent = directory.root.join("provider");
    fs::create_dir(&parent).unwrap();
    let file = parent.join("item.json");
    fs::write(&file, b"{}").unwrap();
    assert_eq!(
        collect_json_files(&directory.root).unwrap(),
        vec![file.clone()]
    );
    fs::remove_file(&file).unwrap();
    fs::remove_dir(&parent).unwrap();
    fs::write(directory.outside.join("item.json"), b"{\"outside\": true}").unwrap();
    directory.link(parent, directory.outside.clone());
    assert!(
        read_provider_credential_json_with_retry(&directory.root, &file)
            .await
            .is_err()
    );
}

#[cfg(any(unix, windows))]
#[tokio::test]
async fn configured_root_link_remains_trusted_for_all_operations() {
    let mut directory = Directory::new();
    fs::remove_dir(&directory.root).unwrap();
    directory.link(directory.root.clone(), directory.outside.clone());
    let (relative, _, written) =
        write_export_file(&directory.root, "provider/item.json", b"{}").unwrap();
    assert!(written);
    let files = collect_json_files(&directory.root).unwrap();
    assert_eq!(files.len(), 1);
    assert!(
        read_provider_credential_json_with_retry(&directory.root, &files[0])
            .await
            .is_ok()
    );
    let mut count = 0;
    delete_stale_files(&directory.root, &HashSet::from([relative]), &mut count).unwrap();
    assert_eq!(count, 0);
    delete_stale_files(&directory.root, &HashSet::new(), &mut count).unwrap();
    assert_eq!(count, 1);
    assert!(!directory.outside.join("provider/item.json").exists());
}

#[test]
fn missing_root_is_an_error_not_an_empty_authoritative_listing() {
    let directory = Directory::new();
    fs::remove_dir(&directory.root).unwrap();
    assert!(collect_json_files(&directory.root).is_err());
}

#[test]
fn stale_cleanup_preserves_expected_and_non_json_files() {
    let directory = Directory::new();
    for name in ["keep.json", "stale.JSON", "notes.txt"] {
        fs::write(directory.root.join(name), b"{}").unwrap();
    }
    let mut count = 0;
    delete_stale_files(
        &directory.root,
        &HashSet::from(["keep.json".to_owned()]),
        &mut count,
    )
    .unwrap();
    assert_eq!(count, 1);
    assert!(directory.root.join("keep.json").exists());
    assert!(directory.root.join("notes.txt").exists());
    assert!(!directory.root.join("stale.JSON").exists());
}

#[cfg(unix)]
#[tokio::test]
async fn leaf_symlink_and_dangling_link_are_rejected_by_all_io() {
    let directory = Directory::new();
    for exists in [true, false] {
        let target = directory.outside.join("victim.json");
        if exists {
            fs::write(&target, b"{}").unwrap();
        }
        let link = directory.root.join("item.json");
        std::os::unix::fs::symlink(&target, &link).unwrap();
        assert!(collect_json_files(&directory.root).is_err());
        assert!(
            read_provider_credential_json_with_retry(&directory.root, &link)
                .await
                .is_err()
        );
        assert!(write_export_file(&directory.root, "item.json", b"{}").is_err());
        assert!(delete_stale_files(&directory.root, &HashSet::new(), &mut 0).is_err());
        fs::remove_file(link).unwrap();
        if exists {
            fs::remove_file(target).unwrap();
        }
    }
}
