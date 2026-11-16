//! Cryptographic contracts use signed mutations, not unsigned payload decoding as proof.
use super::super::oidc::verify_token;
use super::{identity_support::*, *};

#[test]
fn es256_identity_checks_signature_and_keeps_identity_out_of_debug() {
    let signer = Signer::new();
    let client = client();
    let auth = authorization(&client);
    let claims = claims(&client, &auth, NOW);
    let identity = verify_token(
        &client,
        &signer.sign(&claims),
        auth.nonce(),
        "synthetic-access",
        &signer.jwks(),
        NOW,
    )
    .unwrap();
    assert_eq!(identity.subject(), "private-synthetic-subject");
    assert_eq!(identity.issuer(), ISSUER);
    assert_eq!(identity.client_id(), client.client_id());
    assert_eq!(identity.expires_at(), NOW + 300);
    assert!(!format!("{identity:?}").contains("private-synthetic-subject"));
    assert!(!format!("{identity:?}").contains(auth.nonce()));
    let other = Signer::new();
    assert!(verify_token(
        &client,
        &other.sign(&claims),
        auth.nonce(),
        "synthetic-access",
        &signer.jwks(),
        NOW
    )
    .is_err());
}

#[test]
fn signed_claims_reject_wrong_issuer_audience_nonce_time_and_types() {
    let signer = Signer::new();
    let client = client();
    let auth = authorization(&client);
    let base = claims(&client, &auth, NOW);
    let mutations = [
        ("iss", json!("https://auth.x.ai/")),
        ("iss", json!("https://attacker.test")),
        ("sub", json!("")),
        ("sub", json!(123)),
        ("sub", json!("x".repeat(256))),
        ("aud", json!("other-client")),
        ("aud", json!([])),
        ("aud", json!(["gateway-test-client", "other-client"])),
        ("aud", json!(["gateway-test-client", 1])),
        ("azp", json!("other-client")),
        ("nonce", json!("wrong-nonce")),
        ("nonce", Value::Null),
        ("exp", json!(NOW - 61)),
        ("exp", json!(u64::MAX)),
        ("exp", json!(-1)),
        ("exp", json!(NOW as f64 + 0.5)),
        ("iat", json!(NOW + 61)),
        ("iat", json!(-1)),
        ("nbf", json!(NOW + 61)),
        ("nbf", Value::Null),
    ];
    for (field, value) in mutations {
        let mut bad = base.clone();
        bad[field] = value;
        let error = verify_token(
            &client,
            &signer.sign(&bad),
            auth.nonce(),
            "synthetic-access",
            &signer.jwks(),
            NOW,
        )
        .unwrap_err();
        assert_eq!(error, OAuthError::InvalidIdentity, "field={field}");
        assert!(!format!("{error:?} {error}").contains("private-synthetic-subject"));
    }
    for field in ["iss", "sub", "aud", "exp", "iat", "nonce"] {
        let mut bad = base.clone();
        bad.as_object_mut().unwrap().remove(field);
        assert!(
            verify_token(
                &client,
                &signer.sign(&bad),
                auth.nonce(),
                "synthetic-access",
                &signer.jwks(),
                NOW
            )
            .is_err(),
            "missing={field}"
        );
    }
    let mut valid = base;
    valid["aud"] = json!(["gateway-test-client"]);
    valid["azp"] = json!("gateway-test-client");
    valid["nbf"] = json!(NOW);
    assert!(verify_token(
        &client,
        &signer.sign(&valid),
        auth.nonce(),
        "synthetic-access",
        &signer.jwks(),
        NOW
    )
    .is_ok());
}

#[test]
fn optional_access_token_hash_is_verified_when_present() {
    let signer = Signer::new();
    let client = client();
    let auth = authorization(&client);
    let mut body = claims(&client, &auth, NOW);
    body["at_hash"] = json!(URL_SAFE_NO_PAD.encode(&Sha256::digest(b"synthetic-access")[..16]));
    let token = signer.sign(&body);
    assert!(verify_token(
        &client,
        &token,
        auth.nonce(),
        "synthetic-access",
        &signer.jwks(),
        NOW
    )
    .is_ok());
    assert!(verify_token(
        &client,
        &token,
        auth.nonce(),
        "different-access",
        &signer.jwks(),
        NOW
    )
    .is_err());
    body["at_hash"] = Value::Null;
    assert!(verify_token(
        &client,
        &signer.sign(&body),
        auth.nonce(),
        "synthetic-access",
        &signer.jwks(),
        NOW
    )
    .is_err());
}

