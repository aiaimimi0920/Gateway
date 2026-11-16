//! Missing explicit CLI must refund Local quota without starting or discovering another runtime.
use super::*;
use crate::pipeline::{stage_filter, stage_finalize};

#[tokio::test]
#[ignore = "requires explicit nonexistent absolute CLI path and fresh existing probe/workspace roots"]
async fn copilot_prepaid_startup_missing_cli_refunds_without_child() {
    let executable = std::path::PathBuf::from(std::env::var_os("GATEWAY_COPILOT_CLI").unwrap());
    assert!(executable.is_absolute() && !executable.exists());
    let root = std::path::PathBuf::from(std::env::var_os("GATEWAY_PROBE_ROOT").unwrap());
    let workspace =
        std::path::PathBuf::from(std::env::var_os("GATEWAY_COPILOT_WORKSPACE_ROOT").unwrap());
    assert!(workspace.starts_with(&root));
    assert_eq!(std::fs::read_dir(&workspace).unwrap().count(), 0);
    for responses in [false, true] {
        for mode in ["message_prepaid", "token_prepaid"] {
            let mut f = prepaid(mode, 1000, "ok", responses).await;
            stage_filter::run(&mut f.ctx, &f.console.state)
                .await
                .unwrap();
            let reserved = f.ctx.quota_pre_deducted_tokens as i64;
            assert!(reserved > 0);
            assert_eq!(remaining(&f, mode).await, 1000 - reserved);
            crate::cash_billing::admission::prepare(&mut f.ctx, &f.console.state)
                .await
                .unwrap();
            let mut attempt = prepared(&f).await;
            attempt.retry_policy.max_retries = 3;
            let result = recovery_wait::send_cancellable(
                &mut f.ctx,
                &f.console.state,
                &f.candidate,
                0,
                attempt,
            )
            .await;
            let error = match result {
                Err(AttemptError::Stop(error)) => error,
                _ => panic!("missing runtime must fail without substitution"),
            };
            assert_eq!(error.code.as_deref(), Some("copilot_execution_unavailable"));
            assert!(!error.retryable && !crate::request_wait::recoverable(&error));
            assert!(matches!(
                error.fallback_hint,
                crate::error::FallbackHint::Abort { .. }
            ));
            assert!(!format!("{error:?}").contains("synthetic-token"));
            assert_eq!(f.ctx.observed_upstream_status, Some(None));
            stage_finalize::run_failure(&error, &f.ctx, &f.console.state).await;
            assert_eq!(remaining(&f, mode).await, 1000);
            // The gate counts one execution admission, not one confirmed SDK generation.
            audit(&f, "failed", 1).await;
            f.local
                .save_access_key_with_quota(
                    Some(&f.key),
                    fixture::input(json!({"scopes":["relay"]})),
                    Some(&KeyQuotaInput {
                        mode: "unlimited".into(),
                        limit: None,
                        currency: None,
                    }),
                )
                .await
                .unwrap();
            assert!(AccessBalanceStore::Sqlite(&f.local)
                .cash_ledger(&f.key)
                .await
                .unwrap()
                .is_empty());
            finish(f).await;
        }
    }
    assert_eq!(std::fs::read_dir(&workspace).unwrap().count(), 0);
    assert!(!std::fs::read_dir(&root)
        .unwrap()
        .filter_map(Result::ok)
        .any(|entry| entry.file_name().to_string_lossy().starts_with("child-")));
}
