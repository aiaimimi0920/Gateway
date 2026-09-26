#[path = "pipeline_send_runtime/config.rs"]
mod config;
#[path = "provider_credential_folder_sync_deletion/fixture.rs"]
mod fixture;
mod support;

use std::fs;

use fixture::Directory;
use neuro_gateway::provider_credential_folder_sync::delete_synced_credential_file;

#[tokio::test]
async fn parent_traversal_cannot_delete_outside_file() {
    let directory = Directory::new();
    let state = directory.state(true);
    let victim = directory.outside.join("victim.json");
    fs::write(&victim, b"outside credential").unwrap();
    let result = delete_synced_credential_file(&state, "../outside/victim.json");
    assert!(victim.exists(), "parent traversal deleted the outside file");
    assert!(result.is_err(), "parent traversal must be rejected");
}

#[tokio::test]
async fn backslash_traversal_cannot_delete_outside_file() {
    let directory = Directory::new();
    let state = directory.state(true);
    let victim = directory.outside.join("victim.json");
    fs::write(&victim, b"outside credential").unwrap();
    let result = delete_synced_credential_file(&state, "..\\outside\\victim.json");
    assert!(
        victim.exists(),
        "backslash traversal deleted the outside file"
    );
    assert!(result.is_err(), "backslash traversal must be rejected");
}

#[tokio::test]
async fn nested_traversal_cannot_delete_outside_file() {
    let directory = Directory::new();
    let state = directory.state(true);
    fs::create_dir(directory.root.join("provider")).unwrap();
    let victim = directory.outside.join("victim.json");
    fs::write(&victim, b"outside credential").unwrap();
    let result = delete_synced_credential_file(&state, "provider/../../outside/victim.json");
    assert!(victim.exists(), "nested traversal deleted the outside file");
    assert!(result.is_err(), "nested traversal must be rejected");
}

#[cfg(any(unix, windows))]
#[tokio::test]
async fn directory_link_cannot_delete_outside_file() {
    let mut directory = Directory::new();
    let state = directory.state(true);
    let victim = directory.outside.join("victim.json");
    fs::write(&victim, b"outside credential").unwrap();
    directory.directory_link(directory.outside.clone());
    let result = delete_synced_credential_file(&state, "linked/victim.json");
    assert!(victim.exists(), "directory link deleted the outside file");
    assert!(result.is_err(), "linked directory must be rejected");
}

#[cfg(any(unix, windows))]
#[tokio::test]
async fn in_root_directory_link_is_rejected_too() {
    let mut directory = Directory::new();
    let state = directory.state(true);
    let target = directory.root.join("provider");
    fs::create_dir(&target).unwrap();
    let victim = target.join("victim.json");
    fs::write(&victim, b"real credential").unwrap();
    directory.directory_link(target);
    let result = delete_synced_credential_file(&state, "linked/victim.json");
    assert!(victim.exists(), "link alias deleted the real credential");
    assert!(result.is_err(), "in-root link must be rejected");
}

#[tokio::test]
async fn rooted_paths_are_rejected_before_normalization() {
    let directory = Directory::new();
    let state = directory.state(true);
    for input in [
        "/victim.json",
        "\\victim.json",
        "//host/share/victim.json",
        "C:/victim.json",
    ] {
        let victim = directory.root.join("victim.json");
        fs::write(&victim, b"real credential").unwrap();
        let result = delete_synced_credential_file(&state, input);
        assert!(
            victim.exists(),
            "rooted input deleted a relative file: {input}"
        );
        assert!(result.is_err(), "rooted input must be rejected: {input}");
    }
}

#[tokio::test]
async fn ambiguous_names_are_rejected_without_alias_deletion() {
    let directory = Directory::new();
    let state = directory.state(true);
    for input in [
        "./victim.json",
        "victim.json.",
        "victim.json ",
        " victim.json",
        "victim.json:stream",
        "NUL.json",
        "COM1",
        "bad\0name",
        "provider/../victim.json",
    ] {
        let victim = directory.root.join("victim.json");
        fs::write(&victim, b"real credential").unwrap();
        let result = delete_synced_credential_file(&state, input);
        assert!(
            victim.exists(),
            "ambiguous input deleted a real file: {input:?}"
        );
        assert!(
            result.is_err(),
            "ambiguous input must be rejected: {input:?}"
        );
    }
}

#[tokio::test]
async fn normal_relative_deletion_preserves_separator_compatibility() {
    let directory = Directory::new();
    let state = directory.state(true);
    fs::create_dir(directory.root.join("provider")).unwrap();
    for input in [
        "provider/victim.json",
        "provider\\victim.json",
        "provider//victim.json",
        "provider\\/victim.json",
    ] {
        let victim = directory.root.join("provider/victim.json");
        fs::write(&victim, b"owned credential").unwrap();
        assert!(delete_synced_credential_file(&state, input).unwrap());
        assert!(!victim.exists());
        assert!(!delete_synced_credential_file(&state, input).unwrap());
    }
}

#[tokio::test]
async fn empty_missing_and_disabled_deletions_are_noops() {
    let directory = Directory::new();
    let state = directory.state(true);
    for input in ["", "   ", "missing.json", "missing/victim.json"] {
        assert!(!delete_synced_credential_file(&state, input).unwrap());
    }
    let disabled = directory.state(false);
    let victim = directory.root.join("victim.json");
    fs::write(&victim, b"keep credential").unwrap();
    assert!(!delete_synced_credential_file(&disabled, "victim.json").unwrap());
    assert!(!delete_synced_credential_file(&disabled, "../outside/victim.json").unwrap());
    assert!(victim.exists());
    fs::remove_file(&victim).unwrap();
    fs::remove_dir(&directory.root).unwrap();
    assert!(!delete_synced_credential_file(&state, "victim.json").unwrap());
}

#[cfg(unix)]
#[tokio::test]
async fn leaf_symlinks_including_dangling_links_are_rejected() {
    let directory = Directory::new();
    let state = directory.state(true);
    let victim = directory.outside.join("victim.json");
    fs::write(&victim, b"outside credential").unwrap();
    for target in [&victim, &directory.outside.join("absent.json")] {
        let link = directory.root.join("victim.json");
        std::os::unix::fs::symlink(target, &link).unwrap();
        assert!(delete_synced_credential_file(&state, "victim.json").is_err());
        assert!(fs::symlink_metadata(&link).is_ok());
        assert!(victim.exists());
        fs::remove_file(link).unwrap();
    }
}
