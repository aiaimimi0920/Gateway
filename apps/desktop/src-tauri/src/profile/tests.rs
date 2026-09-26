use super::*;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

fn temporary_package_dir() -> PathBuf {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock must be after unix epoch")
        .as_nanos();
    std::env::temp_dir().join(format!("gateway-profile-contract-{nonce}"))
}

#[test]
fn profile_serializes_runtime_contract_and_resolves_package_relative_routes() {
    let package_dir = temporary_package_dir();
    let working_dir = package_dir.join("runtime");
    fs::create_dir_all(&working_dir).expect("create test working directory");
    fs::write(working_dir.join("routes.yaml"), "routes: []\n").expect("write test routes file");

    let profile = GatewayProfile {
        name: "portable".to_string(),
        runtime_role: GatewayRuntimeRole::Standalone,
        gateway_management_token: Some("management-secret".to_string()),
        port: 4200,
        gateway_redis_url: "redis://127.0.0.1:6379".to_string(),
        gateway_database_url: None,
        gateway_routes_file: Some("routes.yaml".to_string()),
        log_level: Some("info".to_string()),
        working_directory: Some("runtime".to_string()),
        extra_env: Vec::new(),
    };

    let resolved_working_dir = resolve_profile_working_directory_from(&profile, &package_dir)
        .expect("resolve working directory");
    let resolved_routes = resolve_profile_routes_file_from(
        &profile,
        &package_dir,
        profile
            .gateway_routes_file
            .as_deref()
            .expect("routes value"),
    )
    .expect("resolve routes file");
    let serialized = serde_json::to_value(&profile).expect("serialize profile");

    assert_eq!(resolved_working_dir, working_dir);
    assert_eq!(resolved_routes, working_dir.join("routes.yaml"));
    assert_eq!(serialized["runtimeRole"], "standalone");
    assert_eq!(serialized["gatewayManagementToken"], "management-secret");

    let _ = fs::remove_dir_all(package_dir);
}

#[test]
fn dev_layout_resolves_headless_binary_from_gateway_target() {
    let package_directory = temporary_package_dir()
        .join("Gateway")
        .join("apps")
        .join("desktop")
        .join("src-tauri")
        .join("target")
        .join("debug");
    let gateway_target = package_directory
        .ancestors()
        .nth(5)
        .expect("Gateway ancestor")
        .join("target")
        .join("debug");
    fs::create_dir_all(&package_directory).expect("create package directory");
    fs::create_dir_all(&gateway_target).expect("create Gateway target directory");
    let expected = gateway_target.join(if cfg!(windows) {
        "gateway.exe"
    } else {
        "gateway"
    });
    fs::write(&expected, b"fixture").expect("write headless fixture");

    assert_eq!(
        resolve_gateway_sidecar_path_from(&package_directory),
        expected
    );
    let _ = fs::remove_dir_all(
        package_directory
            .ancestors()
            .nth(5)
            .expect("Gateway ancestor"),
    );
}
