use super::*;
use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
use ring::signature::{Ed25519KeyPair, KeyPair, UnparsedPublicKey, ED25519};
use serde_json::{json, Value};

pub(super) fn body() -> Value {
    json!({"protocol":1,"version":"synthetic-v1","issuedAt":1000,"refreshAt":2000,
        "fetchId":"synthetic-fetch","plansUrl":"https://example.invalid/plans",
        "rows":[row("open", "open"),row("locked", "locked"),row("future", "open")]})
}
fn row(key: &str, access: &str) -> Value {
    let mut row = json!({"key":key,"handle":format!("fbm1.synthetic-{key}"),
        "displayName":key,"tagline":"synthetic","badges":[],"multimodal":false,
        "premium":false,"dataUse":"service","access":access,"sortOrder":0});
    if key == "future" {
        row["opensAt"] = json!(1001);
    }
    row
}

#[test]
fn catalog_selects_stable_key_but_sends_handle_and_same_fetch() {
    let catalog = ModelCatalog::parse(&serde_json::to_vec(&body()).unwrap()).unwrap();
    let selected = catalog.select("open").unwrap();
    assert_eq!(selected.key, "open");
    assert_eq!(selected.handle, "fbm1.synthetic-open");
    assert_eq!(selected.fetch_id, Some("synthetic-fetch"));
    assert_eq!(
        catalog.select("locked").err(),
        Some(CatalogError::ModelUnavailable)
    );
    assert_eq!(
        catalog.select("future").err(),
        Some(CatalogError::ModelUnavailable)
    );
    assert_eq!(
        catalog.select("fbm1.synthetic-open").err(),
        Some(CatalogError::ModelUnavailable)
    );
}

#[test]
fn refresh_time_is_not_handle_expiry_and_missing_fetch_id_is_supported() {
    for fetch_id in [None, Some("")] {
        let mut value = body();
        value.as_object_mut().unwrap().remove("fetchId");
        if let Some(id) = fetch_id {
            value["fetchId"] = json!(id);
        }
        let catalog = ModelCatalog::parse(&serde_json::to_vec(&value).unwrap()).unwrap();
        assert!(catalog.needs_refresh(u64::MAX));
        assert!(!catalog.needs_refresh(1999));
        assert_eq!(catalog.select("open").unwrap().fetch_id, None);
        assert!(
            catalog.select("future").is_err(),
            "local future clock cannot open a row"
        );
    }
}

pub(super) fn pair() -> (Vec<u8>, Vec<u8>) {
    let material = Ed25519KeyPair::generate_pkcs8(&ring::rand::SystemRandom::new()).unwrap();
    let pair = Ed25519KeyPair::from_pkcs8(material.as_ref()).unwrap();
    (
        material.as_ref().to_vec(),
        pair.public_key().as_ref().to_vec(),
    )
}

#[test]
fn signing_covers_exact_utf8_body_method_path_timestamp_and_fetch() {
    let (private, public) = pair();
    let device = DeviceKey::from_pkcs8(&private, &public).unwrap();
    let body = "{\"message\":\"中文\", \"n\":1}".as_bytes();
    let signature = URL_SAFE_NO_PAD
        .decode(device.sign(
            "post",
            "/api/v1/chat/completions",
            body,
            Some("fetch"),
            1234,
        ))
        .unwrap();
    let hash = hex::encode(ring::digest::digest(&ring::digest::SHA256, body));
    let expected =
        format!("freebuff-device-v1\nPOST\n/api/v1/chat/completions\n1234\n{hash}\nfetch");
    UnparsedPublicKey::new(&ED25519, &public)
        .verify(expected.as_bytes(), &signature)
        .unwrap();
    for changed in [
        expected.replace("POST", "GET"),
        expected.replace("1234", "1235"),
        expected.replace("fetch", "other"),
        expected.replace("completions", "session"),
    ] {
        assert!(UnparsedPublicKey::new(&ED25519, &public)
            .verify(changed.as_bytes(), &signature)
            .is_err());
    }
}
