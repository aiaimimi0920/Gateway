//! Crash recovery resumes only durable evidence; failed sends and rejected identities never replay HTTP.
use super::{lobsterai_fixture::*, *};

#[tokio::test]
async fn lobsterai_restart_recovers_receipt_and_result_without_network_or_new_401() {
    for save_result in [false, true] {
        let mut f = fixture().await;
        let original = request(&f);
        let sending = reserve(&f, &original).await;
        let tokens = ReceivedTokens::from_json(reply().to_string().as_bytes()).unwrap();
        let record = f
            .runtime
            .store_refresh_grant_receipt(
                &sending,
                &GrantReceipt::LobsterAi {
                    tokens: tokens.clone(),
                },
            )
            .await
            .unwrap();
        if save_result {
            let next = tokens
                .interpret_successor(
                    original.material.lobsterai.as_ref().unwrap(),
                    &original.material.access,
                    &original.material.refresh,
                )
                .unwrap();
            f.runtime
                .store_refresh_grant_result(&record, &original.result_with_lobster(&next).unwrap())
                .await
                .unwrap();
        }
        restart(&mut f).await;
        let peer = Peer::start(vec![]).await;
        refresh(&f, &peer, &mut Pending::new()).await;
        assert_eq!(f.saved().api_key.as_deref(), Some("lobster-new-access"));
        assert!(f.saved().expires_at.is_none());
        assert_eq!(load(&f, &original).phase(), GrantPhase::Published);
        peer.inspect(|s| assert!(s.is_empty()));
    }
}

#[tokio::test]
async fn lobsterai_restart_preserves_sending_fence_without_receipt() {
    let mut f = fixture().await;
    let original = request(&f);
    reserve(&f, &original).await;
    restart(&mut f).await;
    let peer = Peer::start(vec![]).await;
    refresh(&f, &peer, &mut Pending::new()).await;
    assert_eq!(load(&f, &original).phase(), GrantPhase::Sending);
    assert_eq!(f.saved().api_key.as_deref(), Some("synthetic-access"));
    assert_eq!(
        owner(&f).dispatch_readiness().unwrap_err().code.as_deref(),
        Some("oauth_reauthorization_required")
    );
    peer.inspect(|s| assert!(s.is_empty()));
}

#[tokio::test]
async fn lobsterai_invalid_successor_is_retained_before_quarantine_and_never_resent() {
    for data in [
        json!({"accessToken":"next","refreshToken":"rotated","user":{"yid":"different"}}),
        json!({"accessToken":"next","refreshToken":"rotated","accountMode":"enterprise"}),
        json!({"accessToken":17,"refreshToken":"rotated"}),
    ] {
        let mut f = fixture().await;
        let original = request(&f);
        let peer = Peer::start(vec![Reply::json(json!({"code":0,"data":data}))]).await;
        ready(&f, &peer).await;
        reject(&f);
        refresh(&f, &peer, &mut Pending::new()).await;
        let record = load(&f, &original);
        assert!(record.receipt().is_some());
        assert!(record.result().is_none());
        assert_eq!(
            f.saved().refresh_token.as_deref(),
            Some("synthetic-refresh")
        );
        assert_eq!(
            owner(&f).dispatch_readiness().unwrap_err().code.as_deref(),
            Some("oauth_reauthorization_required")
        );
        restart(&mut f).await;
        refresh(&f, &peer, &mut Pending::new()).await;
        peer.inspect(|s| assert_eq!(s.len(), 1));
    }
}

#[tokio::test]
async fn lobsterai_receipt_waits_for_writer_ack_and_retries_only_storage() {
    let f = fixture().await;
    let original = request(&f);
    let sending = reserve(&f, &original).await;
    let state = owner(&f);
    state.confirm_lobster_recovery(0, true);
    let attempt = state
        .begin_refresh(Duration::ZERO, "synthetic-access")
        .unwrap();
    let receipt = ReceivedTokens::from_json(reply().to_string().as_bytes()).unwrap();
    let mut pending = super::super::received::PendingWrite::received_lobster(
        state.clone(),
        attempt,
        f.saved(),
        original.clone(),
        sending,
        receipt,
    );
    let guard = f.writer_lock();
    assert!(!pending.write_to(Some(&f.runtime), None).await);
    assert!(load(&f, &original).receipt().is_none());
    assert_eq!(f.saved().api_key.as_deref(), Some("synthetic-access"));
    assert!(state
        .begin_refresh(Duration::ZERO, "synthetic-access")
        .is_none());
    drop(guard);
    assert!(pending.write_to(Some(&f.runtime), None).await);
    assert_eq!(load(&f, &original).phase(), GrantPhase::Published);
}

#[tokio::test]
async fn lobsterai_late_receipt_is_saved_but_does_not_overwrite_reauthorized_account() {
    let f = fixture().await;
    let original = request(&f);
    let sending = reserve(&f, &original).await;
    let state = owner(&f);
    state.confirm_lobster_recovery(0, true);
    let attempt = state
        .begin_refresh(Duration::ZERO, "synthetic-access")
        .unwrap();
    let receipt = ReceivedTokens::from_json(reply().to_string().as_bytes()).unwrap();
    let mut pending = super::super::received::PendingWrite::received_lobster(
        state,
        attempt,
        f.saved(),
        original.clone(),
        sending,
        receipt,
    );
    let snapshot = f.runtime.route_config().snapshot();
    let mut doc = snapshot.document().clone();
    doc.providers[0].credentials.clear();
    f.runtime
        .commit_document(snapshot.revision().id(), doc, None)
        .await
        .unwrap();
    assert!(pending.write_to(Some(&f.runtime), None).await);
    let record = load(&f, &original);
    assert_eq!(record.phase(), GrantPhase::Result);
    assert!(record.receipt().is_some());
    assert!(f.runtime.route_config().snapshot().document().providers[0]
        .credentials
        .is_empty());
}

#[tokio::test]
async fn lobsterai_durable_receipt_does_not_cross_a_changed_authority_revision() {
    let mut f = fixture().await;
    let original = request(&f);
    let sending = reserve(&f, &original).await;
    let receipt = ReceivedTokens::from_json(reply().to_string().as_bytes()).unwrap();
    f.runtime
        .store_refresh_grant_receipt(&sending, &GrantReceipt::LobsterAi { tokens: receipt })
        .await
        .unwrap();
    let snapshot = f.runtime.route_config().snapshot();
    let mut doc = snapshot.document().clone();
    doc.providers[0].label = Some("new policy revision".into());
    f.runtime
        .commit_document(snapshot.revision().id(), doc, None)
        .await
        .unwrap();
    restart(&mut f).await;
    let peer = Peer::start(vec![]).await;
    refresh(&f, &peer, &mut Pending::new()).await;
    assert_eq!(f.saved().api_key.as_deref(), Some("synthetic-access"));
    assert_eq!(
        owner(&f).dispatch_readiness().unwrap_err().code.as_deref(),
        Some("oauth_reauthorization_required")
    );
    assert!(load(&f, &original).receipt().is_some());
    peer.inspect(|seen| assert!(seen.is_empty()));
}
