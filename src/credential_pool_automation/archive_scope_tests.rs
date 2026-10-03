//! A configurable/shared archive directory never makes ordinary JSON deletable.
use super::archive::{
    archive_pruned_credentials_in_directory, archived_credential_count_in_directory,
    purge_credential_archive_directory,
};
use crate::routing::config::ProviderConfigYaml;
use std::{collections::HashSet, fs};
#[test]
fn shared_archive_purge_preserves_other_providers_and_material_json() {
    let root = std::env::temp_dir().join(format!("archive-scope-{}", uuid::Uuid::new_v4()));
    for id in ["provider-a", "provider-b"] {
        let provider: ProviderConfigYaml = serde_json::from_value(serde_json::json!({
            "id":id, "base_url":"https://example.invalid", "credentials":[{"id":"account", "api_key":"fixture"}]
        })).unwrap();
        archive_pruned_credentials_in_directory(
            &provider,
            &HashSet::from(["account".into()]),
            &root,
        )
        .unwrap();
    }
    fs::write(
        root.join("ordinary.json"),
        r#"{"id":"live","api_key":"preserve"}"#,
    )
    .unwrap();
    fs::write(
        root.join("malformed.json"),
        r#"{"schemaVersion":1,"providerId":"provider-a"}"#,
    )
    .unwrap();
    assert_eq!(
        archived_credential_count_in_directory(&root, "provider-a"),
        1
    );
    assert_eq!(
        archived_credential_count_in_directory(&root, "provider-b"),
        1
    );
    assert_eq!(
        purge_credential_archive_directory(&root, "provider-a").unwrap(),
        1
    );
    assert_eq!(
        archived_credential_count_in_directory(&root, "provider-a"),
        0
    );
    assert_eq!(
        archived_credential_count_in_directory(&root, "provider-b"),
        1
    );
    assert!(root.join("ordinary.json").exists());
    assert!(root.join("malformed.json").exists());
    // A storage directory accidentally selected as an archive retains live files.
    assert_eq!(
        purge_credential_archive_directory(&root, "not-an-owner").unwrap(),
        0
    );
    assert!(root.exists());
    fs::remove_dir_all(root).unwrap();
}

#[cfg(unix)]
#[test]
fn unreadable_archive_files_and_directories_are_errors_not_zero_counts() {
    use super::archive::checked_archive_count_in_directory;
    use std::os::unix::fs::PermissionsExt;
    let root = std::env::temp_dir().join(format!("archive-permissions-{}", uuid::Uuid::new_v4()));
    fs::create_dir(&root).unwrap();
    let file = root.join("record.json");
    fs::write(&file, b"{}").unwrap();
    fs::set_permissions(&file, fs::Permissions::from_mode(0o000)).unwrap();
    let open_failed = fs::File::open(&file).is_err();
    let count_failed = checked_archive_count_in_directory(&root, "provider-a").is_err();
    let purge_failed = purge_credential_archive_directory(&root, "provider-a").is_err();
    fs::set_permissions(&file, fs::Permissions::from_mode(0o600)).unwrap();
    fs::set_permissions(&root, fs::Permissions::from_mode(0o000)).unwrap();
    let directory_failed = checked_archive_count_in_directory(&root, "provider-a").is_err();
    fs::set_permissions(&root, fs::Permissions::from_mode(0o700)).unwrap();
    fs::remove_dir_all(&root).unwrap();
    if open_failed {
        assert!(count_failed && purge_failed && directory_failed);
    } else {
        // Privileged runners can bypass Unix mode bits. Exercise the same real
        // reader error path explicitly rather than claiming a permission denial.
        struct DeniedReader;
        impl std::io::Read for DeniedReader {
            fn read(&mut self, _: &mut [u8]) -> std::io::Result<usize> {
                Err(std::io::Error::from(std::io::ErrorKind::PermissionDenied))
            }
        }
        assert!(super::archive::checked_archive_reader(DeniedReader, "provider-a").is_err());
        eprintln!("Privileged runner: used injected PermissionDenied reader, not filesystem mode-bit denial");
    }
}
