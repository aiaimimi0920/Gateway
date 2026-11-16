use super::*;

#[tokio::test]
async fn copilot_identity_rejects_untrusted_or_incomplete_contexts() {
    let mut f = Fixture::new().await;
    let original = f.ctx.session.clone();
    f.ctx.session = None;
    assert!(CopilotIdentity::bind(&f.console.state, &f.ctx, &f.candidate, "sdk-model").is_err());
    f.ctx.session = original;
    f.ctx.credential_source = CredentialSource::Hosted;
    assert!(CopilotIdentity::bind(&f.console.state, &f.ctx, &f.candidate, "sdk-model").is_err());
    f.ctx.credential_source = CredentialSource::Platform;
    let snapshot = f.ctx.cash_route_snapshot.take();
    assert!(CopilotIdentity::bind(&f.console.state, &f.ctx, &f.candidate, "sdk-model").is_err());
    f.ctx.cash_route_snapshot = snapshot;
    let id = f.candidate.payload.credential_id.take();
    assert!(CopilotIdentity::bind(&f.console.state, &f.ctx, &f.candidate, "sdk-model").is_err());
    f.candidate.payload.credential_id = id;
    f.candidate.provider_account_id = "foreign".into();
    assert!(CopilotIdentity::bind(&f.console.state, &f.ctx, &f.candidate, "sdk-model").is_err());
    f.candidate.provider_account_id = "copilot".into();
    f.candidate.provider_credential_id = Some("server-db-account".into());
    assert!(CopilotIdentity::bind(&f.console.state, &f.ctx, &f.candidate, "sdk-model").is_err());
    f.candidate.provider_credential_id = None;
    f.ctx.account_group_id = Some("missing".into());
    assert!(f.bind().token_for_send().await.is_err());
    f.ctx.account_group_id = None;
    f.ctx.canonical_req.metadata = Some(Default::default());
    assert!(CopilotIdentity::bind(&f.console.state, &f.ctx, &f.candidate, "sdk-model").is_err());
    f.close().await;
}

#[tokio::test]
async fn copilot_identity_observes_cancel_shutdown_and_shared_deadline() {
    for mode in 0..3 {
        let mut f = Fixture::new().await;
        let (tx, rx) = tokio::sync::watch::channel(false);
        f.ctx.caller_cancel = Some(rx);
        let identity = f.bind();
        match mode {
            0 => {
                tx.send(true).unwrap();
            }
            1 => {
                f.console.state.shutdown.request("synthetic cancel");
            }
            _ => {
                f.ctx.request_budget.cancel_caller();
            }
        }
        assert!(identity.token_for_send().await.is_err());
        drop(identity);
        f.close().await;
    }
    let mut f = Fixture::new().await;
    f.ctx.request_budget =
        crate::pipeline::request_budget::RequestBudget::new(1, std::time::Duration::ZERO);
    assert!(CopilotIdentity::bind(&f.console.state, &f.ctx, &f.candidate, "sdk-model").is_err());
    f.close().await;
}

#[tokio::test]
async fn copilot_identity_rejects_expired_saved_token_and_stale_frozen_snapshot() {
    let mut f = Fixture::new().await;
    for expiry in ["2000-01-01T00:00:00Z", "invalid-expiry"] {
        f.replace(|doc| doc.providers[0].credentials[0].expires_at = Some(expiry.into()));
        assert!(
            CopilotIdentity::bind(&f.console.state, &f.ctx, &f.candidate, "sdk-model").is_err()
        );
        f.ctx.cash_route_snapshot = Some(f.console.state.route_config.snapshot());
        assert!(
            CopilotIdentity::bind(&f.console.state, &f.ctx, &f.candidate, "sdk-model").is_err()
        );
    }
    f.close().await;
}
