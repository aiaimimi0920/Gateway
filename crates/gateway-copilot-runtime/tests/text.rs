#![cfg(feature = "test-fixture")]
use gateway_copilot_runtime::{CopilotRuntime, RuntimeConfig, TextError};
use std::{path::PathBuf, time::Duration};

async fn start_owner(name: &str) -> (CopilotRuntime, PathBuf) {
    let root = PathBuf::from(std::env::var_os("GATEWAY_PROBE_ROOT").unwrap()).join(name);
    std::fs::create_dir_all(root.join("home")).unwrap();
    let owner = CopilotRuntime::start(RuntimeConfig {
        executable: PathBuf::from(env!("CARGO_BIN_EXE_synthetic-runtime")),
        directory: root.join("home"),
    })
    .await
    .unwrap();
    (owner, root)
}

fn requests(root: &std::path::Path) -> String {
    std::fs::read_to_string(root.join("rpc.jsonl")).unwrap()
}

#[tokio::test]
async fn text_completion_preserves_unknown_usage_and_waits_for_shutdown() {
    for (prompt, count) in [("ok", None), ("known-usage", Some(3))] {
        let (owner, root) = start_owner(prompt).await;
        let result = tokio::time::timeout(
            Duration::from_secs(45),
            owner.complete_text(
                "synthetic-token".into(),
                "synthetic-model".into(),
                prompt.into(),
                Duration::from_secs(2),
            ),
        )
        .await
        .unwrap()
        .unwrap();
        assert_eq!(result.text, "synthetic answer");
        assert_eq!(result.final_message_output_tokens, count);
        assert!(root.join("graceful-exit").is_file());
        assert_eq!(requests(&root).matches("session.sendAndWait").count(), 1);
    }
}

#[tokio::test]
async fn text_rejects_failed_missing_unsupported_and_secret_results_without_replay() {
    for prompt in [
        "denied",
        "missing",
        "aborted",
        "wrong-agent",
        "wrong-type",
        "malformed",
        "multipart",
        "tool",
        "echo",
        "oversize",
        "negative",
    ] {
        let (owner, root) = start_owner(prompt).await;
        let error = tokio::time::timeout(
            Duration::from_secs(45),
            owner.complete_text(
                "synthetic-token".into(),
                "synthetic-model".into(),
                prompt.into(),
                Duration::from_secs(2),
            ),
        )
        .await
        .unwrap()
        .unwrap_err();
        assert!(!error.to_string().contains("synthetic-token"));
        assert!(root.join("graceful-exit").is_file(), "{prompt}: {error}");
        assert_eq!(requests(&root).matches("session.sendAndWait").count(), 1);
        assert!(!requests(&root).contains("\"session.send\""));
    }
}

#[tokio::test]
async fn text_invalid_input_never_creates_or_sends_a_session() {
    for (name, prompt) in [
        ("blank", String::new()),
        ("large", "x".repeat(64 * 1024 + 1)),
    ] {
        let (owner, root) = start_owner(name).await;
        let error = owner
            .complete_text(
                "synthetic-token".into(),
                "synthetic-model".into(),
                prompt,
                Duration::from_secs(2),
            )
            .await
            .unwrap_err();
        assert!(matches!(error, TextError::InvalidRequest));
        assert!(!requests(&root).contains("session.create"));
        assert!(root.join("graceful-exit").is_file());
    }
}

#[tokio::test]
async fn text_timeout_and_external_cancel_relinquish_owned_child() {
    let (owner, root) = start_owner("text-timeout").await;
    let result = owner
        .complete_text(
            "synthetic-token".into(),
            "synthetic-model".into(),
            "hang".into(),
            Duration::from_millis(100),
        )
        .await;
    assert!(result.is_err());
    assert!(root.join("graceful-exit").is_file());
    assert_eq!(requests(&root).matches("session.sendAndWait").count(), 1);

    let (owner, root) = start_owner("text-cancel").await;
    assert!(tokio::time::timeout(
        Duration::from_millis(150),
        owner.complete_text(
            "synthetic-token".into(),
            "synthetic-model".into(),
            "hang".into(),
            Duration::from_secs(30)
        )
    )
    .await
    .is_err());
    assert_eq!(requests(&root).matches("session.sendAndWait").count(), 1);
    // Runner checks the recorded PID through Windows process handles; Drop alone is not proof.
}
