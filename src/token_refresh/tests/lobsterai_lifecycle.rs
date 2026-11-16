//! Unknown expiry is usable only after recovery inspection and refreshes only for an exact-generation 401.
use super::{lobsterai_fixture::*, *};

#[tokio::test]
async fn lobsterai_401_rotates_through_receipt_and_publication_without_fabricated_expiry() {
    let mut f = fixture().await;
    let original = request(&f);
    let peer = Peer::start(vec![Reply::json(reply())]).await;
    assert!(owner(&f).dispatch_readiness().is_err());
    ready(&f, &peer).await;
    let old = owner(&f);
    let generation = old.lobster_dispatch_generation().unwrap();
    for status in [403, 429, 500] {
        assert!(!old.reject_lobster_access(generation, "synthetic-access", status));
    }
    assert!(!old.reject_lobster_access(generation, "wrong-token", 401));
    assert!(old
        .begin_refresh(Duration::from_secs(u32::MAX as u64), "synthetic-access")
        .is_none());
    reject(&f);
    assert!(old.dispatch_readiness().is_err());
    let mut pending = Pending::new();
    refresh(&f, &peer, &mut pending).await;
    assert!(pending.is_empty());
    let saved = f.saved();
    assert_eq!(saved.api_key.as_deref(), Some("lobster-new-access"));
    assert_eq!(saved.refresh_token.as_deref(), Some("synthetic-refresh"));
    assert!(saved.expires_at.is_none() && saved.token_expires_in_secs.is_none());
    assert_eq!(load(&f, &original).phase(), GrantPhase::Published);
    assert!(load(&f, &original).receipt().is_some());
    assert!(!old.reject_lobster_access(generation, "synthetic-access", 401));
    owner(&f).dispatch_readiness().unwrap();
    restart(&mut f).await;
    ready(&f, &peer).await;
    peer.inspect(|seen| {
        assert_eq!(seen.len(), 1);
        let s = &seen[0];
        assert_eq!(s.path, "/api/auth/refresh");
        assert_eq!(s.body["refreshToken"], "synthetic-refresh");
        assert_eq!(s.body["firstKeyfrom"], "neuro-gateway");
        assert!(!s.headers.contains_key("authorization"));
        assert!(!s.headers.contains_key("cookie"));
    });
}

#[tokio::test]
async fn lobsterai_unchanged_tokens_advance_generation_and_keep_permanent_history() {
    let f = fixture().await;
    let same = json!({"code":0,"data":{"accessToken":"synthetic-access"}});
    let peer = Peer::start(vec![Reply::json(same.clone()), Reply::json(same)]).await;
    ready(&f, &peer).await;
    let a = request(&f);
    reject(&f);
    refresh(&f, &peer, &mut Pending::new()).await;
    let b = request(&f);
    assert_eq!(a.family_id(), b.family_id());
    assert_ne!(a.material_id(), b.material_id());
    reject(&f);
    refresh(&f, &peer, &mut Pending::new()).await;
    assert_eq!(load(&f, &a).phase(), GrantPhase::Published);
    assert_eq!(load(&f, &b).phase(), GrantPhase::Published);
    assert_ne!(b.material_id(), request(&f).material_id());
    peer.inspect(|s| assert_eq!(s.len(), 2));
}

#[tokio::test]
async fn lobsterai_single_flight_and_account_replacement_reject_late_results() {
    let f = fixture().await;
    let peer = Peer::start(vec![]).await;
    ready(&f, &peer).await;
    reject(&f);
    let old = owner(&f);
    let original = request(&f);
    let attempt = old
        .begin_refresh(Duration::ZERO, "synthetic-access")
        .unwrap();
    assert!(old
        .begin_refresh(Duration::ZERO, "synthetic-access")
        .is_none());
    let next = ReceivedTokens::from_json(reply().to_string().as_bytes())
        .unwrap()
        .interpret_successor(
            original.material.lobsterai.as_ref().unwrap(),
            &original.material.access,
            &original.material.refresh,
        )
        .unwrap();
    let snapshot = f.runtime.route_config().snapshot();
    let mut doc = snapshot.document().clone();
    let other = ReceivedTokens::from_json(
        json!({"code":0,"data":{"accessToken":"other-access","refreshToken":"other-refresh",
        "accountMode":"personal","user":{"yid":"other-person"}}})
        .to_string()
        .as_bytes(),
    )
    .unwrap()
    .admit_initial(uuid::Uuid::new_v4())
    .unwrap();
    let c = &mut doc.providers[0].credentials[0];
    c.api_key = Some(other.access_token);
    c.refresh_token = Some(other.refresh_token);
    c.lobsterai = Some(other.context);
    f.runtime
        .commit_document(snapshot.revision().id(), doc, None)
        .await
        .unwrap();
    assert!(attempt.apply_lobster(&next).is_none());
    assert_eq!(f.saved().api_key.as_deref(), Some("other-access"));
    peer.inspect(|s| assert!(s.is_empty()));
}

#[tokio::test]
async fn lobsterai_two_local_runtimes_reserve_only_one_sender() {
    let a = fixture().await;
    let b = LocalFixture {
        config: a.config.clone(),
        root: a.root.clone(),
        runtime: RouteConfigRuntime::new_local(&a.config).unwrap(),
    };
    b.runtime.recover_startup().await.unwrap();
    let peer = Peer::start(vec![Reply::json(reply())]).await;
    ready(&a, &peer).await;
    ready(&b, &peer).await;
    reject(&a);
    reject(&b);
    let (mut pa, mut pb) = (Pending::new(), Pending::new());
    tokio::join!(refresh(&a, &peer, &mut pa), refresh(&b, &peer, &mut pb));
    assert!(pa.is_empty() && pb.is_empty());
    peer.inspect(|s| assert_eq!(s.len(), 1));
    let reopened = RouteConfigRuntime::new_local(&a.config).unwrap();
    reopened.recover_startup().await.unwrap();
    assert_eq!(
        reopened.route_config().snapshot().document().providers[0].credentials[0]
            .api_key
            .as_deref(),
        Some("lobster-new-access")
    );
}
