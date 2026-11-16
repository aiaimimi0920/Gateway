use super::*;
#[tokio::test]
async fn freebuff_pending_binding_uses_exact_local_account_and_ignores_caller_identity_hints() {
    let f = fixture();
    let peer = wire::Peer::new().await;
    let mut request = TestRequest::new(&f, &peer).await;
    let original = request.binding();
    for _ in 0..20 {
        assert!(original == request.binding());
    }
    assert_eq!(
        request.snapshot.resolve_candidates(Some("open"))[0]
            .payload
            .credential_id
            .as_deref(),
        Some("a"),
        "capture must not consume the normal pool round-robin cursor after original b"
    );
    request.ctx.session.as_mut().unwrap().user_id = Some("forged user".into());
    request
        .ctx
        .request_headers
        .insert("x-request-id".into(), "forged-request".into());
    request
        .ctx
        .client_session_hints
        .insert("session".into(), "forged-session".into());
    request
        .ctx
        .canonical_req
        .extra
        .insert("freebuffConfirmed".into(), json!(true));
    assert!(
        original == request.binding(),
        "caller hints are not new authenticated authority"
    );
    let saved = request.candidate.payload.clone();
    request.candidate.payload.credential_id = None;
    assert!(RequestBinding::capture(
        &request.ctx,
        &request.snapshot,
        &request.candidate,
        "open",
        &request.body
    )
    .is_err());
    request.candidate.payload = saved.clone();
    request.candidate.payload.api_key = "rotated-without-revision".into();
    assert!(RequestBinding::capture(
        &request.ctx,
        &request.snapshot,
        &request.candidate,
        "open",
        &request.body
    )
    .is_err());
    request.candidate.payload = saved;
    request.candidate.provider_credential_id = Some("unmatched-server-row".into());
    assert!(RequestBinding::capture(
        &request.ctx,
        &request.snapshot,
        &request.candidate,
        "open",
        &request.body
    )
    .is_err());
    request.candidate.provider_credential_id = None;
    for api in ["dev-mode", "gateway-key"] {
        let session = request.ctx.session.as_mut().unwrap();
        session.api_key_id = Some(api.into());
        session.access_key_id = None;
        assert!(RequestBinding::capture(
            &request.ctx,
            &request.snapshot,
            &request.candidate,
            "open",
            &request.body
        )
        .is_err());
    }
    request.ctx.session = None;
    assert!(RequestBinding::capture(
        &request.ctx,
        &request.snapshot,
        &request.candidate,
        "open",
        &request.body
    )
    .is_err());
    assert_eq!(peer.purchases(), 0);
    peer.close().await;
}
