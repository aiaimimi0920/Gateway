use super::{test_support::*, *};
use serde_json::json;

#[tokio::test]
async fn authorization_uses_proprietary_fragment_and_exact_callback() {
    let auth = Authorization::new(client(), "http://127.0.0.1:12345/auth/callback").unwrap();
    let login = auth.authorization_url();
    assert!(login.starts_with(&format!("{LOGIN_URL}?source=electron&")));
    for forbidden in [
        "client_id=",
        "scope=",
        "code_challenge=",
        "official",
        "return_to=",
    ] {
        assert!(!login.contains(forbidden));
    }
    let callback = callback(&login, "synthetic-code");
    assert_eq!(auth.callback_code(&callback).unwrap(), "synthetic-code");
    for invalid in [
        callback.replace("127.0.0.1", "localhost"),
        callback.replace("12345", "12346"),
        callback.replace("/auth/callback", "/oauth/callback"),
        format!("{callback}&state=duplicate"),
        format!("{callback}&code=duplicate"),
        format!("{callback}#fragment"),
        format!("{callback}&error=denied"),
    ] {
        assert!(auth.callback_code(&invalid).is_err(), "{invalid}");
    }
    assert_eq!(
        auth.callback_code("http://127.0.0.1:12345/auth/callback?code=c&state=wrong")
            .unwrap_err(),
        LoginError::StateMismatch
    );
    assert!(auth.into_code_grant(&callback).is_ok());
}
#[tokio::test]
async fn authorization_rejects_non_loopback_or_noncanonical_targets() {
    for uri in [
        "https://127.0.0.1:123/auth/callback",
        "http://localhost:123/auth/callback",
        "http://127.0.0.1/auth/callback",
        "http://127.0.0.1:0/auth/callback",
        "http://127.0.0.1:123/other",
        "http://u:p@127.0.0.1:123/auth/callback",
        "http://127.0.0.1:123/auth/callback?x=1",
        "http://127.0.0.1:123/auth/callback#x",
        " http://127.0.0.1:123/auth/callback",
    ] {
        assert_eq!(
            Authorization::new(client(), uri).unwrap_err(),
            LoginError::InvalidInput
        );
    }
    assert_eq!(
        Client::new(uuid::Uuid::nil(), rquest::Client::builder()).unwrap_err(),
        LoginError::InvalidInput
    );
}
#[test]
fn received_session_preserves_receipt_before_account_admission() {
    let mut value = tokens();
    value["data"].as_object_mut().unwrap().remove("accountMode");
    let mut received = ReceivedSession::capture(client(), value, None).unwrap();
    assert_eq!(
        received.admit().unwrap_err(),
        LoginError::AccountContextRequired
    );
    assert_eq!(
        received.admit().unwrap_err(),
        LoginError::AccountContextRequired
    );
    assert!(!format!("{received:?}").contains("synthetic"));
    let session = receipt().admit().unwrap();
    assert_eq!(session.user_id(), "synthetic-user");
    assert_eq!(session.access_token(), "synthetic-access");
    assert!(!format!("{session:?}").contains("synthetic"));
    assert!(format!("{session:?}").contains("expiry: unknown"));
}
#[test]
fn personal_admission_rejects_enterprise_and_ambiguous_identity() {
    for (path, replacement) in [
        ("accountMode", json!("enterprise")),
        ("accountMode", json!("unknown")),
        ("enterpriseContext", json!({"accountMode":"enterprise"})),
        ("organizationContext", json!({"enterpriseId":7})),
        ("quota", json!({"subscriptionStatus":"enterprise"})),
        ("user", json!({"nickname":"display-only"})),
        ("user", json!({"yid":"bad\nidentity"})),
        ("refreshToken", json!(null)),
        ("accessToken", json!("bad\r\ntoken")),
    ] {
        let mut value = tokens();
        value["data"][path] = replacement;
        let mut received = ReceivedSession::capture(client(), value, None).unwrap();
        assert!(received.admit().is_err(), "{path}");
    }
}
#[test]
fn refresh_preserves_absent_successor_and_detects_identity_switch() {
    for successor in [
        None,
        Some(json!(null)),
        Some(json!("")),
        Some(json!("  ")),
        Some(json!("synthetic-next-refresh")),
    ] {
        let mut data = json!({"accessToken":"synthetic-next-access"});
        if let Some(value) = successor.clone() {
            data["refreshToken"] = value;
        }
        let mut received = ReceivedSession::capture(
            client(),
            json!({"code":0,"data":data}),
            Some(receipt().admit().unwrap()),
        )
        .unwrap();
        let next = received.admit().unwrap();
        assert_eq!(next.access_token(), "synthetic-next-access");
        assert_eq!(next.user_id(), "synthetic-user");
        assert_eq!(
            next.refresh_token(),
            if successor == Some(json!("synthetic-next-refresh")) {
                "synthetic-next-refresh"
            } else {
                "synthetic-refresh"
            }
        );
    }
    let mut received = ReceivedSession::capture(
        client(),
        json!({"code":0,"data":{
        "accessToken":"next", "refreshToken":"next-r", "user":{"yid":"another-user"}}}),
        Some(receipt().admit().unwrap()),
    )
    .unwrap();
    assert_eq!(received.admit().unwrap_err(), LoginError::IdentityMismatch);
    assert!(!format!("{received:?}").contains("next-r"));
}
#[test]
fn only_exact_401_can_prepare_a_refresh() {
    for status in [200, 400, 403, 429, 500] {
        assert_eq!(
            receipt()
                .admit()
                .unwrap()
                .into_refresh_grant("synthetic-access", status)
                .unwrap_err(),
            LoginError::Rejected
        );
    }
    assert_eq!(
        receipt()
            .admit()
            .unwrap()
            .into_refresh_grant("old-access", 401)
            .unwrap_err(),
        LoginError::IdentityMismatch
    );
    assert!(receipt()
        .admit()
        .unwrap()
        .into_refresh_grant("synthetic-access", 401)
        .is_ok());
}

