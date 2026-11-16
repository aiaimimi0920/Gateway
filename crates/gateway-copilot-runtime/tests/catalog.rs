#![cfg(feature = "test-fixture")]
//! 同一 owned runtime 上显式选择账号；目录不会消费会话名额或触发生成。
use gateway_copilot_runtime::{CopilotRuntime, RuntimeConfig, RuntimeError};
use std::{path::PathBuf, time::Duration};

#[tokio::test]
async fn catalog_is_explicit_uncached_and_preserves_policy_and_unknown_billing() {
    let root = PathBuf::from(std::env::var_os("GATEWAY_PROBE_ROOT").unwrap()).join("catalog");
    std::fs::create_dir_all(root.join("home")).unwrap();
    let mut owner = CopilotRuntime::start(RuntimeConfig {
        executable: PathBuf::from(env!("CARGO_BIN_EXE_synthetic-runtime")),
        directory: root.join("home"),
    })
    .await
    .unwrap();
    let checked = tokio::time::timeout(Duration::from_secs(10), async {
        assert!(matches!(
            owner.list_models_for_token(" \n".into()).await,
            Err(RuntimeError::EmptyCatalogToken)
        ));
        let (a, b) = tokio::join!(
            owner.list_models_for_token("catalog-a".into()),
            owner.list_models_for_token("catalog-b".into())
        );
        let (a, b) = (a.unwrap(), b.unwrap());
        let again = owner
            .list_models_for_token("catalog-a".into())
            .await
            .unwrap();
        assert_eq!(a[0].id, "model-a");
        assert_eq!(b[0].id, "model-b");
        assert_eq!(again[0].id, "model-a");
        assert_eq!(a[0].billing.as_ref().unwrap().multiplier, Some(2.0));
        assert!(b[0].billing.is_none());
        assert_eq!(
            a[0].policy.as_ref().unwrap().terms.as_deref(),
            Some("fixture terms")
        );
        assert_eq!(
            serde_json::to_value(&a[0]).unwrap()["policy"]["state"],
            "disabled"
        );
        assert_eq!(
            a[0].capabilities.limits.as_ref().unwrap().max_output_tokens,
            Some(4096)
        );
        assert!(owner
            .list_models_for_token("catalog-empty".into())
            .await
            .unwrap()
            .is_empty());
        for token in ["catalog-denied", "catalog-malformed"] {
            let error = owner.list_models_for_token(token.into()).await.unwrap_err();
            assert!(matches!(error, RuntimeError::Catalog(_)));
            assert_eq!(error.to_string(), "Copilot model catalog request failed");
        }
        // 目录只读，不预留生成会话，也不替换该会话的明确身份。
        let prepared = owner
            .prepare_text_session("synthetic-token".into(), "synthetic-model".into())
            .unwrap();
        let _events = prepared.subscribe();
        prepared.start().await.unwrap();
    })
    .await;
    let stopped = tokio::time::timeout(Duration::from_secs(40), owner.stop()).await;
    checked.unwrap();
    stopped.unwrap().unwrap();
    let log = std::fs::read_to_string(root.join("rpc.jsonl")).unwrap();
    assert_eq!(log.matches("models.list").count(), 6);
    assert!(!log.contains("catalog-a"));
    assert!(!log.contains("session.send"));
    assert!(root.join("graceful-exit").is_file());
}

#[tokio::test]
async fn canceled_catalog_does_not_replay_or_prevent_runtime_shutdown() {
    let root =
        PathBuf::from(std::env::var_os("GATEWAY_PROBE_ROOT").unwrap()).join("catalog-cancel");
    std::fs::create_dir_all(root.join("home")).unwrap();
    let owner = CopilotRuntime::start(RuntimeConfig {
        executable: PathBuf::from(env!("CARGO_BIN_EXE_synthetic-runtime")),
        directory: root.join("home"),
    })
    .await
    .unwrap();
    let requested = tokio::time::timeout(
        Duration::from_millis(100),
        owner.list_models_for_token("catalog-hang".into()),
    )
    .await;
    let stopped = tokio::time::timeout(Duration::from_secs(40), owner.stop()).await;
    assert!(requested.is_err());
    stopped.unwrap().unwrap();
    let log = std::fs::read_to_string(root.join("rpc.jsonl")).unwrap();
    assert_eq!(log.matches("models.list").count(), 1);
    assert!(!log.contains("session.create"));
    assert!(root.join("graceful-exit").is_file());
}