#[test]
fn jose_rejects_algorithm_confusion_duplicate_fields_and_external_key_headers() {
    let signer = Signer::new();
    let client = client();
    let auth = authorization(&client);
    let body = claims(&client, &auth, NOW).to_string();
    for header in [
        r#"{"alg":"none","kid":"synthetic-key"}"#,
        r#"{"alg":"HS256","kid":"synthetic-key"}"#,
        r#"{"alg":"ES384","kid":"synthetic-key"}"#,
        r#"{"alg":"ES256"}"#,
        r#"{"alg":"ES256","kid":"synthetic-key","kid":"other"}"#,
        r#"{"alg":"ES256","kid":"synthetic-key","crit":["new"]}"#,
        r#"{"alg":"ES256","kid":"synthetic-key","b64":false}"#,
        r#"{"alg":"ES256","kid":"synthetic-key","jku":"https://attacker.test/jwks"}"#,
        r#"{"alg":"ES256","kid":"synthetic-key","jwk":{}}"#,
        r#"{"alg":"ES256","kid":"synthetic-key","x5u":"https://attacker.test/key"}"#,
    ] {
        assert!(verify_token(
            &client,
            &signer.sign_raw(header, &body),
            auth.nonce(),
            "synthetic-access",
            &signer.jwks(),
            NOW
        )
        .is_err());
    }
    let duplicate = body.replacen('{', r#"{"sub":"different-subject","#, 1);
    let token = signer.sign_raw(r#"{"alg":"ES256","kid":"synthetic-key"}"#, &duplicate);
    assert!(verify_token(
        &client,
        &token,
        auth.nonce(),
        "synthetic-access",
        &signer.jwks(),
        NOW
    )
    .is_err());
    for token in [
        "".into(),
        "a.b.c.d".into(),
        "x".repeat(32769),
        format!("{}=", signer.sign(&claims(&client, &auth, NOW))),
    ] {
        assert!(verify_token(
            &client,
            &token,
            auth.nonce(),
            "synthetic-access",
            &signer.jwks(),
            NOW
        )
        .is_err());
    }
}

#[test]
fn jwks_rejects_missing_ambiguous_or_inappropriate_key_material() {
    let signer = Signer::new();
    let client = client();
    let auth = authorization(&client);
    let token = signer.sign(&claims(&client, &auth, NOW));
    for (field, value) in [
        ("kty", json!("RSA")),
        ("crv", json!("P-384")),
        ("alg", json!("HS256")),
        ("use", json!("enc")),
        ("key_ops", json!(["sign"])),
        ("d", json!("private-key")),
        ("x", json!("")),
        ("y", json!("AA")),
        ("kid", json!("other")),
    ] {
        let mut key = signer.jwk();
        key[field] = value;
        let keys = serde_json::to_vec(&json!({"keys":[key]})).unwrap();
        assert!(
            verify_token(
                &client,
                &token,
                auth.nonce(),
                "synthetic-access",
                &keys,
                NOW
            )
            .is_err(),
            "field={field}"
        );
    }
    for keys in [
        json!({"keys":[]}),
        json!({"keys":[signer.jwk(),signer.jwk()]}),
        json!({"keys":vec![signer.jwk();33]}),
    ] {
        assert!(verify_token(
            &client,
            &token,
            auth.nonce(),
            "synthetic-access",
            &serde_json::to_vec(&keys).unwrap(),
            NOW
        )
        .is_err());
    }
    let key = signer
        .jwk()
        .to_string()
        .replacen('{', r#"{"kid":"other","#, 1);
    let keys = format!("{{\"keys\":[{key}]}}");
    assert!(verify_token(
        &client,
        &token,
        auth.nonce(),
        "synthetic-access",
        keys.as_bytes(),
        NOW
    )
    .is_err());
    let keys = json!({"keys":[{"kid":"unrelated-rsa","kty":"RSA"},signer.jwk()]});
    assert!(verify_token(
        &client,
        &token,
        auth.nonce(),
        "synthetic-access",
        &serde_json::to_vec(&keys).unwrap(),
        NOW
    )
    .is_ok());
}
