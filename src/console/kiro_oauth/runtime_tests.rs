//! The existing registration/device/save owner binds the selected runtime profile, not the OIDC Region.
use super::{test_support::*, *};
use crate::protocol::kiro_oauth::{Directory, Scope, TEST_ORIGIN};
use serde_json::json;
#[path = "../../../tests/console_contract_support/mod.rs"]
mod support;

#[tokio::test]
async fn kiro_runtime_device_save_uses_normal_secret_patches_and_persists_explicit_profile() {
    save_case(false).await;
}
#[tokio::test]
async fn kiro_runtime_reauthorization_preserves_the_effective_credential_profile_override() {
    save_case(true).await;
}
async fn save_case(existing: bool) {
    let mut document = document();
    let provider = &mut document.providers[0];
    provider.base_url = "https://runtime.eu-central-1.kiro.dev".into();
    provider.auth_mode = Some("kiro_oauth_runtime".into());
    provider.responses_path = Some("/".into());
    let default_profile = "arn:aws:codewhisperer:eu-central-1:123456789012:profile/synthetic";
    provider
        .extra_body
        .insert("profileArn".into(), json!(default_profile));
    let profile = if existing {
        "arn:aws:codewhisperer:eu-central-1:123456789012:profile/credential"
    } else {
        default_profile
    };
    if existing {
        provider.credentials.push(serde_json::from_value(json!({"id":"existing","api_key":"synthetic-old-access",
            "extra_body":{"profileArn":profile},"kiro_oauth":{"auth_method":"builder_id","directory_region":"us-east-1",
            "start_url":"https://view.awsapps.com/start","client_id":"synthetic-old-client","client_secret":"synthetic-old-secret",
            "client_secret_expires_at":1893456000i64,"scopes":["codewhisperer:conversations"],"machine_id":"a".repeat(64),
            "runtime_target":{"base_url":"https://runtime.eu-central-1.kiro.dev","profile_arn":profile}}})).unwrap());
    }
    let fixture = support::ConsoleStateFixture::new(document, true);
    let state = &fixture.state;
    let binding = Binding::new(
        "synthetic-owner",
        "kiro-pool",
        state.route_config.snapshot().revision().id(),
    )
    .unwrap();
    let binding = if existing {
        binding.with_credential("existing").unwrap()
    } else {
        binding
    };
    let mock = Mock::new(vec![registration(), device(), tokens(true, true)]).await;
    let session = TEST_ORIGIN
        .scope(mock.origin.clone(), async {
            state
                .lifecycle
                .kiro_oauth
                .start(
                    binding,
                    http(),
                    Directory::builder_id(),
                    vec![Scope::Conversations],
                    "a".repeat(64),
                )
                .unwrap()
        })
        .await;
    wait(&session, SessionStatus::WaitingUser).await;
    TEST_ORIGIN
        .scope(mock.origin.clone(), async {
            state
                .lifecycle
                .kiro_oauth
                .start_poll(session.id(), session.binding(), http())
                .unwrap();
        })
        .await;
    wait(&session, SessionStatus::Ready).await;
    let operation = session
        .begin_save(state, session.binding())
        .unwrap()
        .unwrap();
    Box::pin(operation.run(state.clone(), session.clone())).await;
    assert_eq!(session.view().status, SessionStatus::Succeeded);
    let snapshot = state.route_config.snapshot();
    let credential = &snapshot.document().providers[0].credentials[0];
    let context = credential.kiro_oauth.as_ref().unwrap();
    assert_eq!(context.runtime_target().unwrap().profile_arn(), profile);
    assert_eq!(
        context.token_endpoint(),
        "https://oidc.us-east-1.amazonaws.com/token"
    );
    assert_eq!(
        credential.api_key.as_deref(),
        Some("synthetic-console-access")
    );
    assert_eq!(
        credential.refresh_token.as_deref(),
        Some("synthetic-console-refresh")
    );
    assert_eq!(context.client_secret(), Some("synthetic-console-secret"));
    assert_eq!(mock.count(), 3);
    state.lifecycle.kiro_oauth.join().await;
    mock.stop().await;
}
