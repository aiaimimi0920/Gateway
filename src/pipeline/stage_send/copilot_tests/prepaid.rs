//! Real Local quota admission/finalization around the trusted internal SDK sender.
use super::*;
use crate::access_balance::{quota::KeyQuotaInput, AccessBalanceStore};
#[path = "prepaid_interruptions.rs"]
mod interruptions;
#[path = "prepaid_startup.rs"]
mod startup;

async fn prepaid(mode: &str, limit: i64, prompt: &str, responses: bool) -> fixture::Fixture {
    let mut f = configured().await;
    f.local
        .save_access_key_with_quota(
            Some(&f.key),
            fixture::input(
                json!({"scopes":["relay"], "models":["public-alias"], "providerIds":["copilot"]}),
            ),
            Some(&KeyQuotaInput {
                mode: mode.into(),
                limit: Some(limit),
                currency: (mode == "cash_prepaid").then(|| "USD".into()),
            }),
        )
        .await
        .unwrap();
    f.ctx.requesting_access_key_id = Some(f.key.clone());
    f.ctx.candidates = vec![f.candidate.clone()];
    f.ctx.canonical_req = if responses {
        crate::protocol::responses::normalize_responses(
            json!({"model":"public-alias", "input":prompt}),
        )
        .unwrap()
    } else {
        crate::protocol::openai::normalize_chat_completions(json!({
            "model":"public-alias", "messages":[{"role":"user","content":prompt}]
        }))
        .unwrap()
    };
    super::super::super::stage_finalize::begin_request_audit(&mut f.ctx, &f.console.state).await;
    f
}

async fn remaining(f: &fixture::Fixture, mode: &str) -> i64 {
    let balance = AccessBalanceStore::Sqlite(&f.local)
        .get(&f.key)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(balance.balance_mode, mode);
    if mode == "token_prepaid" {
        balance.remaining_tokens.unwrap()
    } else {
        balance.remaining_messages.unwrap()
    }
}

async fn audit(f: &fixture::Fixture, status: &str, attempts: u32) {
    let rows = f
        .local
        .list_audits(&crate::db::RequestAuditFilters::default())
        .await
        .unwrap();
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].status, status);
    assert_eq!(rows[0].access_key_id.as_deref(), Some(f.key.as_str()));
    assert_eq!(
        rows[0].route_trace.as_ref().unwrap()["routeAttemptCount"],
        attempts
    );
    assert!(rows[0].upstream_status.is_none());
    assert!(
        rows[0].prompt_tokens.is_none()
            && rows[0].completion_tokens.is_none()
            && rows[0].total_tokens.is_none()
    );
}

#[tokio::test]
#[ignore = "requires synthetic CLI with isolated probe/workspace roots; run serially"]
async fn copilot_prepaid_message_and_token_success_and_denial() {
    for responses in [false, true] {
        for mode in ["message_prepaid", "token_prepaid"] {
            for success in [true, false] {
                let mut f = prepaid(
                    mode,
                    1000,
                    if success { "known-usage" } else { "denied" },
                    responses,
                )
                .await;
                super::super::super::stage_filter::run(&mut f.ctx, &f.console.state)
                    .await
                    .unwrap();
                let reserved = f.ctx.quota_pre_deducted_tokens as i64;
                assert!(reserved > 0);
                assert_eq!(remaining(&f, mode).await, 1000 - reserved);
                crate::cash_billing::admission::prepare(&mut f.ctx, &f.console.state)
                    .await
                    .unwrap();
                assert!(f.ctx.cash_charge.is_none());
                let attempt = prepared(&f).await;
                let result = recovery_wait::send_cancellable(
                    &mut f.ctx,
                    &f.console.state,
                    &f.candidate,
                    0,
                    attempt,
                )
                .await;
                match result {
                    Ok(PipelineOutput::Json(body)) if success => {
                        assert!(body.get("usage").is_none());
                        assert!(matches!(f.ctx.observed_usage, Some(None)));
                        super::super::super::stage_finalize::run(
                            Some(&body),
                            &f.ctx,
                            &f.console.state,
                        )
                        .await;
                    }
                    Err(AttemptError::Stop(error)) if !success => {
                        assert!(!error.retryable);
                        super::super::super::stage_finalize::run_failure(
                            &error,
                            &f.ctx,
                            &f.console.state,
                        )
                        .await;
                    }
                    _ => panic!("unexpected prepaid SDK outcome"),
                }
                // Unknown usage retains the established estimate; it is not measured SDK token usage.
                assert_eq!(
                    remaining(&f, mode).await,
                    if success { 1000 - reserved } else { 1000 }
                );
                if mode == "message_prepaid" {
                    assert_eq!(reserved, 1);
                }
                audit(&f, if success { "completed" } else { "failed" }, 1).await;
                assert!(AccessBalanceStore::Sqlite(&f.local)
                    .cash_ledger(&f.key)
                    .await
                    .unwrap()
                    .is_empty());
                finish(f).await;
            }
        }
    }
}

#[tokio::test]
async fn copilot_prepaid_exhausted_quota_rejects_without_attempt() {
    for mode in ["message_prepaid", "token_prepaid"] {
        let mut f = prepaid(mode, 0, "ok", false).await;
        let error = super::super::super::stage_filter::run(&mut f.ctx, &f.console.state)
            .await
            .unwrap_err();
        assert_eq!(f.ctx.quota_pre_deducted_tokens, 0);
        assert_eq!(f.ctx.route_attempt_count(), 0);
        super::super::super::stage_finalize::run_failure(&error, &f.ctx, &f.console.state).await;
        assert_eq!(remaining(&f, mode).await, 0);
        audit(&f, "failed", 0).await;
        finish(f).await;
    }
}

#[tokio::test]
async fn copilot_prepaid_cash_rejects_before_http_packing_or_reservation() {
    for with_limit in [false, true] {
        let mut f = prepaid("cash_prepaid", 1_000_000, "ok", false).await;
        if with_limit {
            f.ctx.canonical_req.raw_body["max_tokens"] = json!(16);
        }
        super::super::super::stage_filter::run(&mut f.ctx, &f.console.state)
            .await
            .unwrap();
        let before = AccessBalanceStore::Sqlite(&f.local)
            .cash_balance(&f.key)
            .await
            .unwrap();
        let error = crate::cash_billing::admission::prepare(&mut f.ctx, &f.console.state)
            .await
            .unwrap_err();
        assert_eq!(
            error.code.as_deref(),
            Some(if with_limit {
                "cash_adapter_unsupported"
            } else {
                "cash_output_limit_required"
            })
        );
        assert!(f.ctx.cash_charge.is_none());
        assert_eq!(f.ctx.route_attempt_count(), 0);
        super::super::super::stage_finalize::run_failure(&error, &f.ctx, &f.console.state).await;
        let store = AccessBalanceStore::Sqlite(&f.local);
        assert_eq!(
            serde_json::to_value(store.cash_balance(&f.key).await.unwrap()).unwrap(),
            serde_json::to_value(before).unwrap()
        );
        assert!(store.cash_ledger(&f.key).await.unwrap().is_empty());
        audit(&f, "failed", 0).await;
        finish(f).await;
    }
}
