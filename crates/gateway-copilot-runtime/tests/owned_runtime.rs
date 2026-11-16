#![cfg(feature = "test-fixture")]
//! 合成 child 经生产 owner；测试目录由 runner 显式指向 linshi。
use gateway_copilot_runtime::{CopilotRuntime, RuntimeConfig, RuntimeError};
use std::{path::PathBuf, time::Duration};

#[tokio::test]
async fn owner_enforces_explicit_identity_single_session_and_graceful_stop() {
    let root =
        PathBuf::from(std::env::var_os("GATEWAY_PROBE_ROOT").expect("isolated test directory"));
    std::fs::create_dir_all(root.join("home")).unwrap();
    let mut owner = CopilotRuntime::start(RuntimeConfig {
        executable: PathBuf::from(env!("CARGO_BIN_EXE_synthetic-runtime")),
        directory: root.join("home"),
    })
    .await
    .unwrap();
    assert!(matches!(
        owner.prepare_text_session(String::new(), "synthetic-model".into()),
        Err(RuntimeError::EmptyIdentity)
    ));
    let prepared = owner
        .prepare_text_session("synthetic-token".into(), "synthetic-model".into())
        .unwrap();
    assert!(matches!(
        owner.prepare_text_session("another-token".into(), "other-model".into()),
        Err(RuntimeError::SessionReserved)
    ));
    let _events = prepared.subscribe();
    let session = tokio::time::timeout(Duration::from_secs(5), prepared.start()).await;
    // 先回收再断言结果，避免失败路径把实验进程留在后台。
    let stopped = tokio::time::timeout(Duration::from_secs(40), owner.stop()).await;
    session.unwrap().unwrap();
    stopped.unwrap().unwrap();
    assert!(root.join("graceful-exit").is_file());
    let options: serde_json::Value =
        serde_json::from_slice(&std::fs::read(root.join("options.json")).unwrap()).unwrap();
    assert_eq!(options["includedBuiltinSkills"], serde_json::json!([]));
    assert_eq!(options["installedPlugins"], serde_json::json!([]));
}

#[tokio::test]
async fn missing_program_fails_before_sdk_discovery() {
    let root =
        PathBuf::from(std::env::var_os("GATEWAY_PROBE_ROOT").expect("isolated test directory"));
    let result = CopilotRuntime::start(RuntimeConfig {
        executable: root.join("not-installed.exe"),
        directory: root,
    })
    .await;
    assert!(matches!(result, Err(RuntimeError::Configuration(_))));
}

#[tokio::test]
async fn dropping_owner_stops_child_without_waiting_for_a_session() {
    let root =
        PathBuf::from(std::env::var_os("GATEWAY_PROBE_ROOT").expect("isolated test directory"))
            .join("drop");
    std::fs::create_dir_all(root.join("home")).unwrap();
    let owner = CopilotRuntime::start(RuntimeConfig {
        executable: PathBuf::from(env!("CARGO_BIN_EXE_synthetic-runtime")),
        directory: root.join("home"),
    })
    .await
    .unwrap();
    assert!(root.join("child.pid").is_file());
    drop(owner);
    // runner 通过 Windows 句柄检查这里记录的 PID，不能只把 Drop 返回当退出证明。
}
