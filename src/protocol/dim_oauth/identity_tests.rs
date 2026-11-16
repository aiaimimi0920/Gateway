//! Public synthetic vectors; no production key, signing secret or live identity is present.
use super::*;
use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
use serde_json::{json, Value};

fn fixture() -> Value {
    serde_json::from_str(include_str!(
        "../../../tests/fixtures/dim_oauth/rs256-vectors.json"
    ))
    .unwrap()
}
#[test]
fn dim_rs256_identity_verifies_signature_nonce_issuer_client_time_and_access_hash() {
    let f = fixture();
    let client = OAuthClient::new(Site::China, "permitted-gateway", &["openid"]).unwrap();
    let keys = serde_json::to_vec(&f["jwks"]).unwrap();
    let verify = |token: &str| {
        super::identity::verify(
            &client,
            token,
            f["nonce"].as_str().unwrap(),
            f["access"].as_str().unwrap(),
            &keys,
            f["now"].as_u64().unwrap(),
        )
    };
    let identity = verify(f["valid"].as_str().unwrap()).unwrap();
    assert_eq!(identity.issuer(), Site::China.issuer());
    assert_eq!(identity.subject(), "synthetic-subject");
    assert_eq!(identity.client_id(), "permitted-gateway");
    assert_eq!(identity.expires_at(), 1800003600);
    assert!(!format!("{identity:?}").contains("synthetic-subject"));
    for rejected in f["rejected"].as_array().unwrap() {
        assert!(
            verify(rejected["token"].as_str().unwrap()).is_err(),
            "{}",
            rejected["reason"]
        );
    }
    let token = f["valid"].as_str().unwrap();
    let (signed, signature) = token.rsplit_once('.').unwrap();
    let mut bytes = URL_SAFE_NO_PAD.decode(signature).unwrap();
    bytes[0] ^= 1;
    assert!(verify(&format!("{signed}.{}", URL_SAFE_NO_PAD.encode(bytes))).is_err());
    assert!(super::identity::verify(
        &client,
        token,
        "different-nonce",
        "synthetic-access",
        &keys,
        1800000100
    )
    .is_err());
    assert!(super::identity::verify(
        &client,
        token,
        "synthetic-nonce",
        "different-access",
        &keys,
        1800000100
    )
    .is_err());
}

#[test]
fn dim_jwks_rejects_ambiguous_private_weak_and_wrong_purpose_keys() {
    let f = fixture();
    let client = OAuthClient::new(Site::China, "permitted-gateway", &["openid"]).unwrap();
    let verify = |keys: Value| {
        super::identity::verify(
            &client,
            f["valid"].as_str().unwrap(),
            "synthetic-nonce",
            "synthetic-access",
            &serde_json::to_vec(&keys).unwrap(),
            1800000100,
        )
    };
    let key = f["jwks"]["keys"][0].clone();
    assert!(verify(json!({"keys":[key.clone(),key.clone()]})).is_err());
    assert!(verify(json!({"keys":[]})).is_err());
    for (field, value) in [
        ("kty", json!("EC")),
        ("alg", json!("HS256")),
        ("use", json!("enc")),
        ("key_ops", json!(["sign", "verify"])),
        ("d", json!("must-not-be-present")),
        ("n", json!("AQAB")),
        ("kid", json!("missing-key")),
    ] {
        let mut bad = key.clone();
        bad[field] = value;
        assert!(verify(json!({"keys":[bad]})).is_err(), "{field}");
    }
    let duplicate = format!(r#"{{"keys":[{},{}]}}"#, key, key);
    assert!(super::identity::verify(
        &client,
        f["valid"].as_str().unwrap(),
        "synthetic-nonce",
        "synthetic-access",
        duplicate.as_bytes(),
        1800000100
    )
    .is_err());
}
