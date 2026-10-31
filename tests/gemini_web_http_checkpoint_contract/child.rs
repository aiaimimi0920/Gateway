//! No global environment mutation in the test parent; a fresh OS process per request.
use super::*;
use base64::{engine::general_purpose::STANDARD, Engine};

pub fn directory() -> std::path::PathBuf {
    #[cfg(windows)]
    let root = std::path::PathBuf::from(
        "C:/Users/Public/nas_home/AI/GameEditor/linshi/gateway-gemini-dual-mode-20261030-r1/r17",
    );
    #[cfg(not(windows))]
    let root = std::env::temp_dir().join("gateway-gemini-http-checkpoint-tests");
    let dir = root.join(format!("http-checkpoint-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&dir).unwrap();
    dir
}
pub async fn invoke(
    base: &str,
    directory: &std::path::Path,
    mode: &str,
    seed: &str,
    expected: &str,
) {
    let result = tokio::process::Command::new(std::env::current_exe().unwrap())
        .args(["--exact", "checkpoint_child", "--nocapture"])
        .env("GATEWAY_TEST_HTTP_CHECKPOINT_BASE", base)
        .env("GATEWAY_TEST_HTTP_CHECKPOINT_MODE", mode)
        .env("GATEWAY_TEST_HTTP_CHECKPOINT_SEED", seed)
        .env("GATEWAY_TEST_HTTP_CHECKPOINT_EXPECTED", expected)
        .env(
            "GEMINI_WEB_HTTP_AUTH_CHECKPOINT_DRIVER",
            if mode == "disabled" {
                "disabled"
            } else {
                "local"
            },
        )
        .env("GEMINI_WEB_HTTP_AUTH_CHECKPOINT_DIR", directory)
        .env(
            "GEMINI_WEB_HTTP_AUTH_CHECKPOINT_KEY",
            if mode == "invalid-config" {
                "invalid".into()
            } else {
                STANDARD.encode([7u8; 32])
            },
        )
        .kill_on_drop(true)
        .output()
        .await
        .unwrap();
    assert!(
        result.status.success(),
        "child failed: {} {}",
        String::from_utf8_lossy(&result.stdout),
        String::from_utf8_lossy(&result.stderr)
    );
}

pub async fn run() {
    let Ok(base) = std::env::var("GATEWAY_TEST_HTTP_CHECKPOINT_BASE") else {
        return;
    };
    let mode = std::env::var("GATEWAY_TEST_HTTP_CHECKPOINT_MODE").unwrap();
    let seed = std::env::var("GATEWAY_TEST_HTTP_CHECKPOINT_SEED").unwrap();
    let payload: ProviderAccountPayload = serde_json::from_value(json!({
        "adapter":"gemini_web_compatible", "base_url":base, "api_key":"",
        "credential_id":"synthetic-checkpoint-account", "chat_completions_path":"/generate",
        "session_auth":{"transport":"cookie", "primaryCookieName":"synthetic-primary"},
        "headers":{"cookie":format!("pref={seed}")}, "extra_body":{"pureHttpMode":"required"}
    }))
    .unwrap();
    let http = neuro_gateway::http_client::builder()
        .no_proxy()
        .build()
        .unwrap();
    let text =
        normalize_chat_completions(json!({"model":"gemini-web-chat", "stream":mode=="stream",
        "messages":[{"role":"user","content":"synthetic"}]}))
        .unwrap();
    let result = match mode.as_str() {
        "speech" => {
            let req = normalize_audio_speech(
                json!({"model":"gemini-web-chat", "input":"synthetic", "response_format":"wav"}),
            )
            .unwrap();
            web_reverse::execute_speech(
                &http,
                Duration::from_secs(2),
                &payload,
                &req,
                "gemini-web-chat",
                None,
            )
            .await
            .map(|_| ())
        }
        "stream" => web_reverse::execute_stream(
            &http,
            Duration::from_secs(2),
            &payload,
            &text,
            "gemini-web-chat",
            None,
        )
        .await
        .map(|_| ()),
        _ => web_reverse::execute(
            &http,
            Duration::from_secs(2),
            &payload,
            &text,
            "gemini-web-chat",
            None,
        )
        .await
        .map(|_| ()),
    };
    let expected = std::env::var("GATEWAY_TEST_HTTP_CHECKPOINT_EXPECTED").unwrap();
    if expected.is_empty() {
        result.unwrap();
    } else if expected == "*" {
        assert!(result.is_err());
    } else {
        assert_eq!(result.unwrap_err().code.as_deref(), Some(expected.as_str()));
    }
}
