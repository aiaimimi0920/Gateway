use std::path::PathBuf;
use std::sync::OnceLock;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use crate::error::GatewayError;

pub(crate) fn lumalabs_browser_worker_script_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("scripts")
        .join("lumalabs-browser-worker.mjs")
}

pub(crate) fn aistudio_browser_worker_script_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("scripts")
        .join("aistudio-web-browser-worker.mjs")
}

pub(crate) fn gemini_canvas_http_replay_worker_script_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("scripts")
        .join("gemini-canvas-http-replay-worker.mjs")
}

pub(crate) fn producer_browser_worker_script_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("scripts")
        .join("producer-browser-worker.mjs")
}

pub(crate) fn suno_browser_worker_script_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("scripts")
        .join("suno-browser-worker.mjs")
}

pub(crate) fn udio_browser_worker_script_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("scripts")
        .join("udio-browser-worker.mjs")
}

pub(crate) fn udio_browser_request_timeout(
    base_timeout: Duration,
    wait_timeout: Duration,
) -> Duration {
    base_timeout
        .max(Duration::from_secs(300))
        .max(wait_timeout.saturating_add(Duration::from_secs(60)))
}

pub(crate) fn gemini_canvas_browser_pool_script_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("scripts")
        .join("gemini-canvas-browser-pool.mjs")
}

pub(crate) fn gemini_canvas_browser_pool_log_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join(".runtime")
        .join("gemini-canvas-browser-pool.log")
}

pub(crate) fn gemini_canvas_browser_pool_base_url() -> String {
    if let Some(explicit) = std::env::var("GEMINI_CANVAS_BROWSER_POOL_BASE_URL")
        .ok()
        .map(|value| value.trim().trim_end_matches('/').to_string())
        .filter(|value| !value.is_empty())
    {
        return explicit;
    }
    let port = std::env::var("GEMINI_CANVAS_BROWSER_POOL_PORT")
        .ok()
        .and_then(|value| value.parse::<u16>().ok())
        .unwrap_or(42321);
    format!("http://127.0.0.1:{port}")
}

static GEMINI_CANVAS_BROWSER_POOL_START_MUTEX: OnceLock<tokio::sync::Mutex<()>> = OnceLock::new();

pub(crate) fn gemini_canvas_browser_pool_start_mutex() -> &'static tokio::sync::Mutex<()> {
    GEMINI_CANVAS_BROWSER_POOL_START_MUTEX.get_or_init(|| tokio::sync::Mutex::new(()))
}

pub(crate) fn gemini_canvas_browser_pool_log_open_error(
    log_path: &std::path::Path,
    error: &str,
) -> GatewayError {
    GatewayError::server_error(format!(
        "Failed to open Gemini Canvas browser pool log at {}: {error}",
        log_path.display()
    ))
    .with_provider("gemini_canvas_compatible")
    .with_code("gemini_canvas_browser_pool_log_open_failed")
}

pub(crate) fn gemini_canvas_browser_pool_spawn_failed_error(
    script_path: &std::path::Path,
    error: &str,
) -> GatewayError {
    GatewayError::server_error(format!(
        "Failed to launch Gemini Canvas browser pool at {}: {error}",
        script_path.display()
    ))
    .with_provider("gemini_canvas_compatible")
    .with_code("gemini_canvas_browser_pool_spawn_failed")
}

pub(crate) fn gemini_canvas_browser_pool_start_timeout_error(
    log_path: &std::path::Path,
) -> GatewayError {
    GatewayError::server_error(format!(
        "Gemini Canvas browser pool did not become healthy in time. Inspect {} for startup details.",
        log_path.display()
    ))
    .with_provider("gemini_canvas_compatible")
    .with_code("gemini_canvas_browser_pool_start_timeout")
}

