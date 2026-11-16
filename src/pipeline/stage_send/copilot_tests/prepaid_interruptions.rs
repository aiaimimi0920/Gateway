//! Each cancellation runs in its own process because the production text slot stays quarantined.
use super::*;
use crate::pipeline::{stage_filter, stage_finalize};
use std::{path::Path, time::Duration};

fn sends(root: &Path) -> usize {
    std::fs::read_dir(root)
        .unwrap()
        .filter_map(Result::ok)
        .filter(|entry| entry.file_name().to_string_lossy().ends_with("-rpc.jsonl"))
        .map(|entry| {
            std::fs::read_to_string(entry.path())
                .unwrap()
                .matches("session.sendAndWait")
                .count()
        })
        .sum()
}

async fn cancelled(mode: &str, responses: bool) {
    let root = std::path::PathBuf::from(std::env::var_os("GATEWAY_PROBE_ROOT").unwrap());
    assert_eq!(
        sends(&root),
        0,
        "requires a fresh probe root and independent test process"
    );
    let mut f = prepaid(mode, 1000, "hang", responses).await;
    stage_filter::run(&mut f.ctx, &f.console.state)
        .await
        .unwrap();
    let reserved = f.ctx.quota_pre_deducted_tokens as i64;
    assert!(reserved > 0);
    assert_eq!(remaining(&f, mode).await, 1000 - reserved);
    crate::cash_billing::admission::prepare(&mut f.ctx, &f.console.state)
        .await
        .unwrap();
    assert!(f.ctx.cash_charge.is_none());
    let (cancel, receiver) = tokio::sync::watch::channel(false);
    f.ctx.caller_cancel = Some(receiver);
    let attempt = prepared(&f).await;
    let execution =
        recovery_wait::send_cancellable(&mut f.ctx, &f.console.state, &f.candidate, 0, attempt);
    let interrupt = async {
        while sends(&root) == 0 {
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
        cancel.send(true).unwrap();
    };
    let (result, ()) = tokio::time::timeout(Duration::from_secs(30), async {
        tokio::join!(execution, interrupt)
    })
    .await
    .expect("cancel must not wait for the generation timeout");
    let error = match result {
        Err(AttemptError::Stop(error)) => error,
        _ => panic!("cancelled prepaid request must stop without fallback"),
    };
    assert_eq!(
        f.ctx.request_budget.stopped_reason(),
        Some("caller_cancelled")
    );
    assert_eq!(f.ctx.observed_upstream_status, Some(None));
    assert!(!error.retryable && !crate::request_wait::recoverable(&error));
    assert!(!format!("{error:?}").contains("synthetic-token"));
    stage_finalize::run_failure(&error, &f.ctx, &f.console.state).await;
    assert_eq!(remaining(&f, mode).await, 1000);
    audit(&f, "cancelled", 1).await;
    assert_eq!(sends(&root), 1);
    assert!(AccessBalanceStore::Sqlite(&f.local)
        .cash_ledger(&f.key)
        .await
        .unwrap()
        .is_empty());

    // A quota mode edit is rejected while Local holds are outstanding. Its success proves release.
    f.local
        .save_access_key_with_quota(
            Some(&f.key),
            fixture::input(
                json!({"scopes":["relay"],"models":["public-alias"],"providerIds":["copilot"]}),
            ),
            Some(&KeyQuotaInput {
                mode: "unlimited".into(),
                limit: None,
                currency: None,
            }),
        )
        .await
        .unwrap();
    assert_eq!(
        AccessBalanceStore::Sqlite(&f.local)
            .get(&f.key)
            .await
            .unwrap()
            .unwrap()
            .balance_mode,
        "unlimited"
    );
    assert_eq!(
        std::fs::read_dir(std::env::var_os("GATEWAY_COPILOT_WORKSPACE_ROOT").unwrap())
            .unwrap()
            .count(),
        1
    );
    finish(f).await;
}

#[tokio::test]
#[ignore = "run alone with synthetic CLI and fresh roots; permanently quarantines process text slot"]
async fn copilot_prepaid_cancellation_message_chat_refunds() {
    cancelled("message_prepaid", false).await;
}

#[tokio::test]
#[ignore = "run alone with synthetic CLI and fresh roots; permanently quarantines process text slot"]
async fn copilot_prepaid_cancellation_token_responses_refunds() {
    cancelled("token_prepaid", true).await;
}
