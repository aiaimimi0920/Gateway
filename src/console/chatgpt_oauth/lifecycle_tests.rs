use super::*;

fn make_session() -> Arc<Session> {
    Arc::new(Session {
        id: uuid::Uuid::new_v4().to_string(),
        owner: "owner".into(),
        provider: "chatgpt".into(),
        group: "free".into(),
        created: Instant::now(),
        nonce: "nonce".into(),
        verifier: "private-verifier".into(),
        auth_url: authorization_url("nonce", "private-verifier"),
        cancel: Arc::new(Notify::new()),
        inner: Mutex::new(SessionState {
            status: "waiting_user",
            message: String::new(),
            material: None,
            credential_id: None,
        }),
    })
}

#[test]
fn state_is_single_use_and_mismatch_does_not_consume_session() {
    let session = make_session();
    assert!(session.begin_exchange("wrong").is_err());
    assert_eq!(session.view().status, "waiting_user");
    assert!(session.begin_exchange("nonce").is_ok());
    assert!(session.begin_exchange("nonce").is_err());
    session.cancel();
    assert!(session.begin_exchange("nonce").is_err());
}

#[test]
fn ownership_expiry_and_public_view_do_not_expose_material() {
    let session = make_session();
    sessions()
        .lock()
        .insert(session.id.clone(), session.clone());
    let unavailable = get(&session.id, "other-operator").err().unwrap();
    assert_eq!(
        unavailable.code.as_deref(),
        Some("chatgpt_auth_session_unavailable")
    );
    assert!(get(&session.id, "owner").is_ok());
    let json = serde_json::to_string(&session.view()).unwrap();
    assert!(!json.contains("private-verifier"));
    sessions().lock().remove(&session.id);
    let mut expired = make_session();
    Arc::get_mut(&mut expired).unwrap().created = Instant::now() - TTL;
    assert!(expired.begin_exchange("nonce").is_err());
    sessions()
        .lock()
        .insert(expired.id.clone(), expired.clone());
    let unavailable = get(&expired.id, "owner").err().unwrap();
    sessions().lock().remove(&expired.id);
    assert_eq!(
        unavailable.code.as_deref(),
        Some("chatgpt_auth_session_unavailable")
    );
}

#[test]
fn callback_rejects_wrong_targets_and_ambiguous_parameters() {
    let prefix = "http://localhost:1455/auth/callback";
    assert_eq!(
        parse_callback(&format!("{prefix}?code=a%2Bb&state=s")).unwrap(),
        ("a+b".into(), "s".into())
    );
    for value in [
        format!("{prefix}?code=a&code=b&state=s"),
        format!("{prefix}?code=a"),
        "https://localhost:1455/auth/callback?code=a&state=s".into(),
        "http://example.com:1455/auth/callback?code=a&state=s".into(),
        "http://u@localhost:1455/auth/callback?code=a&state=s".into(),
        "x".repeat(16_385),
    ] {
        assert!(parse_callback(&value).is_err());
    }
}

#[tokio::test]
async fn cancelling_listener_releases_bound_port() {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let session = make_session();
    callback::serve(listener, session.clone(), rquest::Client::new());
    session.cancel();
    tokio::time::timeout(Duration::from_secs(2), async {
        loop {
            if tokio::net::TcpListener::bind(address).await.is_ok() {
                break;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .unwrap();
}
