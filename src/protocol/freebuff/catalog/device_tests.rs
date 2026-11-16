use super::{account::Binding, tests::pair, *};
use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
use ring::signature::{UnparsedPublicKey, ED25519};
use std::sync::Arc;

#[test]
fn imports_webcrypto_pkcs8_v1_and_ring_v2_and_rejects_mismatched_keys() {
    // RFC 8032 Ed25519 test vector 1, wrapped in RFC 8410 PKCS#8 v1 (no embedded public key).
    let seed = "9d61b19deffd5a60ba844af492ec2cc44449c5697b326919703bac031cae7f60";
    let private = hex::decode(format!("302e020100300506032b657004220420{seed}")).unwrap();
    let public =
        hex::decode("d75a980182b10ab7d54bfed3c964073a0ee172f3daa62325af021a68f707511a").unwrap();
    let key = DeviceKey::from_pkcs8(&private, &public).unwrap();
    let signature = URL_SAFE_NO_PAD
        .decode(key.sign("GET", "/api/v1/freebuff/models", b"", None, 1))
        .unwrap();
    let expected = "freebuff-device-v1\nGET\n/api/v1/freebuff/models\n1\ne3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855\n";
    UnparsedPublicKey::new(&ED25519, &public)
        .verify(expected.as_bytes(), &signature)
        .unwrap();
    let (mut v2, public2) = pair();
    assert!(DeviceKey::from_pkcs8(&v2, &public2).is_ok());
    assert_eq!(
        DeviceKey::from_pkcs8(&v2, &public).err(),
        Some(CatalogError::InvalidKey)
    );
    *v2.last_mut().unwrap() ^= 1;
    assert_eq!(
        DeviceKey::from_pkcs8(&v2, &public2).err(),
        Some(CatalogError::InvalidKey)
    );
    for (private, public) in [
        (vec![], public2.clone()),
        (vec![0; 4097], public2.clone()),
        (private, vec![0; 31]),
    ] {
        assert_eq!(
            DeviceKey::from_pkcs8(&private, &public).err(),
            Some(CatalogError::InvalidKey)
        );
    }
}
#[test]
fn byte_whitespace_and_unicode_changes_invalidate_signature() {
    let (private, public) = pair();
    let key = DeviceKey::from_pkcs8(&private, &public).unwrap();
    let original = "{\"text\":\"中文\"}";
    let signature = URL_SAFE_NO_PAD
        .decode(key.sign(
            "POST",
            "/api/v1/chat/completions",
            original.as_bytes(),
            None,
            10,
        ))
        .unwrap();
    for changed in [
        "{\"text\": \"中文\"}",
        "{\"text\":\"中 文\"}",
        "{\"text\":\"中文\"}\n",
    ] {
        let hash = hex::encode(ring::digest::digest(
            &ring::digest::SHA256,
            changed.as_bytes(),
        ));
        let payload = format!("freebuff-device-v1\nPOST\n/api/v1/chat/completions\n10\n{hash}\n");
        assert!(UnparsedPublicKey::new(&ED25519, &public)
            .verify(payload.as_bytes(), &signature)
            .is_err());
    }
}
#[test]
fn registration_capability_is_bound_to_token_origin_and_client_not_only_key_id() {
    let origin = BusinessOrigin::CodebuffWww;
    let binding = Binding::new(origin, "synthetic-A", ClientSurface::Cli).unwrap();
    let (private, public) = pair();
    let key = Arc::new(DeviceKey::from_pkcs8(&private, &public).unwrap());
    let registration = DeviceRegistration::new(&binding, key.clone(), "private-key-id".into());
    for foreign in [
        Binding::new(origin, "synthetic-B", ClientSurface::Cli).unwrap(),
        Binding::new(BusinessOrigin::Codebuff, "synthetic-A", ClientSurface::Cli).unwrap(),
        Binding::new(origin, "synthetic-A", ClientSurface::Desktop).unwrap(),
    ] {
        assert_eq!(
            registration.catalog_headers(&foreign, 1).err(),
            Some(CatalogError::ScopeMismatch)
        );
    }
    let equivalent = Binding::new(origin, "synthetic-A", ClientSurface::Cli).unwrap();
    let headers = registration.catalog_headers(&equivalent, 1).unwrap();
    let debug = format!("{registration:?} {key:?} {headers:?}");
    for secret in [
        "synthetic-A",
        "private-key-id",
        &URL_SAFE_NO_PAD.encode(private),
    ] {
        assert!(!debug.contains(secret));
    }
}
#[test]
fn authority_input_is_explicit_and_does_not_accept_header_injection() {
    assert_eq!(BusinessOrigin::Codebuff.as_str(), "https://codebuff.com");
    assert_eq!(
        BusinessOrigin::CodebuffWww.as_str(),
        "https://www.codebuff.com"
    );
    for token in [
        "",
        " padded",
        "bad\r\nauthorization: attacker",
        "中文",
        &"x".repeat(16_385),
    ] {
        assert_eq!(
            CatalogSession::new(BusinessOrigin::Codebuff, token, ClientSurface::Cli).err(),
            Some(CatalogError::InvalidInput)
        );
    }
}
#[cfg(not(feature = "line-freebuff-web-reverse-api"))]
#[tokio::test]
async fn disabled_network_feature_cannot_bootstrap_or_register() {
    let mut session = CatalogSession::new(
        BusinessOrigin::CodebuffWww,
        "synthetic-A",
        ClientSurface::Cli,
    )
    .unwrap();
    assert_eq!(
        session
            .refresh(None, std::time::Duration::from_secs(1))
            .await
            .err(),
        Some(CatalogError::FeatureDisabled)
    );
    session.catalog =
        Some(ModelCatalog::parse(&serde_json::to_vec(&super::tests::body()).unwrap()).unwrap());
    let (private, public) = pair();
    let key = Arc::new(DeviceKey::from_pkcs8(&private, &public).unwrap());
    assert_eq!(
        session
            .register_device(key, std::time::Duration::from_secs(1))
            .await
            .err(),
        Some(CatalogError::FeatureDisabled)
    );
}
