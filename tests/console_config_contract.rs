use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use neuro_gateway::console::{validate_state_directory, ConsoleConfig, ConsoleConfigValues};
use neuro_gateway::routing::config::RouteConfigStore;

mod support;

static ENV_LOCK: Mutex<()> = Mutex::new(());

#[test]
fn console_defaults_are_loopback_first_and_namespaced() {
    let config = ConsoleConfig::from_values(ConsoleConfigValues::default()).unwrap();

    assert!(!config.remote_access_enabled);
    assert_eq!(config.redis_namespace, "default");
    assert_eq!(config.secret_grant_ttl_secs, 300);
    assert_eq!(config.max_event_records, 2_000);
}

#[test]
fn release_state_directory_must_not_be_inside_release_payload() {
    let error = validate_state_directory(
        Path::new(r"C:\release\Gateway\V1"),
        Some(Path::new(r"C:\release\Gateway\V1")),
    )
    .unwrap_err();

    assert_eq!(error.code(), "console_state_inside_release_payload");
}

#[test]
fn shared_state_builder_is_used_by_existing_integration_contracts() {
    let smoke = include_str!("smoke.rs");
    let operator_summary = include_str!("operator_summary_contract.rs");

    assert!(smoke.contains("build_test_app_state("));
    assert!(!smoke.contains("Arc::new(AppState"));
    assert!(operator_summary.contains("build_test_app_state("));
    assert!(!operator_summary.contains("Arc::new(AppState"));
}

#[test]
fn runtime_uses_the_resolved_console_routes_path_as_its_only_yaml_source() {
    let runtime = include_str!("../src/runtime.rs");

    assert!(!runtime.contains("pub async fn load_route_config("));
    assert!(!runtime.contains("std::env::var(\"GATEWAY_ROUTES_FILE\")"));
    assert!(runtime.contains("&config.console.routes_file"));
}

#[test]
fn release_state_directory_descendant_is_rejected() {
    let release = unique_test_path("descendant-release");
    let state = release.join("state");

    let error = validate_state_directory(&state, Some(&release)).unwrap_err();

    assert_eq!(error.code(), "console_state_inside_release_payload");
}

#[test]
fn release_state_directory_sibling_is_allowed() {
    let release = unique_test_path("sibling-release");
    let state = release.with_file_name("sibling-state");

    validate_state_directory(&state, Some(&release)).unwrap();
}

#[test]
fn state_directory_cannot_contain_the_release_payload() {
    let base = unique_test_path("state-contains-release");
    let state = base.join("state-root");
    let release = state.join("releases").join("V1");

    let error = validate_state_directory(&state, Some(&release)).unwrap_err();

    assert_eq!(error.code(), "console_state_contains_release_payload");
}

#[test]
fn routes_file_inside_release_payload_is_rejected() {
    let base = unique_test_path("routes-in-release");
    let release = base.join("release").join("V1");
    let values = ConsoleConfigValues {
        state_dir: Some(base.join("state")),
        routes_file: Some(release.join("routes.yaml")),
        release_payload_root: Some(release),
        ..ConsoleConfigValues::default()
    };

    let error = ConsoleConfig::from_values(values).unwrap_err();

    assert_eq!(error.code(), "console_routes_inside_release_payload");
}

#[test]
fn state_link_located_inside_release_is_rejected_even_when_target_is_outside() {
    let base = unique_test_path("state-link-inside-release");
    let release = base.join("release");
    let outside_state = base.join("outside-state");
    let linked_state = release.join("state-link");
    std::fs::create_dir_all(&release).unwrap();
    std::fs::create_dir_all(&outside_state).unwrap();

    if let Err(error) = create_directory_link(&outside_state, &linked_state) {
        let _ = std::fs::remove_dir_all(&base);
        eprintln!("directory links unavailable for lexical state contract: {error}");
        return;
    }

    let validation = validate_state_directory(&linked_state, Some(&release));
    let _ = std::fs::remove_dir(&linked_state);
    let _ = std::fs::remove_dir_all(&base);

    let error = validation.unwrap_err();
    assert_eq!(error.code(), "console_state_inside_release_payload");
}

