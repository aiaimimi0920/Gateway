use super::*;

fn values() -> OAuthClient {
    OAuthClient {
        client_id: "synthetic-client".into(),
        client_secret: "synthetic-secret".into(),
    }
}
fn fail_default() -> OAuthClient {
    panic!("explicit configuration must not select built-in defaults")
}

#[tokio::test]
async fn complete_env_pair_wins_without_reading_file_or_defaults() {
    let client = source(
        Some("env-id".into()),
        Some("env-secret".into()),
        Some("relative".into()),
        fail_default,
    )
    .unwrap()
    .load()
    .await
    .unwrap();
    assert_eq!(client.client_id(), "env-id");
    assert_eq!(client.client_secret(), "env-secret");
}

#[test]
fn partial_empty_and_invalid_env_never_fall_back() {
    for (id, secret) in [
        (Some("id"), None),
        (None, Some("secret")),
        (Some(""), Some("secret")),
        (Some("id"), Some("")),
        (Some("bad id"), Some("secret")),
        (Some("id"), Some("bad\nsecret")),
    ] {
        assert!(matches!(
            source(
                id.map(Into::into),
                secret.map(Into::into),
                None,
                fail_default
            ),
            Err(INVALID)
        ));
    }
}

#[tokio::test]
async fn explicit_file_is_absolute_and_no_override_needs_no_private_file() {
    assert!(matches!(
        source(None, None, Some("relative.json".into()), fail_default),
        Err(INVALID)
    ));
    assert!(matches!(
        source(None, None, Some("".into()), fail_default),
        Err(INVALID)
    ));
    let root = scratch();
    let builtin = source(None, None, None, OAuthClient::builtin)
        .unwrap()
        .load()
        .await
        .unwrap();
    assert!(builtin.client_id() == super::super::CLIENT_ID);
    assert!(builtin.client_secret() == super::super::CLIENT_SECRET);
    let Source::File(path) = source(
        None,
        None,
        Some(root.clone().into_os_string()),
        fail_default,
    )
    .unwrap() else {
        panic!()
    };
    assert_eq!(path, root);
}

fn scratch() -> PathBuf {
    let root = if cfg!(windows) {
        PathBuf::from("C:/Users/Public/nas_home/AI/GameEditor/linshi")
    } else {
        std::env::temp_dir()
    };
    root.join(format!(
        "gateway-oauth-client-test-{}",
        uuid::Uuid::new_v4()
    ))
}

#[tokio::test]
async fn bounded_file_loader_is_strict_secret_safe_and_recovers_without_restart() {
    let root = scratch();
    tokio::fs::create_dir_all(&root).await.unwrap();
    let path = root.join("client.json");
    assert_eq!(Source::File(path.clone()).load().await.err(), Some(MISSING));
    let invalid = [
        b"not-json-private-value".to_vec(),
        br#"{"client_id":"id","client_secret":"secret","unknown":true}"#.to_vec(),
        br#"{"client_id":"id","client_secret":""}"#.to_vec(),
        br#"{"client_id":"id","client_secret":"bad\nsecret"}"#.to_vec(),
        br#"{"client_id":"id"}"#.to_vec(),
        br#"{"client_id":"id","client_id":"other","client_secret":"secret"}"#.to_vec(),
        vec![b'x'; MAX_BYTES as usize + 1],
    ];
    for bytes in invalid {
        tokio::fs::write(&path, bytes).await.unwrap();
        assert_eq!(Source::File(path.clone()).load().await.err(), Some(INVALID));
    }
    assert!(Source::File(root).load().await.is_err());
    tokio::fs::write(&path, br#"{"client_id":"id","client_secret":"secret"}"#)
        .await
        .unwrap();
    let first = Source::File(path.clone()).load().await.unwrap();
    tokio::fs::write(
        &path,
        br#"{"client_id":"next-id","client_secret":"next-secret"}"#,
    )
    .await
    .unwrap();
    let next = Source::File(path).load().await.unwrap();
    assert_eq!(first.client_id(), "id"); // Existing sessions keep a stable snapshot.
    assert_eq!(first.client_secret(), "secret");
    assert_eq!(next.client_id(), "next-id");
}

#[test]
fn secrets_are_bounded_and_refresh_scope_is_exact() {
    let client = values();
    assert_eq!(
        client.refresh_secret(super::super::TOKEN_ENDPOINT, "synthetic-client"),
        Ok("synthetic-secret")
    );
    for endpoint in [
        "https://oauth2.googleapis.com/token?x=1",
        "https://evil.invalid/token",
        "http://oauth2.googleapis.com/token",
    ] {
        assert_eq!(
            client.refresh_secret(endpoint, "synthetic-client"),
            Err("antigravity_oauth_client_mismatch")
        );
    }
    assert!(client
        .refresh_secret(super::super::TOKEN_ENDPOINT, "other-client")
        .is_err());
    for (id, secret) in [
        ("x".repeat(1025), "secret".into()),
        ("id".into(), "x".repeat(4097)),
    ] {
        assert_eq!(
            OAuthClient {
                client_id: id,
                client_secret: secret
            }
            .validated()
            .err(),
            Some(INVALID)
        );
    }
}
