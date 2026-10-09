use super::*;

fn second_id(runtime: &ConsoleAuthRuntime, token: &str) -> String {
    let list = serde_json::to_value(
        runtime
            .list_management_keys(&ConsoleRequestContext::loopback(), token)
            .unwrap(),
    )
    .unwrap();
    list[1]["id"].as_str().unwrap().to_string()
}

#[test]
fn edited_keys_preserve_identity_and_revoke_replaced_tokens_after_restart() {
    let fixture = TestConsoleFixture::new(false);
    let request = ConsoleRequestContext::loopback();
    let runtime = ConsoleAuthRuntime::new(&fixture.config, Some("first".into())).unwrap();
    runtime
        .add_management_key(&request, "first", "Second", "second")
        .unwrap();
    let id = second_id(&runtime, "first");
    runtime
        .edit_management_key(&request, "first", &id, "Renamed", None)
        .unwrap();
    assert_eq!(
        runtime
            .reveal_management_key(&request, "first", &id)
            .unwrap()
            .as_deref(),
        Some("second")
    );
    runtime
        .edit_management_key(&request, "first", &id, "Renamed", Some("replacement"))
        .unwrap();
    assert!(runtime.verify_management_token("second").is_err());
    runtime.verify_management_token("replacement").unwrap();
    let restarted = ConsoleAuthRuntime::new(&fixture.config, Some("first".into())).unwrap();
    assert_eq!(second_id(&restarted, "first"), id);
    assert!(restarted.verify_management_token("second").is_err());
    assert_eq!(
        restarted
            .reveal_management_key(&request, "first", &id)
            .unwrap()
            .as_deref(),
        Some("replacement")
    );
    let record = fs::read_to_string(runtime.admin_path()).unwrap();
    assert!(!record.contains("replacement"));
    assert!(!record.contains("\"second\""));
    let list =
        serde_json::to_string(&runtime.list_management_keys(&request, "first").unwrap()).unwrap();
    assert!(!list.contains("sealedToken"));
    assert!(!list.contains("tokenHash"));
    assert!(!list.contains("replacement"));
}

#[test]
fn initial_and_legacy_hash_only_keys_can_be_edited_without_locking_out_the_last_admin() {
    let fixture = TestConsoleFixture::new(false);
    let request = ConsoleRequestContext::loopback();
    let runtime = ConsoleAuthRuntime::new(&fixture.config, None).unwrap();
    runtime.bootstrap(&request, "legacy").unwrap();
    assert_eq!(
        runtime
            .reveal_management_key(&request, "legacy", "initial")
            .unwrap()
            .as_deref(),
        Some("legacy")
    );
    runtime
        .edit_management_key(&request, "legacy", "initial", "Renamed", None)
        .unwrap();
    runtime
        .add_management_key(&request, "legacy", "Other", "other")
        .unwrap();
    assert_eq!(
        runtime
            .reveal_management_key(&request, "other", "initial")
            .unwrap(),
        None
    );
    runtime
        .edit_management_key(&request, "other", "initial", "Updated", Some("updated"))
        .unwrap();
    assert!(runtime.verify_management_token("legacy").is_err());
    assert_eq!(
        runtime
            .reveal_management_key(&request, "other", "initial")
            .unwrap()
            .as_deref(),
        Some("updated")
    );
    assert!(runtime
        .admin_path()
        .parent()
        .unwrap()
        .join("admin-before-keyring.json")
        .exists());
}

#[test]
fn invalid_edits_and_reveals_cannot_change_or_disclose_keys() {
    let fixture = TestConsoleFixture::new(false);
    let request = ConsoleRequestContext::loopback();
    let runtime = ConsoleAuthRuntime::new(&fixture.config, Some("first".into())).unwrap();
    runtime
        .add_management_key(&request, "first", "Second", "second")
        .unwrap();
    let id = second_id(&runtime, "first");
    let before = fs::read(runtime.admin_path()).unwrap();
    for (name, token) in [
        ("", None),
        ("New", Some("with spaces")),
        ("New", Some("中文")),
        ("New", Some("first")),
    ] {
        assert!(runtime
            .edit_management_key(&request, "first", &id, name, token)
            .is_err());
    }
    assert!(runtime
        .edit_management_key(&request, "wrong", &id, "New", Some("third"))
        .is_err());
    assert!(runtime
        .edit_management_key(&request, "first", "missing", "New", None)
        .is_err());
    assert!(runtime
        .reveal_management_key(&request, "wrong", &id)
        .is_err());
    assert!(runtime
        .reveal_management_key(&request, "first", "missing")
        .is_err());
    let remote = ConsoleRequestContext::new(IpAddr::from([192, 0, 2, 1]), "remote".into());
    assert!(runtime
        .reveal_management_key(&remote, "first", &id)
        .is_err());
    assert!(runtime
        .edit_management_key(&remote, "first", &id, "New", None)
        .is_err());
    assert_eq!(before, fs::read(runtime.admin_path()).unwrap());
    runtime
        .revoke_management_key(&request, "first", &id)
        .unwrap();
    assert!(runtime
        .reveal_management_key(&request, "first", &id)
        .is_err());
    assert!(runtime
        .reveal_management_key(&request, "second", "initial")
        .is_err());
}

#[test]
fn damaged_display_storage_fails_closed_without_affecting_hash_authentication() {
    let fixture = TestConsoleFixture::new(false);
    let request = ConsoleRequestContext::loopback();
    let runtime = ConsoleAuthRuntime::new(&fixture.config, Some("first".into())).unwrap();
    runtime
        .add_management_key(&request, "first", "Second", "second")
        .unwrap();
    let id = second_id(&runtime, "first");
    let mut record: serde_json::Value =
        serde_json::from_slice(&fs::read(runtime.admin_path()).unwrap()).unwrap();
    record["keys"][1]["sealedToken"] = "tampered".into();
    fs::write(runtime.admin_path(), serde_json::to_vec(&record).unwrap()).unwrap();
    assert!(runtime
        .reveal_management_key(&request, "first", &id)
        .is_err());
    runtime.verify_management_token("second").unwrap();
    let seal_path = runtime
        .admin_path()
        .parent()
        .unwrap()
        .join("management-key-seal.json");
    fs::remove_file(seal_path).unwrap();
    assert!(runtime
        .edit_management_key(&request, "first", &id, "New", Some("third"))
        .is_err());
    runtime.verify_management_token("second").unwrap();
}

#[test]
fn single_environment_key_replacement_is_atomic_and_persists() {
    let fixture = TestConsoleFixture::new(false);
    let request = ConsoleRequestContext::loopback();
    let runtime = ConsoleAuthRuntime::new(&fixture.config, Some("environment".into())).unwrap();
    runtime
        .edit_management_key(
            &request,
            "environment",
            "initial",
            "Main",
            Some("new-token"),
        )
        .unwrap();
    assert!(runtime.verify_management_token("environment").is_err());
    assert_eq!(
        runtime
            .list_management_keys(&request, "new-token")
            .unwrap()
            .len(),
        1
    );
    let restarted = ConsoleAuthRuntime::new(&fixture.config, Some("environment".into())).unwrap();
    assert!(restarted.verify_management_token("environment").is_err());
    restarted.verify_management_token("new-token").unwrap();
}
