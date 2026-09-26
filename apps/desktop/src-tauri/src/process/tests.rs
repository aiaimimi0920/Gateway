use super::shutdown::{build_drain_request, shutdown_state_for_exit};
use super::*;
use crate::profile::GatewayRuntimeRole;
use std::collections::BTreeMap;
#[cfg(windows)]
use std::time::Instant;

fn portable_profile() -> GatewayProfile {
    GatewayProfile {
        name: "portable".to_string(),
        runtime_role: GatewayRuntimeRole::Standalone,
        gateway_management_token: Some("management-secret".to_string()),
        port: 4200,
        gateway_redis_url: "redis://127.0.0.1:6379".to_string(),
        gateway_database_url: None,
        gateway_routes_file: Some("routes.yaml".to_string()),
        log_level: Some("info".to_string()),
        working_directory: None,
        extra_env: Vec::new(),
    }
}

#[test]
fn child_environment_contains_runtime_role_token_and_resolved_routes() {
    let profile = portable_profile();
    let working_directory = std::env::temp_dir().join("gateway-process-contract");
    let environment =
        render_child_environment(&profile, &working_directory).expect("render child environment");

    assert_eq!(
        environment.get("GATEWAY_RUNTIME_ROLE"),
        Some(&"standalone".to_string())
    );
    assert_eq!(
        environment.get("GATEWAY_MANAGEMENT_TOKEN"),
        Some(&"management-secret".to_string())
    );
    assert_eq!(
        environment.get("GATEWAY_ROUTES_FILE"),
        Some(&working_directory.join("routes.yaml").display().to_string())
    );
}

#[test]
fn empty_optional_profile_values_remove_inherited_desktop_environment() {
    let mut profile = portable_profile();
    profile.gateway_database_url = None;
    profile.gateway_routes_file = None;
    profile.gateway_management_token = None;
    profile.log_level = None;
    let environment = render_child_environment(&profile, &std::env::temp_dir())
        .expect("render child environment");
    let mut command = Command::new(if cfg!(windows) { "cmd" } else { "sh" });
    configure_child_environment(&mut command, environment);
    let configured: BTreeMap<_, _> = command
        .get_envs()
        .map(|(key, value)| {
            (
                key.to_string_lossy().into_owned(),
                value.map(|value| value.to_string_lossy().into_owned()),
            )
        })
        .collect();

    for key in [
        "GATEWAY_DATABASE_URL",
        "DATABASE_URL",
        "GATEWAY_ROUTES_FILE",
        "GATEWAY_MANAGEMENT_TOKEN",
        "RUST_LOG",
    ] {
        assert_eq!(configured.get(key), Some(&None), "{key} must be removed");
    }
    assert_eq!(
        configured.get("GATEWAY_REDIS_URL"),
        Some(&Some("redis://127.0.0.1:6379".to_string()))
    );
}

#[test]
fn drain_request_uses_management_token_header() {
    let client = reqwest::blocking::Client::new();
    let request = build_drain_request(&client, 4200, Some("management-secret"))
        .build()
        .expect("build drain request");
    assert_eq!(
        request
            .headers()
            .get("x-management-token")
            .and_then(|value| value.to_str().ok()),
        Some("management-secret")
    );
}

#[test]
fn nonzero_child_exit_never_counts_as_graceful() {
    let mut command = if cfg!(windows) {
        let mut command = Command::new("cmd");
        command.args(["/C", "exit", "7"]);
        command
    } else {
        let mut command = Command::new("sh");
        command.args(["-c", "exit 7"]);
        command
    };
    let mut child = command.spawn().expect("spawn exit-status fixture");
    let status = child.wait().expect("wait exit-status fixture");
    assert!(!status.success());
    assert_eq!(shutdown_state_for_exit(true, &status), "exited");
}

