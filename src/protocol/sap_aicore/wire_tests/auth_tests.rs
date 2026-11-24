//! Real local HTTP token exchange, bounded reuse and secret-revision isolation.
use super::*;
#[tokio::test]
async fn sap_aicore_token_singleflight_expiry_and_secret_replacement() {
    let peer = peer::Peer::new("chat", false, false, 200).await;
    *peer.token.lock().unwrap() = (
        200,
        json!({"access_token":"synthetic-sap-access","token_type":"Bearer","expires_in":1})
            .to_string()
            .into_bytes(),
    );
    let cache = super::super::auth::TokenCache::default();
    let http = peer.client();
    let mut payload = super::super::tests::payload();
    super::super::test_support::origin(peer.origin.clone(), async {
        let (a, b) = tokio::join!(cache.get(&http, &payload), cache.get(&http, &payload));
        assert_eq!(a.unwrap(), "synthetic-sap-access");
        assert!(b.is_ok());
        assert_eq!(peer.oauth.lock().unwrap().len(), 1);
        tokio::time::sleep(Duration::from_millis(1100)).await;
        assert!(cache.get(&http, &payload).await.is_ok());
        assert_eq!(peer.oauth.lock().unwrap().len(), 2);
        payload.api_key = payload
            .api_key
            .replace("synthetic-sap-secret", "replacement-sap-secret");
        assert!(cache.get(&http, &payload).await.is_ok());
        let calls = peer.oauth.lock().unwrap();
        assert_eq!(calls.len(), 3);
        use base64::Engine;
        assert_eq!(
            calls[0].0["authorization"],
            format!(
                "Basic {}",
                base64::engine::general_purpose::STANDARD
                    .encode("synthetic-sap-client:synthetic-sap-secret")
            )
        );
        assert_ne!(calls[0].0["authorization"], calls[2].0["authorization"]);
        for (headers, body) in calls.iter() {
            assert_eq!(body, b"grant_type=client_credentials");
            assert!(!headers.contains_key("cookie"));
            assert!(!headers.contains_key("AI-Resource-Group"));
        }
    })
    .await;
    assert!(peer.calls.lock().unwrap().is_empty());
    peer.stop().await;
}
#[tokio::test]
async fn sap_aicore_token_slots_are_isolated_by_deployment_and_resource_group() {
    let peer = peer::Peer::new("chat", false, false, 200).await;
    let cache = super::super::auth::TokenCache::default();
    let http = peer.client();
    let first = super::super::tests::payload();
    let mut deployment = first.clone();
    deployment.base_url = first.base_url.replace("deployment1", "deployment2");
    let mut group = first.clone();
    group
        .headers
        .insert("AI-Resource-Group".into(), "group-b".into());
    super::super::test_support::origin(peer.origin.clone(), async {
        for (name, payload) in [
            ("first", &first),
            ("deployment", &deployment),
            ("group", &group),
        ] {
            *peer.token.lock().unwrap() = (
                200,
                json!({"access_token": name, "token_type":"Bearer", "expires_in":3600})
                    .to_string()
                    .into_bytes(),
            );
            assert_eq!(cache.get(&http, payload).await.unwrap(), name);
        }
        for (name, payload) in [
            ("group", &group),
            ("first", &first),
            ("deployment", &deployment),
        ] {
            assert_eq!(cache.get(&http, payload).await.unwrap(), name);
        }
        assert_eq!(peer.oauth.lock().unwrap().len(), 3);
    })
    .await;
    assert!(peer.calls.lock().unwrap().is_empty());
    peer.stop().await;
}

#[tokio::test]
async fn sap_aicore_token_failures_are_redacted_not_cached_and_never_generate() {
    let peer = peer::Peer::new("chat", false, false, 200).await;
    let cache = super::super::auth::TokenCache::default();
    let http = peer.client();
    let payload = super::super::tests::payload();
    super::super::test_support::origin(peer.origin.clone(), async {
        for (status, raw) in [
            (401, b"synthetic-sap-secret".to_vec()),
            (302, vec![]),
            (200, b"{bad".to_vec()),
            (
                200,
                json!({"access_token":"synthetic-sap-secret","token_type":"bearer","expires_in":0})
                    .to_string()
                    .into_bytes(),
            ),
            (200, vec![b'x'; 65537]),
        ] {
            *peer.token.lock().unwrap() = (status, raw);
            let error = cache.get(&http, &payload).await.unwrap_err();
            assert!(!error.message.contains("synthetic-sap-secret"));
            assert!(!error.retryable);
        }
        assert_eq!(peer.oauth.lock().unwrap().len(), 5);
        assert!(peer.calls.lock().unwrap().is_empty());
        *peer.token.lock().unwrap() = (
            200,
            json!({"access_token":"recovered","token_type":"bearer","expires_in":3600})
                .to_string()
                .into_bytes(),
        );
        assert_eq!(cache.get(&http, &payload).await.unwrap(), "recovered");
    })
    .await;
    peer.stop().await;
}

#[test]
fn sap_aicore_x509_imports_bound_service_key() {
    use super::super::{test_support::BASE, Endpoint, ServiceKey};
    let endpoint = Endpoint::parse(BASE).unwrap();
    let mut key = mtls_fixtures::service_key(false);
    assert!(ServiceKey::parse(&key.to_string(), &endpoint).is_ok());
    for (field, value) in [
        ("clientsecret", json!("mixed-auth-must-not-fallback")),
        (
            "certurl",
            json!("https://synthetic.authentication.eu10.hana.ondemand.com"),
        ),
        (
            "certurl",
            json!("https://synthetic.authentication.cert.eu10.hana.ondemand.com.invalid"),
        ),
        (
            "url",
            json!("https://foreign.authentication.eu10.hana.ondemand.com"),
        ),
        ("key", json!("invalid-private-key")),
        ("certificate", json!("invalid-leaf")),
        ("certUrl", key["certurl"].clone()), // cites the same field twice
    ] {
        let mut malformed = key.clone();
        malformed[field] = value;
        assert!(
            ServiceKey::parse(&malformed.to_string(), &endpoint).is_err(),
            "{field}"
        );
    }
    key.as_object_mut().unwrap().remove("url");
    key["certUrl"] = key.as_object_mut().unwrap().remove("certurl").unwrap();
    assert!(ServiceKey::parse(&key.to_string(), &endpoint).is_ok());
}