pub(crate) fn maybe_dump_gemini_canvas_http_replay_worker_debug(
    stage: &str,
    operation: &str,
    body: &[u8],
) {
    let Some(root) = std::env::var_os("GEMINI_CANVAS_HTTP_REPLAY_WORKER_DEBUG_DIR") else {
        return;
    };
    let mut dir = PathBuf::from(root);
    if std::fs::create_dir_all(&dir).is_err() {
        return;
    }
    let millis = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|value| value.as_millis())
        .unwrap_or(0);
    let op = operation
        .chars()
        .map(|value| {
            if value.is_ascii_alphanumeric() || value == '-' || value == '_' {
                value
            } else {
                '_'
            }
        })
        .collect::<String>();
    dir.push(format!("{millis}-{op}-{stage}.json"));
    let _ = std::fs::write(dir, body);
}

pub(crate) fn enrich_browser_worker_message(message: String, body: Option<String>) -> String {
    let normalized_message = message.trim().to_string();
    let Some(body) = body
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
    else {
        return normalized_message;
    };
    if body == normalized_message {
        return normalized_message;
    }
    let truncated_body = if body.len() > 2000 {
        format!("{}...(truncated)", &body[..2000])
    } else {
        body
    };
    format!("{normalized_message} upstream body: {truncated_body}")
}

pub(crate) fn gemini_canvas_http_replay_worker_wait_failed_error(error: &str) -> GatewayError {
    GatewayError::server_error(format!(
        "Gemini Canvas HTTP replay worker failed before producing output: {error}"
    ))
    .with_provider("gemini_canvas_compatible")
    .with_code("gemini_canvas_http_replay_worker_wait_failed")
}

pub(crate) fn gemini_canvas_http_replay_worker_timeout_error() -> GatewayError {
    GatewayError::server_error(
        "Gemini Canvas HTTP replay worker timed out before producing output.",
    )
    .with_provider("gemini_canvas_compatible")
    .with_code("gemini_canvas_http_replay_worker_timeout")
}

pub(crate) fn gemini_canvas_http_replay_worker_empty_output_error(stderr: &str) -> GatewayError {
    GatewayError::server_error(format!(
        "Gemini Canvas HTTP replay worker did not return JSON output. stderr: {}",
        if stderr.is_empty() { "<empty>" } else { stderr }
    ))
    .with_provider("gemini_canvas_compatible")
    .with_code("gemini_canvas_http_replay_worker_empty_output")
}

pub(crate) fn gemini_canvas_http_replay_worker_output_parse_error(
    error: &str,
    stdout: &str,
) -> GatewayError {
    GatewayError::server_error(format!(
        "Failed to parse Gemini Canvas HTTP replay worker output: {error}. stdout: {stdout}"
    ))
    .with_provider("gemini_canvas_compatible")
    .with_code("gemini_canvas_http_replay_worker_output_parse_failed")
}

pub(crate) fn gemini_canvas_http_replay_worker_input_serialize_error(error: &str) -> GatewayError {
    GatewayError::server_error(format!(
        "Failed to serialize Gemini Canvas HTTP replay worker input: {error}"
    ))
    .with_provider("gemini_canvas_compatible")
    .with_code("gemini_canvas_http_replay_worker_input_serialize_failed")
}

pub(crate) fn gemini_canvas_http_replay_worker_stdin_error(error: &str) -> GatewayError {
    GatewayError::server_error(format!(
        "Failed to write Gemini Canvas HTTP replay worker input: {error}"
    ))
    .with_provider("gemini_canvas_compatible")
    .with_code("gemini_canvas_http_replay_worker_stdin_failed")
}

