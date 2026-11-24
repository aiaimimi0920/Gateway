//! Local admission diagnostics must never impersonate actual upstream attempts.
use super::super::admission as gate;
use super::*;

fn context() -> PipelineContext {
    PipelineContext::new(
        make_req(ProtocolFamily::OpenAi, EndpointKind::ChatCompletions),
        None,
    )
}

fn rejection() -> GatewayError {
    GatewayError::service_unavailable("private bearer sk-do-not-record")
        .with_code("upstream_credential_unavailable")
}

#[test]
fn admission_unsent_next_restores_tristate_and_records_bounded_safe_facts() {
    let candidate = make_candidate("openai_compatible");
    for previous in [None, Some(None), Some(Some(201))] {
        let mut ctx = context();
        ctx.note_provider_attempt("previous-sender");
        let before = (ctx.route_attempt_count(), previous);
        ctx.observed_upstream_status = Some(None);
        let result = Err(AttemptError::Next(rejection()));
        for _ in 0..2 {
            gate::record_unsent_next(&mut ctx, &candidate, &result, before);
        }
        assert_eq!(ctx.observed_upstream_status, previous);
        assert_eq!(ctx.route_attempt_count(), 1);
        assert_eq!(ctx.attempted_provider_ids(), ["previous-sender"]);
        assert!(ctx.selected_provider_id.is_none());
        let encoded = serde_json::to_value(&ctx.rejected_candidates).unwrap();
        assert_eq!(encoded.as_array().unwrap().len(), 1);
        assert_eq!(
            encoded[0]["providerAccountId"],
            candidate.provider_account_id
        );
        assert_eq!(encoded[0]["realCredentialRef"], "cred-1");
        assert_eq!(encoded[0]["resolvedModel"], "gpt-4o");
        assert_eq!(encoded[0]["reason"], "upstream_credential_unavailable");
        assert!(!encoded.to_string().contains("sk-do-not-record"));
        assert!(!encoded.to_string().contains(&candidate.payload.api_key));
        for index in 0..110 {
            let mut other = candidate.clone();
            other.provider_account_id = format!("other-{index}");
            gate::record_rejection(&mut ctx, &other, &rejection());
        }
        assert_eq!(ctx.rejected_candidates.len(), 100);
    }
}

#[test]
fn admission_success_stop_and_sent_next_keep_their_own_observation() {
    let candidate = make_candidate("openai_compatible");
    for (result, sent) in [
        (Ok(PipelineOutput::Json(serde_json::json!({}))), false),
        (Err(AttemptError::Stop(rejection())), false),
        (Err(AttemptError::Stop(rejection())), true),
        (Err(AttemptError::Next(rejection())), true),
    ] {
        let mut ctx = context();
        let before = (ctx.route_attempt_count(), Some(Some(201)));
        if sent {
            ctx.note_provider_attempt(&candidate.provider_account_id);
        }
        ctx.observed_upstream_status = Some(None);
        gate::record_unsent_next(&mut ctx, &candidate, &result, before);
        assert_eq!(ctx.observed_upstream_status, Some(None));
        assert!(ctx.rejected_candidates.is_empty());
    }
}

#[test]
fn admission_request_stop_never_borrows_previous_provider_error_identity() {
    let candidate = make_candidate("openai_compatible");
    for reason in [
        "caller_cancelled",
        "shutdown",
        "deadline",
        "attempt_limit",
        "cancelled",
        "response_handed_off",
    ] {
        let mut error = rejection();
        error.fallback_hint = FallbackHint::Abort {
            reason: format!("budget_exhausted:{reason}"),
        };
        assert!(error.request_budget_stop_reason().is_some(), "{reason}");
        let mut ctx = context();
        gate::record_rejection(&mut ctx, &candidate, &error);
        let result = Err(AttemptError::Next(error));
        gate::record_unsent_next(&mut ctx, &candidate, &result, (0, None));
        assert!(ctx.rejected_candidates.is_empty());
    }
}