#[test]
fn standalone_startup_ignores_redis_preflight_when_local_routes_exist() {
    let redis_only = crate::profile::GatewayProfilePreflight {
        ok: false,
        sidecar: crate::profile::GatewayProfilePathCheckItem {
            configured_path: None,
            resolved_path: "gateway.exe".to_string(),
            expected_kind: "file".to_string(),
            exists: true,
            is_file: true,
            is_dir: false,
            ok: true,
            message: "Gateway sidecar executable is available".to_string(),
        },
        working_directory: crate::profile::GatewayProfilePathCheckItem {
            configured_path: None,
            resolved_path: ".".to_string(),
            expected_kind: "directory".to_string(),
            exists: true,
            is_file: false,
            is_dir: true,
            ok: true,
            message: "working directory is available".to_string(),
        },
        gateway_routes_file: crate::profile::GatewayProfilePathCheckItem {
            configured_path: None,
            resolved_path: "routes.yaml".to_string(),
            expected_kind: "file".to_string(),
            exists: true,
            is_file: true,
            is_dir: false,
            ok: true,
            message: "default routes.yaml is available".to_string(),
        },
        redis: crate::profile::GatewayDependencyCheckItem {
            name: "Redis".to_string(),
            required: true,
            configured: true,
            ok: false,
            message: "Redis is not reachable. Start Redis or update GATEWAY_REDIS_URL.".to_string(),
        },
        database: crate::profile::GatewayDependencyCheckItem {
            name: "PostgreSQL".to_string(),
            required: false,
            configured: false,
            ok: true,
            message: "PostgreSQL is not configured".to_string(),
        },
        messages: vec![
            "Redis is not reachable. Start Redis or update GATEWAY_REDIS_URL.".to_string(),
        ],
    };
    let profile = portable_profile();

    assert_eq!(
        startup_preflight_failure_message(&profile, &redis_only),
        None
    );
}

#[test]
fn standalone_startup_still_blocks_when_redis_is_missing_and_no_routes_file_exists() {
    let profile = GatewayProfile {
        gateway_routes_file: None,
        ..portable_profile()
    };
    let redis_without_routes = crate::profile::GatewayProfilePreflight {
        ok: false,
        sidecar: crate::profile::GatewayProfilePathCheckItem {
            configured_path: None,
            resolved_path: "gateway.exe".to_string(),
            expected_kind: "file".to_string(),
            exists: true,
            is_file: true,
            is_dir: false,
            ok: true,
            message: "Gateway sidecar executable is available".to_string(),
        },
        working_directory: crate::profile::GatewayProfilePathCheckItem {
            configured_path: None,
            resolved_path: ".".to_string(),
            expected_kind: "directory".to_string(),
            exists: true,
            is_file: false,
            is_dir: true,
            ok: true,
            message: "working directory is available".to_string(),
        },
        gateway_routes_file: crate::profile::GatewayProfilePathCheckItem {
            configured_path: None,
            resolved_path: "routes.yaml".to_string(),
            expected_kind: "file".to_string(),
            exists: false,
            is_file: false,
            is_dir: false,
            ok: true,
            message: "GATEWAY_ROUTES_FILE is not configured and default routes.yaml is absent; Gateway may still load routes from Redis"
                .to_string(),
        },
        redis: crate::profile::GatewayDependencyCheckItem {
            name: "Redis".to_string(),
            required: true,
            configured: true,
            ok: false,
            message: "Redis is not reachable. Start Redis or update GATEWAY_REDIS_URL."
                .to_string(),
        },
        database: crate::profile::GatewayDependencyCheckItem {
            name: "PostgreSQL".to_string(),
            required: false,
            configured: false,
            ok: true,
            message: "PostgreSQL is not configured".to_string(),
        },
        messages: vec![
            "Redis is not reachable. Start Redis or update GATEWAY_REDIS_URL.".to_string(),
        ],
    };

    let failure = startup_preflight_failure_message(&profile, &redis_without_routes)
        .expect("redis must remain blocking when standalone has no local routes file");
    assert!(failure.contains("Redis is not reachable"));
}