pub(crate) fn gemini_canvas_http_replay_worker_spawn_failed_error(
    script_path: &std::path::Path,
    error: &str,
) -> GatewayError {
    GatewayError::server_error(format!(
        "Failed to launch Gemini Canvas HTTP replay worker at {}: {error}",
        script_path.display()
    ))
    .with_provider("gemini_canvas_compatible")
    .with_code("gemini_canvas_http_replay_worker_spawn_failed")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_debug_dir() -> std::path::PathBuf {
        std::env::temp_dir().join(format!(
            "neuro-gw-http-replay-debug-{}",
            uuid::Uuid::new_v4().simple()
        ))
    }

    fn assert_script_checked_in(script_path: PathBuf, label: &str) {
        assert!(script_path.exists(), "{label} at {}", script_path.display());
    }

    #[test]
    fn gemini_canvas_http_replay_worker_wait_failed_error_formats_cause() {
        let err = gemini_canvas_http_replay_worker_wait_failed_error("The pipe has been ended");
        assert_eq!(err.http_status, Some(500));
        assert_eq!(
            err.provider_name.as_deref(),
            Some("gemini_canvas_compatible")
        );
        assert_eq!(
            err.code.as_deref(),
            Some("gemini_canvas_http_replay_worker_wait_failed")
        );
        assert_eq!(
            err.message.as_str(),
            "Gemini Canvas HTTP replay worker failed before producing output: The pipe has been ended"
        );
    }

    #[test]
    fn gemini_canvas_http_replay_worker_empty_output_error_formats_stderr_fallback() {
        let with_stderr = gemini_canvas_http_replay_worker_empty_output_error("permission denied");
        assert_eq!(with_stderr.http_status, Some(500));
        assert_eq!(
            with_stderr.provider_name.as_deref(),
            Some("gemini_canvas_compatible")
        );
        assert_eq!(
            with_stderr.code.as_deref(),
            Some("gemini_canvas_http_replay_worker_empty_output")
        );
        assert_eq!(
            with_stderr.message.as_str(),
            "Gemini Canvas HTTP replay worker did not return JSON output. stderr: permission denied"
        );

        let empty = gemini_canvas_http_replay_worker_empty_output_error("");
        assert_eq!(
            empty.message.as_str(),
            "Gemini Canvas HTTP replay worker did not return JSON output. stderr: <empty>"
        );
    }

    #[test]
    fn gemini_canvas_http_replay_worker_output_parse_error_formats_error_and_stdout() {
        let err =
            gemini_canvas_http_replay_worker_output_parse_error("expected value", "{\"oops\":");
        assert_eq!(err.http_status, Some(500));
        assert_eq!(
            err.provider_name.as_deref(),
            Some("gemini_canvas_compatible")
        );
        assert_eq!(
            err.code.as_deref(),
            Some("gemini_canvas_http_replay_worker_output_parse_failed")
        );
        assert_eq!(
            err.message.as_str(),
            "Failed to parse Gemini Canvas HTTP replay worker output: expected value. stdout: {\"oops\":"
        );
    }

    #[test]
    fn gemini_canvas_http_replay_worker_timeout_error_matches_contract() {
        let err = gemini_canvas_http_replay_worker_timeout_error();
        assert_eq!(err.http_status, Some(500));
        assert_eq!(
            err.provider_name.as_deref(),
            Some("gemini_canvas_compatible")
        );
        assert_eq!(
            err.code.as_deref(),
            Some("gemini_canvas_http_replay_worker_timeout")
        );
        assert_eq!(
            err.message.as_str(),
            "Gemini Canvas HTTP replay worker timed out before producing output."
        );
    }

    #[test]
    fn gemini_canvas_http_replay_worker_input_serialize_error_formats_cause() {
        let err = gemini_canvas_http_replay_worker_input_serialize_error("missing field `url`");
        assert_eq!(err.http_status, Some(500));
        assert_eq!(
            err.provider_name.as_deref(),
            Some("gemini_canvas_compatible")
        );
        assert_eq!(
            err.code.as_deref(),
            Some("gemini_canvas_http_replay_worker_input_serialize_failed")
        );
        assert_eq!(
            err.message.as_str(),
            "Failed to serialize Gemini Canvas HTTP replay worker input: missing field `url`"
        );
    }

    #[test]
    fn gemini_canvas_http_replay_worker_stdin_error_formats_cause() {
        let err = gemini_canvas_http_replay_worker_stdin_error("broken pipe");
        assert_eq!(err.http_status, Some(500));
        assert_eq!(
            err.provider_name.as_deref(),
            Some("gemini_canvas_compatible")
        );
        assert_eq!(
            err.code.as_deref(),
            Some("gemini_canvas_http_replay_worker_stdin_failed")
        );
        assert_eq!(
            err.message.as_str(),
            "Failed to write Gemini Canvas HTTP replay worker input: broken pipe"
        );
    }

    #[test]
    fn gemini_canvas_http_replay_worker_spawn_failed_error_formats_script_path_and_cause() {
        let err = gemini_canvas_http_replay_worker_spawn_failed_error(
            std::path::Path::new("C:/tmp/gemini-canvas-http-replay-worker.mjs"),
            "The system cannot find the file specified. (os error 2)",
        );
        assert_eq!(err.http_status, Some(500));
        assert_eq!(
            err.provider_name.as_deref(),
            Some("gemini_canvas_compatible")
        );
        assert_eq!(
            err.code.as_deref(),
            Some("gemini_canvas_http_replay_worker_spawn_failed")
        );
        assert_eq!(
            err.message.as_str(),
            "Failed to launch Gemini Canvas HTTP replay worker at C:/tmp/gemini-canvas-http-replay-worker.mjs: The system cannot find the file specified. (os error 2)"
        );
    }

    #[test]
    fn gemini_canvas_browser_pool_log_open_error_formats_path_and_cause() {
        let err = gemini_canvas_browser_pool_log_open_error(
            std::path::Path::new("C:/tmp/gemini-canvas-browser-pool.log"),
            "Access is denied. (os error 5)",
        );
        assert_eq!(err.http_status, Some(500));
        assert_eq!(
            err.provider_name.as_deref(),
            Some("gemini_canvas_compatible")
        );
        assert_eq!(
            err.code.as_deref(),
            Some("gemini_canvas_browser_pool_log_open_failed")
        );
        assert_eq!(
            err.message.as_str(),
            "Failed to open Gemini Canvas browser pool log at C:/tmp/gemini-canvas-browser-pool.log: Access is denied. (os error 5)"
        );
    }

    #[test]
    fn gemini_canvas_browser_pool_spawn_failed_error_formats_path_and_cause() {
        let err = gemini_canvas_browser_pool_spawn_failed_error(
            std::path::Path::new("C:/tmp/gemini-canvas-browser-pool.mjs"),
            "The system cannot find the file specified. (os error 2)",
        );
        assert_eq!(err.http_status, Some(500));
        assert_eq!(
            err.provider_name.as_deref(),
            Some("gemini_canvas_compatible")
        );
        assert_eq!(
            err.code.as_deref(),
            Some("gemini_canvas_browser_pool_spawn_failed")
        );
        assert_eq!(
            err.message.as_str(),
            "Failed to launch Gemini Canvas browser pool at C:/tmp/gemini-canvas-browser-pool.mjs: The system cannot find the file specified. (os error 2)"
        );
    }

    #[test]
    fn gemini_canvas_browser_pool_start_timeout_error_formats_log_path() {
        let err = gemini_canvas_browser_pool_start_timeout_error(std::path::Path::new(
            "C:/tmp/gemini-canvas-browser-pool.log",
        ));
        assert_eq!(err.http_status, Some(500));
        assert_eq!(
            err.provider_name.as_deref(),
            Some("gemini_canvas_compatible")
        );
        assert_eq!(
            err.code.as_deref(),
            Some("gemini_canvas_browser_pool_start_timeout")
        );
        assert_eq!(
            err.message.as_str(),
            "Gemini Canvas browser pool did not become healthy in time. Inspect C:/tmp/gemini-canvas-browser-pool.log for startup details."
        );
    }

    #[test]
    fn enrich_browser_worker_message_appends_truncated_body_when_distinct() {
        let long_body = "x".repeat(2205);
        let enriched = enrich_browser_worker_message("worker failed".to_string(), Some(long_body));

        assert!(enriched.starts_with("worker failed upstream body: "));
        assert!(enriched.ends_with("...(truncated)"));

        let same = enrich_browser_worker_message(
            "same message".to_string(),
            Some("same message".to_string()),
        );
        assert_eq!(same, "same message");
    }

    #[test]
    fn maybe_dump_gemini_canvas_http_replay_worker_debug_writes_body_when_env_set() {
        let debug_dir = make_debug_dir();
        let debug_dir_string = debug_dir.to_string_lossy().to_string();
        unsafe {
            std::env::set_var(
                "GEMINI_CANVAS_HTTP_REPLAY_WORKER_DEBUG_DIR",
                &debug_dir_string,
            );
        }

        maybe_dump_gemini_canvas_http_replay_worker_debug(
            "response",
            "image/edit",
            br#"{"ok":true}"#,
        );

        let entries = std::fs::read_dir(&debug_dir)
            .expect("debug dir exists")
            .collect::<Result<Vec<_>, _>>()
            .expect("list debug dir");
        assert_eq!(entries.len(), 1);
        let file_name = entries[0].file_name().to_string_lossy().to_string();
        assert!(file_name.contains("-image_edit-response.json"));
        let body = std::fs::read(entries[0].path()).expect("read dumped body");
        assert_eq!(body, br#"{"ok":true}"#);

        unsafe {
            std::env::remove_var("GEMINI_CANVAS_HTTP_REPLAY_WORKER_DEBUG_DIR");
        }
        let _ = std::fs::remove_file(entries[0].path());
        let _ = std::fs::remove_dir(&debug_dir);
    }

    #[test]
    fn gemini_canvas_browser_pool_start_mutex_returns_stable_mutex_instance() {
        let first = gemini_canvas_browser_pool_start_mutex() as *const _;
        let second = gemini_canvas_browser_pool_start_mutex() as *const _;

        assert_eq!(first, second);
    }

    #[test]
    fn lumalabs_browser_worker_script_is_checked_in() {
        assert_script_checked_in(
            lumalabs_browser_worker_script_path(),
            "expected lumalabs browser worker script",
        );
    }

    #[test]
    fn aistudio_browser_worker_script_is_checked_in() {
        assert_script_checked_in(
            aistudio_browser_worker_script_path(),
            "expected AI Studio browser worker script",
        );
    }

    #[test]
    fn gemini_canvas_browser_pool_script_is_checked_in() {
        assert_script_checked_in(
            gemini_canvas_browser_pool_script_path(),
            "expected gemini canvas browser pool script",
        );
    }

    #[test]
    fn gemini_canvas_http_replay_worker_script_is_checked_in() {
        assert_script_checked_in(
            gemini_canvas_http_replay_worker_script_path(),
            "expected gemini canvas HTTP replay worker script",
        );
    }

    #[test]
    fn producer_browser_worker_script_is_checked_in() {
        assert_script_checked_in(
            producer_browser_worker_script_path(),
            "expected producer browser worker script",
        );
    }

    #[test]
    fn suno_browser_worker_script_is_checked_in() {
        assert_script_checked_in(
            suno_browser_worker_script_path(),
            "expected suno browser worker script",
        );
    }

    #[test]
    fn udio_browser_worker_script_is_checked_in() {
        assert_script_checked_in(
            udio_browser_worker_script_path(),
            "expected udio browser worker script",
        );
    }

    #[test]
    fn udio_browser_request_timeout_respects_wait_timeout_budget() {
        let timeout =
            udio_browser_request_timeout(Duration::from_secs(60), Duration::from_secs(480));
        assert_eq!(timeout, Duration::from_secs(540));
    }

    #[test]
    fn udio_browser_request_timeout_keeps_minimum_budget() {
        let timeout =
            udio_browser_request_timeout(Duration::from_secs(30), Duration::from_secs(240));
        assert_eq!(timeout, Duration::from_secs(300));
    }
}