#[test]
fn successful_receipt_can_only_deliver_one_refreshable_session() {
    let mut received = receipt();
    let session = received.admit().unwrap();
    assert_eq!(received.admit().unwrap_err(), LoginError::ReceiptConsumed);
    assert!(session.into_refresh_grant("synthetic-access", 401).is_ok());
    let previous = receipt().admit().unwrap();
    let mut rotated = ReceivedSession::capture(
        client(),
        json!({"code":0,"data":{
        "accessToken":"next","refreshToken":"next-refresh"}}),
        Some(previous),
    )
    .unwrap();
    assert!(rotated.admit().is_ok());
    assert_eq!(rotated.admit().unwrap_err(), LoginError::ReceiptConsumed);
}
#[test]
fn catalog_reads_model_id_and_preserves_unknown_capabilities() {
    let id = "m".repeat(512);
    let rows = catalog::parse(json!({"code":0,"data":[
        {"modelId":id,"modelName":"模型","contextWindow":999,"supportsToolCalling":true},
        {"modelId":"opaque","supportsToolCalling":false,"runtimeProfile":"moonshot-kimi-k3"},
        {"modelId":"unavailable","accessible":false}
    ]}))
    .unwrap();
    assert_eq!(rows.len(), 2);
    assert_eq!(rows[0].model_id, id);
    assert_eq!(rows[0].supports_thinking, None);
    assert_eq!(rows[0].max_tokens, None);
    assert_eq!(rows[1].supports_tool_calling, Some(false));
    assert_eq!(rows[1].runtime_profile.as_deref(), Some("moonshot-kimi-k3"));
    assert!(catalog::parse(json!({"code":0,"data":[]}))
        .unwrap()
        .is_empty());
}
#[test]
fn catalog_rejects_legacy_aliases_bad_types_duplicates_and_limits() {
    for rows in [
        json!([{"id":"wrong"}]),
        json!([{"model":"wrong"}]),
        json!([{"modelId":"x","supportsToolCalling":"yes"}]),
        json!([{"modelId":"x","contextWindow":0}]),
        json!([{"modelId":"x"},{"modelId":"x"}]),
        json!([{"modelId":" x"}]),
        json!([{"modelId":"x".repeat(513)}]),
        json!([{"modelId":"x","requestCapabilities":[1]}]),
        json!(vec![json!({"modelId":"x"}); 2049]),
    ] {
        assert!(catalog::parse(json!({"code":0,"data":rows})).is_err());
    }
}
#[test]
fn shared_json_rejects_nested_duplicate_security_fields() {
    for bytes in [
        br#"{"code":0,"code":1}"#.as_slice(),
        br#"{"data":{"accessToken":"a","accessToken":"b"}}"#,
        br#"{"user":{"accountMode":"enterprise","accountMode":"personal"}}"#,
        br#"{"data":[{"modelId":"a","modelId":"b"}]}"#,
        br#"{} {}"#,
    ] {
        assert!(crate::protocol::unique_json::parse(bytes).is_err());
    }
    assert_eq!(
        crate::protocol::unique_json::parse(br#"{"x":[null,1,-1,1.5,true,"text"]}"#).unwrap(),
        json!({"x":[null,1,-1,1.5,true,"text"]})
    );
}
