use super::*;
fn client() -> config::OAuthClient {
    serde_json::from_value(serde_json::json!({
        "client_id":"synthetic-client", "client_secret":"synthetic-client-secret"
    }))
    .unwrap()
}
fn session() -> Session {
    Session {
        id: "synthetic-login".into(),
        owner: "owner".into(),
        provider: "pool".into(),
        created: Instant::now(),
        nonce: "state".into(),
        verifier: "verifier".into(),
        auth_url: authorization_url(&client(), "state", "verifier"),
        oauth_client: client(),
        cancel: Arc::new(Notify::new()),
        inner: Mutex::new(SessionState {
            status: "waiting_user",
            message: String::new(),
            material: None,
            credential_id: None,
            receipt_at: None,
        }),
    }
}
#[test]
fn antigravity_pkce_and_offline_authorization_are_bound_to_installed_client() {
    let url = url::Url::parse(&authorization_url(
        &client(),
        "state",
        "dBjftJeZ4CVP-mB92K27uhbUJU1p1r_wW1gFWFOEjXk",
    ))
    .unwrap();
    let fields: HashMap<_, _> = url.query_pairs().collect();
    assert_eq!(url.host_str(), Some("accounts.google.com"));
    assert_eq!(
        fields["code_challenge"],
        "E9Melhoa2OwvFrEMTJguCHaoeK1t8URWbuGJSstw-cM"
    );
    assert_eq!(fields["client_id"], "synthetic-client");
    assert_eq!(fields["access_type"], "offline");
    assert_eq!(fields["redirect_uri"], config::REDIRECT_URI);
    assert_ne!(random_secret(), random_secret());
}
#[test]
fn antigravity_state_is_single_use_and_cancel_discards_ephemeral_material() {
    let session = session();
    assert!(session.begin_exchange("wrong").is_err());
    session.begin_exchange("state").unwrap();
    assert!(session.begin_exchange("state").is_err());
    session.cancel();
    assert_eq!(session.view().status, "cancelled");
    assert!(session.begin_exchange("state").is_err());
    let view = serde_json::to_string(&session.view()).unwrap();
    assert!(!view.contains("verifier"));
    assert!(!view.contains("refresh_token"));
    assert!(!view.contains("synthetic-client-secret"));
}
#[test]
fn antigravity_sessions_bind_owner_and_expiry() {
    let session = Arc::new(session());
    sessions()
        .lock()
        .insert(session.id.clone(), session.clone());
    assert!(get(&session.id, "other").is_err());
    assert!(get(&session.id, "owner").is_ok());
    sessions().lock().remove(&session.id);
    let mut expired = super::tests::session();
    expired.created = Instant::now() - TTL;
    assert!(expired.begin_exchange("state").is_err());
}

#[test]
fn antigravity_commit_crossing_auth_ttl_keeps_a_bounded_readable_receipt() {
    let mut session = session();
    session.created = Instant::now() - TTL;
    session.inner.lock().status = "importing";
    assert!(session.retention_delay().is_some());
    {
        let mut inner = session.inner.lock();
        inner.status = "succeeded";
        inner.receipt_at = Some(Instant::now());
        inner.credential_id = Some("saved-id".into());
    }
    assert!(session.retention_delay().is_some());
    assert_eq!(session.view().credential_id.as_deref(), Some("saved-id"));
    session.inner.lock().receipt_at = Some(Instant::now() - RECEIPT_TTL - Duration::from_secs(1));
    assert!(session.retention_delay().is_none());
}