#[test]
fn worker_startup_still_requires_redis_even_when_routes_exist() {
    let profile = GatewayProfile {
        runtime_role: GatewayRuntimeRole::Worker,
        ..portable_profile()
    };
    let preflight = crate::profile::GatewayProfilePreflight {
        ok: false,
        sidecar: crate::profile::GatewayProfilePathCheckItem {
            configured_path: None,
            resolved_path: "gateway.exe".to_string(),
            expected_kind: "file".to_string(),
            exists: true,
            is_file: true,
            is_dir: false,
            ok: true,
            message: "Gateway sidecar executable is available".to_string(),
        },
        working_directory: crate::profile::GatewayProfilePathCheckItem {
            configured_path: None,
            resolved_path: ".".to_string(),
            expected_kind: "directory".to_string(),
            exists: true,
            is_file: false,
            is_dir: true,
            ok: true,
            message: "working directory is available".to_string(),
        },
        gateway_routes_file: crate::profile::GatewayProfilePathCheckItem {
            configured_path: None,
            resolved_path: "routes.yaml".to_string(),
            expected_kind: "file".to_string(),
            exists: true,
            is_file: true,
            is_dir: false,
            ok: true,
            message: "default routes.yaml is available".to_string(),
        },
        redis: crate::profile::GatewayDependencyCheckItem {
            name: "Redis".to_string(),
            required: true,
            configured: true,
            ok: false,
            message: "Redis is not reachable. Start Redis or update GATEWAY_REDIS_URL.".to_string(),
        },
        database: crate::profile::GatewayDependencyCheckItem {
            name: "PostgreSQL".to_string(),
            required: false,
            configured: false,
            ok: true,
            message: "PostgreSQL is not configured".to_string(),
        },
        messages: vec![
            "Redis is not reachable. Start Redis or update GATEWAY_REDIS_URL.".to_string(),
        ],
    };

    let failure = startup_preflight_failure_message(&profile, &preflight)
        .expect("worker startup must still block on Redis");
    assert!(failure.contains("Redis is not reachable"));
}

#[cfg(windows)]
#[test]
fn terminate_process_tree_kills_windows_grandchildren() {
    use std::fs;
    use std::thread;
    use std::time::{Duration, SystemTime, UNIX_EPOCH};

    fn process_exists(pid: u32) -> bool {
        Command::new("tasklist")
            .args(["/FI", &format!("PID eq {pid}"), "/FO", "CSV", "/NH"])
            .output()
            .map(|output| String::from_utf8_lossy(&output.stdout).contains(&pid.to_string()))
            .unwrap_or(false)
    }

    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock must be after unix epoch")
        .as_nanos();
    let pid_file = std::env::temp_dir().join(format!("gateway-tree-{nonce}.pid"));
    let escaped_path = pid_file.display().to_string().replace('\'', "''");
    let script = format!(
        "$child = Start-Process -FilePath $env:ComSpec -ArgumentList '/c','ping 127.0.0.1 -n 60 > nul' -PassThru -WindowStyle Hidden; [IO.File]::WriteAllText('{escaped_path}', $child.Id.ToString()); Start-Sleep -Seconds 60"
    );
    let mut parent = Command::new("powershell.exe")
        .args(["-NoProfile", "-NonInteractive", "-Command", &script])
        .spawn()
        .expect("spawn process-tree fixture");

    let started_at = Instant::now();
    let child_pid = loop {
        if let Ok(contents) = std::fs::read_to_string(&pid_file) {
            if let Ok(pid) = contents.trim().parse::<u32>() {
                break pid;
            }
        }
        assert!(started_at.elapsed() < Duration::from_secs(5));
        thread::sleep(Duration::from_millis(50));
    };
    assert!(process_exists(child_pid));

    terminate_process_tree(&mut parent);
    assert!(!process_exists(child_pid));
    let _ = fs::remove_file(pid_file);
}
