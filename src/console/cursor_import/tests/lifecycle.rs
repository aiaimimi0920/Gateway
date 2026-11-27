use super::*;
use std::sync::atomic::Ordering;

#[tokio::test]
async fn cursor_expired_management_permission_retains_verified_material_for_resume_without_login() {
    let fixture = Fixture::new().await;
    let peer = Peer::new(true).await;
    let input = fixture.body(true);
    fixture.create(&peer, input.clone()).await;
    peer.checked().await;
    // Revocation exercises the same rejected-grant branch as expiry, without wall-clock sleeps.
    let context = crate::console::ConsoleRequestContext::loopback();
    fixture
        .state
        .console_auth
        .logout(&context, support::MANAGEMENT_TOKEN)
        .unwrap();
    peer.release.notify_one();
    let id = input["attemptId"].as_str().unwrap();
    let waiting = fixture.wait(id, "awaiting_permission").await;
    assert_eq!(waiting["authReceived"], true);
    assert_eq!(waiting["accountMatched"], true);
    assert_eq!(waiting["credentialPersisted"], false);
    let denied = fixture
        .call(
            Method::POST,
            &format!("{ROOT}/{id}/resume"),
            Value::Null,
            Some(&fixture.grant),
        )
        .await;
    assert_eq!(denied.status(), StatusCode::FORBIDDEN);
    let fresh = support::grant_console_secret_access(&fixture.state).await;
    let resumed = fixture
        .call(
            Method::POST,
            &format!("{ROOT}/{id}/resume"),
            Value::Null,
            Some(&fresh),
        )
        .await;
    assert_eq!(resumed.status(), StatusCode::OK);
    fixture.wait(id, "succeeded").await;
    assert_eq!(peer.checks.load(Ordering::SeqCst), 1);
    assert_eq!(peer.polls.load(Ordering::SeqCst), 0);
    fixture.close().await;
}

#[tokio::test]
async fn cursor_cancel_after_queued_permission_renewal_drops_the_grant_slot() {
    let fixture = Fixture::new().await;
    let peer = Peer::new(true).await;
    let context = crate::console::ConsoleRequestContext::loopback();
    let actor = fixture
        .state
        .console_auth
        .authenticate_management_token(&context, support::MANAGEMENT_TOKEN)
        .unwrap();
    let consent = Consent {
        actor: actor.clone(),
        context: context.clone(),
        grant: fixture.grant.clone(),
    };
    let view = TEST_ORIGIN
        .scope(peer.origin.clone(), async {
            fixture
                .state
                .lifecycle
                .cursor_import
                .create(
                    fixture.state.clone(),
                    "owner".into(),
                    serde_json::from_value(fixture.body(true)).unwrap(),
                    consent,
                )
                .unwrap()
        })
        .await;
    let session = fixture
        .state
        .lifecycle
        .cursor_import
        .get(&view.id, "owner")
        .unwrap();
    peer.checked().await;
    fixture
        .state
        .console_auth
        .logout(&context, support::MANAGEMENT_TOKEN)
        .unwrap();
    peer.release.notify_one();
    tokio::time::timeout(Duration::from_secs(5), async {
        while session.view().status != "awaiting_permission" {
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap();
    let grant = support::grant_console_secret_access(&fixture.state).await;
    // Both calls are synchronous: cancellation wins before the waiting task consumes the new grant.
    fixture
        .state
        .lifecycle
        .cursor_import
        .resume(
            &fixture.state,
            &view.id,
            "owner",
            Consent {
                actor,
                context,
                grant,
            },
        )
        .unwrap();
    assert!(session.renew.borrow().is_some());
    session.cancel();
    fixture.state.lifecycle.cursor_import.join().await;
    assert!(session.renew.borrow().is_none());
    assert_eq!(session.view().status, "cancelled");
    assert_eq!(peer.checks.load(Ordering::SeqCst), 1);
    assert!(
        fixture.state.route_config.snapshot().document().providers[0]
            .credentials
            .is_empty()
    );
    fixture.close().await;
}

#[tokio::test]
async fn cursor_waiting_permission_cancellation_discards_material_without_saving() {
    let fixture = Fixture::new().await;
    let context = crate::console::ConsoleRequestContext::loopback();
    let actor = fixture
        .state
        .console_auth
        .authenticate_management_token(&context, support::MANAGEMENT_TOKEN)
        .unwrap();
    let session = Session::new(
        uuid::Uuid::new_v4().to_string(),
        Binding {
            owner: "owner".into(),
            provider: "cursor".into(),
            revision: fixture.state.route_config.snapshot().revision().id().into(),
            credential: None,
            email: None,
        },
        None,
    );
    let state = fixture.state.clone();
    let target = session.clone();
    let task = tokio::spawn(async move {
        super::super::completion::save_verified(
            &state,
            &target,
            Consent {
                actor,
                context,
                grant: "expired".into(),
            },
            crate::protocol::cursor_auth::Material {
                token: access(),
                subject: "auth0|synthetic".into(),
                email: "owner@example.test".into(),
                exp: 4070908800,
            },
        )
        .await;
    });
    tokio::time::timeout(Duration::from_secs(5), async {
        while session.view().status != "awaiting_permission" {
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap();
    session.cancel();
    tokio::time::timeout(Duration::from_secs(5), task)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(session.view().status, "cancelled");
    assert!(
        fixture.state.route_config.snapshot().document().providers[0]
            .credentials
            .is_empty()
    );
    fixture.close().await;
}
