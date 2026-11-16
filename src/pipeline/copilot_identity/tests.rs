use super::*;
use serde_json::json;
#[path = "fixture.rs"]
mod fixture;
use fixture::{input, Fixture};
#[path = "negative_tests.rs"]
mod negative;

#[tokio::test]
async fn copilot_identity_binds_authenticated_key_and_exact_saved_identity_without_enabling_routes()
{
    let f = Fixture::new().await;
    let identity = f.bind();
    assert_eq!(identity.local_key, f.key);
    assert_eq!(identity.credential, "account");
    assert_eq!(identity.request().model, "sdk-model");
    assert_eq!(identity.request().prompt, "hello");
    assert_eq!(
        identity.token_for_send().await.unwrap(),
        "synthetic-identity-secret"
    );
    let snapshot = f.console.state.route_config.snapshot();
    assert_eq!(
        snapshot.resolve_candidates(Some("public-alias")).len(),
        usize::from(cfg!(feature = "copilot-sdk-runtime"))
    );
    assert_eq!(
        snapshot.get_providers()[0].credential_pool[0].enabled,
        cfg!(feature = "copilot-sdk-runtime")
    );
    assert!(!format!("{:?}", f.candidate.payload).contains("synthetic-identity-secret"));
    drop(identity);
    f.close().await;
}

#[tokio::test]
async fn copilot_identity_rejects_same_id_rotation_removal_and_disabled_account() {
    for mode in 0..4 {
        let mut f = Fixture::new().await;
        let identity = f.bind();
        assert!(identity.token_for_send().await.is_ok());
        f.replace(|doc| match mode {
            0 => {
                doc.providers[0].credentials[0].copilot_github_token = Some("rotated-secret".into())
            }
            1 => doc.providers[0].credentials[0].enabled = Some(false),
            2 => {
                doc.providers[0].credentials.clear();
                doc.account_groups[0].provider_credential_ids.clear();
            }
            _ => doc.providers[0].credentials[0].copilot_github_token = None,
        });
        let error = identity.token_for_send().await.unwrap_err();
        assert!(!error.retryable);
        assert!(matches!(error.fallback_hint, FallbackHint::Abort { .. }));
        assert!(!format!("{error:?}").contains("secret"));
        drop(identity);
        f.ctx.cash_route_snapshot = Some(f.console.state.route_config.snapshot());
        let rebound = CopilotIdentity::bind(&f.console.state, &f.ctx, &f.candidate, "sdk-model");
        if mode == 0 {
            assert_eq!(
                rebound.unwrap().token_for_send().await.unwrap(),
                "rotated-secret"
            );
        } else {
            assert!(rebound.is_err());
        }
        f.close().await;
    }
}

#[tokio::test]
async fn copilot_identity_rechecks_live_key_scope_model_provider_group_and_expiry() {
    let f = Fixture::new().await;
    let identity = f.bind();
    let initial =
        json!({"scopes":["chat.completions"],"models":["public-alias"],"providerIds":["copilot"]});
    for metadata in [
        json!({"scopes":["responses"]}),
        json!({"models":["sdk-model"]}),
        json!({"providerIds":["other"]}),
        json!({"accountGroupIds":[]}),
    ] {
        assert!(identity.token_for_send().await.is_ok());
        f.local
            .save_access_key(Some(&f.key), input(metadata))
            .await
            .unwrap();
        assert!(identity.token_for_send().await.is_err());
        f.local
            .save_access_key(Some(&f.key), input(initial.clone()))
            .await
            .unwrap();
    }
    f.local
        .save_access_key(
            Some(&f.key),
            input(json!({"accountGroupIds":["sdk-group"]})),
        )
        .await
        .unwrap();
    assert!(identity.token_for_send().await.is_ok());
    let mut expired = input(initial);
    expired.expires_at = Some("2000-01-01T00:00:00Z".into());
    f.local
        .save_access_key(Some(&f.key), expired)
        .await
        .unwrap();
    assert!(identity.token_for_send().await.is_err());
    drop(identity);
    f.close().await;
}

#[tokio::test]
async fn copilot_identity_rechecks_key_disable_and_revocation_after_binding() {
    let f = Fixture::new().await;
    let identity = f.bind();
    f.local.set_access_key_enabled(&f.key, false).await.unwrap();
    assert!(identity.token_for_send().await.is_err());
    f.local.set_access_key_enabled(&f.key, true).await.unwrap();
    assert!(identity.token_for_send().await.is_ok());
    f.local
        .revoke_access_key(&f.key, Some("synthetic test"))
        .await
        .unwrap();
    assert!(identity.token_for_send().await.is_err());
    drop(identity);
    f.close().await;
}

#[tokio::test]
async fn copilot_identity_never_combines_grants_from_different_key_versions() {
    let f = Fixture::new().await;
    let identity = f.bind();
    let a = json!({"scopes":["chat.completions"],"models":["other"]});
    let b = json!({"scopes":["responses"],"models":["public-alias"]});
    f.local
        .save_access_key(Some(&f.key), input(a.clone()))
        .await
        .unwrap();
    let writer = async {
        for index in 0..32 {
            f.local
                .save_access_key(
                    Some(&f.key),
                    input(if index % 2 == 0 { b.clone() } else { a.clone() }),
                )
                .await
                .unwrap();
            tokio::task::yield_now().await;
        }
    };
    let reader = async {
        for _ in 0..64 {
            assert!(
                identity.token_for_send().await.is_err(),
                "Neither key version grants this complete request"
            );
            tokio::task::yield_now().await;
        }
    };
    tokio::join!(writer, reader);
    drop(identity);
    f.close().await;
}
