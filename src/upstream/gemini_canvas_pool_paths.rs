//! Gemini pool payload discovery; state and logs remain owned by ConsoleConfig.
use std::path::{Path, PathBuf};

use crate::console::{ConsoleConfig, ConsoleConfigError, ConsoleConfigValues};
use crate::error::GatewayError;

const SCRIPT: &str = "scripts/gemini-canvas-browser-pool.mjs";

fn resolve_script(
    payload: Option<&Path>,
    executable: Option<&Path>,
    development: &Path,
) -> PathBuf {
    // An explicit payload is authoritative. A broken install must not run checkout code.
    if let Some(payload) = payload {
        return payload.join(SCRIPT);
    }
    if let Some(executable) = executable {
        if let Some(parent) = executable.parent() {
            let adjacent = parent.join(SCRIPT);
            if adjacent.is_file() || !executable.starts_with(development.join("target")) {
                return adjacent;
            }
        }
    }
    development.join(SCRIPT)
}

pub(super) fn script_path() -> PathBuf {
    resolve_script(
        gateway_local_data::env_path("GATEWAY_RELEASE_PAYLOAD_ROOT").as_deref(),
        std::env::current_exe().ok().as_deref(),
        Path::new(env!("CARGO_MANIFEST_DIR")),
    )
}

fn log_path_from_values(values: ConsoleConfigValues) -> Result<PathBuf, ConsoleConfigError> {
    Ok(ConsoleConfig::from_values(values)?
        .state_dir
        .join("logs/gemini-canvas-browser-pool.log"))
}

pub(super) fn log_path() -> Result<PathBuf, GatewayError> {
    // Reuse the state owner and its payload exclusion checks, including server defaults.
    // Do not change the child's cwd: explicit relative object storage keeps its meaning.
    log_path_from_values(ConsoleConfigValues {
        state_dir: gateway_local_data::env_path("GATEWAY_STATE_DIR"),
        routes_file: gateway_local_data::env_path("GATEWAY_ROUTES_FILE"),
        release_payload_root: gateway_local_data::env_path("GATEWAY_RELEASE_PAYLOAD_ROOT"),
        ..Default::default()
    })
    .map_err(|error| {
        GatewayError::server_error(format!("Cannot resolve Gemini browser pool log: {error}"))
            .with_provider("gemini_canvas_compatible")
            .with_code("gemini_canvas_browser_pool_log_path_invalid")
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn explicit_payload_is_authoritative_even_when_missing() {
        let root = std::env::temp_dir().join("gemini-pool-payload-not-created");
        assert_eq!(
            resolve_script(Some(&root), None, Path::new(env!("CARGO_MANIFEST_DIR"))),
            root.join(SCRIPT)
        );
        assert!(!root.exists());
    }

    #[test]
    fn packaged_executable_never_falls_back_to_compile_machine() {
        let root = std::env::temp_dir().join("gemini-pool-package-not-created");
        assert_eq!(
            resolve_script(
                None,
                Some(&root.join("gateway.exe")),
                Path::new(env!("CARGO_MANIFEST_DIR"))
            ),
            root.join(SCRIPT)
        );
        assert!(!root.exists());
    }

    #[test]
    fn checkout_target_executable_uses_development_scripts() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR"));
        assert_eq!(
            resolve_script(None, Some(&root.join("target/debug/gateway.exe")), root),
            root.join(SCRIPT)
        );
    }

    #[test]
    fn explicit_state_owns_log_without_creating_directories() {
        let root = std::env::temp_dir().join("gemini-pool-state-not-created");
        let result = log_path_from_values(ConsoleConfigValues {
            state_dir: Some(root.clone()),
            ..Default::default()
        })
        .unwrap();
        assert_eq!(result, root.join("logs/gemini-canvas-browser-pool.log"));
        assert!(!root.exists());
    }

    #[test]
    fn server_route_directory_default_is_preserved() {
        let root = std::env::temp_dir().join("gemini-pool-server-not-created");
        let result = log_path_from_values(ConsoleConfigValues {
            routes_file: Some(root.join("routes.yaml")),
            ..Default::default()
        })
        .unwrap();
        assert_eq!(
            result,
            root.join(".gateway-state/logs/gemini-canvas-browser-pool.log")
        );
        assert!(!root.exists());
    }

    #[test]
    fn immutable_payload_cannot_be_used_as_log_state() {
        let root = std::env::temp_dir().join("gemini-pool-immutable-not-created");
        let result = log_path_from_values(ConsoleConfigValues {
            release_payload_root: Some(root.clone()),
            state_dir: Some(root.join("state")),
            ..Default::default()
        });
        assert!(result.is_err());
        assert!(!root.exists());
    }
}
