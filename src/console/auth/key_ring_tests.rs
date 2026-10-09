use super::*;

#[test]
fn concurrent_revocation_cannot_remove_the_last_key() {
    let fixture = TestConsoleFixture::new(false);
    let runtime = ConsoleAuthRuntime::new(&fixture.config, Some("first".into())).unwrap();
    let request = ConsoleRequestContext::loopback();
    runtime
        .add_management_key(&request, "first", "Second", "second")
        .unwrap();
    let keys =
        serde_json::to_value(runtime.list_management_keys(&request, "first").unwrap()).unwrap();
    let second_id = keys[1]["id"].as_str().unwrap().to_string();
    let barrier = std::sync::Arc::new(std::sync::Barrier::new(2));
    let threads: Vec<_> = ["initial".to_string(), second_id]
        .into_iter()
        .map(|id| {
            let runtime = runtime.clone();
            let barrier = barrier.clone();
            std::thread::spawn(move || {
                barrier.wait();
                runtime
                    .revoke_management_key(&ConsoleRequestContext::loopback(), "first", &id)
                    .is_ok()
            })
        })
        .collect();
    assert_eq!(
        threads
            .into_iter()
            .filter_map(|thread| thread.join().ok())
            .filter(|ok| *ok)
            .count(),
        1
    );
    let record: serde_json::Value =
        serde_json::from_slice(&fs::read(runtime.admin_path()).unwrap()).unwrap();
    assert_eq!(record["keys"].as_array().unwrap().len(), 1);
}

#[test]
fn invalid_keyring_never_falls_back_to_the_environment_key() {
    let fixture = TestConsoleFixture::new(false);
    let runtime = ConsoleAuthRuntime::new(&fixture.config, Some("environment".into())).unwrap();
    runtime
        .add_management_key(
            &ConsoleRequestContext::loopback(),
            "environment",
            "New",
            "new-key",
        )
        .unwrap();
    fs::write(
        runtime.admin_path(),
        br#"{"version":2,"revision":"invalid","keys":[]}"#,
    )
    .unwrap();
    assert!(runtime.verify_management_token("environment").is_err());
    assert!(runtime
        .bootstrap_status(&ConsoleRequestContext::loopback())
        .is_err());
}

#[test]
fn environment_key_can_add_and_revoke_keys_persistently_without_plaintext_storage() {
    let fixture = TestConsoleFixture::new(false);
    let request = ConsoleRequestContext::loopback();
    let runtime = ConsoleAuthRuntime::new(&fixture.config, Some("old-env".into())).unwrap();
    assert_eq!(
        runtime
            .list_management_keys(&request, "old-env")
            .unwrap()
            .len(),
        1
    );
    assert_eq!(
        runtime
            .revoke_management_key(&request, "old-env", "initial")
            .unwrap_err()
            .code
            .as_deref(),
        Some("console_key_last")
    );
    runtime
        .add_management_key(&request, "old-env", "Second", "new-secret")
        .unwrap();
    runtime.verify_management_token("new-secret").unwrap();
    runtime.verify_management_token("old-env").unwrap();
    assert!(
        !runtime
            .bootstrap_status(&request)
            .unwrap()
            .environment_override
    );
    assert_eq!(
        runtime
            .add_management_key(&request, "old-env", "Duplicate", "new-secret")
            .unwrap_err()
            .code
            .as_deref(),
        Some("console_key_duplicate")
    );
    let record = fs::read_to_string(runtime.admin_path()).unwrap();
    assert!(!record.contains("old-env"));
    assert!(!record.contains("new-secret"));
    runtime
        .revoke_management_key(&request, "new-secret", "initial")
        .unwrap();
    assert!(runtime.verify_management_token("old-env").is_err());
    let restarted = ConsoleAuthRuntime::new(&fixture.config, Some("old-env".into())).unwrap();
    assert!(restarted.verify_management_token("old-env").is_err());
    restarted.verify_management_token("new-secret").unwrap();
    assert_eq!(
        restarted
            .list_management_keys(&request, "new-secret")
            .unwrap()
            .len(),
        1
    );
}

#[test]
fn legacy_admin_is_preserved_on_upgrade_and_revocations_invalidate_cached_auth_and_grants() {
    let fixture = TestConsoleFixture::new(false);
    let runtime = ConsoleAuthRuntime::new(&fixture.config, None).unwrap();
    let request = ConsoleRequestContext::loopback();
    runtime.bootstrap(&request, "legacy-token").unwrap();
    let before = fs::read(runtime.admin_path()).unwrap();
    runtime
        .add_management_key(&request, "legacy-token", "Second", "second-token")
        .unwrap();
    let backup = runtime
        .admin_path()
        .parent()
        .unwrap()
        .join("admin-before-keyring.json");
    assert_eq!(
        serde_json::from_slice::<serde_json::Value>(&before).unwrap(),
        serde_json::from_slice::<serde_json::Value>(&fs::read(backup).unwrap()).unwrap()
    );
    let grant = runtime
        .confirm_secret_access(&request, "legacy-token", "legacy-token")
        .unwrap();
    let actor = runtime
        .authenticate_management_token(&request, "legacy-token")
        .unwrap();
    runtime
        .revoke_management_key(&request, "second-token", "initial")
        .unwrap();
    assert!(runtime.verify_management_token("legacy-token").is_err());
    assert!(runtime
        .verify_secret_grant(&request, &actor, &grant.grant)
        .is_err());
    assert_eq!(
        runtime
            .rotate_token(&request, "second-token", "other-token")
            .unwrap_err()
            .code
            .as_deref(),
        Some("console_keyring_rotation_required")
    );
    assert!(runtime.bootstrap(&request, "bootstrap-again").is_err());
}

#[test]
fn invalid_unauthenticated_remote_and_failed_mutations_do_not_change_storage() {
    let fixture = TestConsoleFixture::new(false);
    let runtime = ConsoleAuthRuntime::new(&fixture.config, Some("existing".into())).unwrap();
    let local = ConsoleRequestContext::loopback();
    let remote = ConsoleRequestContext::new(IpAddr::from([192, 0, 2, 1]), "remote".into());
    assert!(runtime
        .add_management_key(&local, "wrong", "New", "new-token")
        .is_err());
    assert!(runtime
        .add_management_key(&remote, "existing", "New", "new-token")
        .is_err());
    for token in ["", "with spaces", "中文"] {
        assert!(runtime
            .add_management_key(&local, "existing", "New", token)
            .is_err());
    }
    assert!(!runtime.admin_path().exists());
    runtime
        .add_management_key(&local, "existing", "New", "new-token")
        .unwrap();
    let before = fs::read(runtime.admin_path()).unwrap();
    assert!(runtime
        .revoke_management_key(&local, "existing", "missing")
        .is_err());
    assert_eq!(before, fs::read(runtime.admin_path()).unwrap());
}