#[test]
fn routes_link_located_inside_release_is_rejected_even_when_target_is_outside() {
    let base = unique_test_path("routes-link-inside-release");
    let release = base.join("release");
    let outside_routes = base.join("outside-routes");
    let routes_link = release.join("routes-link");
    std::fs::create_dir_all(&release).unwrap();
    std::fs::create_dir_all(&outside_routes).unwrap();
    std::fs::write(outside_routes.join("routes.yaml"), "providers: []\n").unwrap();

    if let Err(error) = create_directory_link(&outside_routes, &routes_link) {
        let _ = std::fs::remove_dir_all(&base);
        eprintln!("directory links unavailable for lexical routes contract: {error}");
        return;
    }

    let values = ConsoleConfigValues {
        state_dir: Some(base.join("state")),
        routes_file: Some(routes_link.join("routes.yaml")),
        release_payload_root: Some(release.clone()),
        ..ConsoleConfigValues::default()
    };
    let validation = ConsoleConfig::from_values(values);
    let _ = std::fs::remove_dir(&routes_link);
    let _ = std::fs::remove_dir_all(&base);

    let error = validation.unwrap_err();
    assert_eq!(error.code(), "console_routes_inside_release_payload");
}

#[test]
fn console_config_stores_resolved_absolute_mutable_paths() {
    let base = unique_test_path("resolved-config-paths");
    let values = ConsoleConfigValues {
        state_dir: Some(base.join("temporary").join("..").join("state")),
        routes_file: Some(base.join("draft").join("..").join("routes.yaml")),
        ..ConsoleConfigValues::default()
    };

    let config = ConsoleConfig::from_values(values).unwrap();

    assert!(config.state_dir.is_absolute());
    assert!(config.routes_file.is_absolute());
    assert!(!config
        .state_dir
        .components()
        .any(|component| component == std::path::Component::ParentDir));
    assert!(!config
        .routes_file
        .components()
        .any(|component| component == std::path::Component::ParentDir));
}

#[cfg(windows)]
#[test]
fn windows_drive_relative_mutable_paths_are_stored_as_absolute() {
    use std::path::Prefix;

    let current_dir = std::env::current_dir().unwrap();
    let Some(prefix) = current_dir
        .components()
        .find_map(|component| match component {
            std::path::Component::Prefix(prefix) => Some(prefix.kind()),
            _ => None,
        })
    else {
        return;
    };
    let drive = match prefix {
        Prefix::Disk(drive) | Prefix::VerbatimDisk(drive) => drive as char,
        _ => return,
    };
    let values = ConsoleConfigValues {
        state_dir: Some(PathBuf::from(format!(r"{drive}:gateway-state"))),
        routes_file: Some(PathBuf::from(format!(r"{drive}:temporary\..\routes.yaml"))),
        ..ConsoleConfigValues::default()
    };

    let config = ConsoleConfig::from_values(values).unwrap();

    assert!(config.state_dir.is_absolute());
    assert!(config.routes_file.is_absolute());
    assert!(!config
        .routes_file
        .components()
        .any(|component| component == std::path::Component::ParentDir));
}

#[test]
fn release_state_directory_dot_dot_components_are_resolved() {
    let release = unique_test_path("dot-dot-release");
    let state = release.join("temporary").join("..").join("state");

    let error = validate_state_directory(&state, Some(&release)).unwrap_err();

    assert_eq!(error.code(), "console_state_inside_release_payload");
}

#[cfg(windows)]
#[test]
fn windows_verbatim_and_regular_paths_compare_as_the_same_location() {
    let release = unique_test_path("verbatim-release");
    let state = release.join("state");
    let verbatim_state = PathBuf::from(format!(r"\\?\{}", state.display()));

    let error = validate_state_directory(&verbatim_state, Some(&release)).unwrap_err();

    assert_eq!(error.code(), "console_state_inside_release_payload");
}

#[cfg(windows)]
#[test]
fn windows_path_components_compare_case_insensitively() {
    let release = PathBuf::from(r"C:\release\Gateway\V1");
    let state = PathBuf::from(r"c:\RELEASE\gateway\v1\state");

    let error = validate_state_directory(&state, Some(&release)).unwrap_err();

    assert_eq!(error.code(), "console_state_inside_release_payload");
}

#[cfg(windows)]
#[test]
fn windows_distinct_non_unicode_components_do_not_compare_equal() {
    use std::os::windows::ffi::OsStringExt;

    let mut release = PathBuf::from(r"C:\");
    release.push(OsString::from_wide(&[
        b'p' as u16,
        b'a' as u16,
        b'y' as u16,
        b'l' as u16,
        b'o' as u16,
        b'a' as u16,
        b'd' as u16,
        b'-' as u16,
        0xD800,
    ]));
    let mut state = PathBuf::from(r"C:\");
    state.push(OsString::from_wide(&[
        b'p' as u16,
        b'a' as u16,
        b'y' as u16,
        b'l' as u16,
        b'o' as u16,
        b'a' as u16,
        b'd' as u16,
        b'-' as u16,
        0xD801,
    ]));
    state.push("state");

    validate_state_directory(&state, Some(&release)).unwrap();
}

#[cfg(unix)]
#[test]
fn unix_path_components_remain_case_sensitive() {
    let release = PathBuf::from("/tmp/gateway-release");
    let state = PathBuf::from("/tmp/GATEWAY-RELEASE/state");

    validate_state_directory(&state, Some(&release)).unwrap();
}

#[test]
fn existing_link_into_release_payload_is_rejected_when_links_are_available() {
    let base = unique_test_path("link-release");
    let release = base.join("payload");
    let outside = base.join("outside");
    let linked_state = outside.join("linked-state");
    std::fs::create_dir_all(&release).unwrap();
    std::fs::create_dir_all(&outside).unwrap();

    if let Err(error) = create_directory_link(&release, &linked_state) {
        let _ = std::fs::remove_dir_all(&base);
        eprintln!("directory links unavailable for contract test: {error}");
        return;
    }

    let validation = validate_state_directory(&linked_state, Some(&release));
    let _ = std::fs::remove_dir(&linked_state);
    let _ = std::fs::remove_dir_all(&base);

    let error = validation.unwrap_err();
    assert_eq!(error.code(), "console_state_inside_release_payload");
}

#[test]
fn environment_path_values_preserve_non_unicode_data() {
    let _guard = ENV_LOCK.lock().unwrap();
    let key = "GATEWAY_ROUTES_FILE";
    let previous = std::env::var_os(key);
    let value = non_unicode_path_value();
    std::env::set_var(key, &value);

    let parsed = ConsoleConfigValues::from_env().unwrap();

    match previous {
        Some(previous) => std::env::set_var(key, previous),
        None => std::env::remove_var(key),
    }
    assert_eq!(parsed.routes_file, Some(PathBuf::from(value)));
}

#[test]
fn route_config_loader_accepts_path_values_without_string_conversion() {
    let path = unique_test_path("path-loader").with_extension("yaml");
    std::fs::write(&path, "providers: []\nmodel_routes: []\naliases: {}\n").unwrap();

    let store = RouteConfigStore::load_from_yaml(&path).unwrap();

    let _ = std::fs::remove_file(path);
    assert_eq!(store.provider_count(), 0);
}

#[cfg(unix)]
#[test]
fn link_parent_traversal_is_resolved_before_dot_dot_components() {
    let base = unique_test_path("link-parent-traversal");
    let release = base.join("payload");
    let release_child = release.join("child");
    let outside = base.join("outside");
    let link = outside.join("linked-child");
    std::fs::create_dir_all(&release_child).unwrap();
    std::fs::create_dir_all(&outside).unwrap();

    if let Err(error) = create_directory_link(&release_child, &link) {
        let _ = std::fs::remove_dir_all(&base);
        eprintln!("directory links unavailable for traversal contract: {error}");
        return;
    }

    let state = link.join("..").join("state");
    let validation = validate_state_directory(&state, Some(&release));
    let _ = std::fs::remove_dir(&link);
    let _ = std::fs::remove_dir_all(&base);

    let error = validation.unwrap_err();
    assert_eq!(error.code(), "console_state_inside_release_payload");
}

fn unique_test_path(label: &str) -> PathBuf {
    std::env::temp_dir().join(format!("gateway-console-{label}-{}", uuid::Uuid::new_v4()))
}

#[cfg(windows)]
fn non_unicode_path_value() -> OsString {
    use std::os::windows::ffi::OsStringExt;

    OsString::from_wide(&[
        b'C' as u16,
        b':' as u16,
        b'\\' as u16,
        b'r' as u16,
        b'o' as u16,
        b'u' as u16,
        b't' as u16,
        b'e' as u16,
        b's' as u16,
        b'-' as u16,
        0xD800,
    ])
}

#[cfg(unix)]
fn non_unicode_path_value() -> OsString {
    use std::os::unix::ffi::OsStringExt;

    OsString::from_vec(b"/tmp/gateway-routes-\xFF".to_vec())
}

#[cfg(windows)]
fn create_directory_link(target: &Path, link: &Path) -> std::io::Result<()> {
    match std::os::windows::fs::symlink_dir(target, link) {
        Ok(()) => Ok(()),
        Err(symlink_error) => {
            let output = std::process::Command::new("cmd")
                .args(["/d", "/c", "mklink", "/J"])
                .arg(link)
                .arg(target)
                .output()?;
            if output.status.success() {
                Ok(())
            } else {
                Err(std::io::Error::new(
                    symlink_error.kind(),
                    format!(
                        "symlink failed ({symlink_error}); junction failed: {}",
                        String::from_utf8_lossy(&output.stderr).trim()
                    ),
                ))
            }
        }
    }
}

#[cfg(unix)]
fn create_directory_link(target: &Path, link: &Path) -> std::io::Result<()> {
    std::os::unix::fs::symlink(target, link)
}
