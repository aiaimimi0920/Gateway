use super::{fixture::*, *};
use serde_json::{json, Value};
use std::{collections::HashMap, time::Duration};

#[tokio::test]
async fn valid_non_200_success_is_accepted_but_empty_or_bad_2xx_never_yields_credentials() {
    let http = LoginTransport::new().unwrap();
    for (code_status, user_status) in [(201, 202), (202, 201), (299, 299)] {
        let mut created = code();
        created.status = code_status;
        let mut approved = user();
        approved.status = user_status;
        let mock = Mock::new(vec![created, approved]).await;
        let mut challenge = mock.begin(&http).await;
        assert!(matches!(
            TEST_ORIGIN
                .scope(mock.origin.clone(), challenge.poll(&http))
                .await
                .unwrap(),
            LoginPoll::Authorized(_)
        ));
        assert_eq!(mock.count(), 2);
        mock.stop().await;
    }
    for status in [201, 204, 299] {
        let mut empty = Spec::json(status, json!({}));
        empty.body.clear();
        let mock = Mock::new(vec![empty]).await;
        assert_eq!(
            TEST_ORIGIN
                .scope(mock.origin.clone(), LoginChallenge::start(&http, "device"))
                .await
                .unwrap_err(),
            LoginError::InvalidResponse
        );
        assert_eq!(mock.count(), 1);
        mock.stop().await;
        let mut invalid_user = user();
        invalid_user.status = status;
        invalid_user.body = b"{\"user\":{}}".to_vec();
        let mock = Mock::new(vec![code(), invalid_user]).await;
        let mut challenge = mock.begin(&http).await;
        assert_eq!(
            TEST_ORIGIN
                .scope(mock.origin.clone(), challenge.poll(&http))
                .await
                .unwrap_err(),
            LoginError::InvalidResponse
        );
        assert_eq!(mock.count(), 2);
        mock.stop().await;
    }
}

#[tokio::test]
async fn code_and_status_use_exact_product_wire_without_inherited_auth_and_claim_once() {
    let mock = Mock::new(vec![code(), user()]).await;
    let http = LoginTransport::new().unwrap();
    let mut challenge = mock.begin(&http).await;
    let result = TEST_ORIGIN
        .scope(mock.origin.clone(), challenge.poll(&http))
        .await
        .unwrap();
    let LoginPoll::Authorized(material) = result else {
        panic!("expected credentials");
    };
    assert_eq!(material.auth_token(), "synthetic-token");
    assert_eq!(
        challenge.poll(&http).await.unwrap_err(),
        LoginError::Consumed
    );
    let calls = mock.calls.lock().unwrap().clone();
    assert_eq!(calls.len(), 2);
    assert_eq!(calls[0].0, "POST");
    assert_eq!(calls[0].1.path(), "/api/auth/cli/code");
    assert_eq!(
        serde_json::from_slice::<Value>(&calls[0].3).unwrap(),
        json!({"fingerprintId":"enhanced-synthetic-device"})
    );
    assert_eq!(calls[1].0, "GET");
    assert_eq!(calls[1].1.path(), "/api/auth/cli/status");
    assert!(calls[1].3.is_empty());
    let query: HashMap<String, String> =
        url::form_urlencoded::parse(calls[1].1.query().unwrap().as_bytes())
            .into_owned()
            .collect();
    assert_eq!(query.len(), 3);
    assert_eq!(query["expiresAt"], "opaque +/= expiry:原样");
    assert_eq!(query["fingerprintHash"], "synthetic-hash+/=");
    assert_eq!(query["fingerprintId"], "enhanced-synthetic-device");
    for call in calls {
        for key in ["authorization", "cookie", "x-api-key"] {
            assert!(!call.2.contains_key(key));
        }
    }
    mock.stop().await;
}

#[tokio::test]
async fn pending_401_and_empty_user_are_not_invalidated_tokens_and_poll_is_paced() {
    for pending in [
        Spec::json(401, json!({"error":"synthetic-pending"})),
        Spec::json(200, json!({"user":null})),
    ] {
        let mock = Mock::new(vec![code(), pending, user()]).await;
        let http = LoginTransport::new().unwrap();
        let mut challenge = mock.begin(&http).await;
        assert!(matches!(
            TEST_ORIGIN
                .scope(mock.origin.clone(), challenge.poll(&http))
                .await
                .unwrap(),
            LoginPoll::Pending
        ));
        let started = std::time::Instant::now();
        assert!(matches!(
            TEST_ORIGIN
                .scope(mock.origin.clone(), challenge.poll(&http))
                .await
                .unwrap(),
            LoginPoll::Authorized(_)
        ));
        assert!(started.elapsed() >= Duration::from_millis(4900));
        assert_eq!(mock.count(), 3);
        mock.stop().await;
    }
}

#[tokio::test]
async fn malformed_or_cross_attempt_user_never_yields_material_or_a_second_claim() {
    for replacement in [
        json!({}),
        json!([]),
        json!({"id":"u","email":"e","authToken":"token",
        "fingerprintId":"other","fingerprintHash":"synthetic-hash+/="}),
    ] {
        let mock = Mock::new(vec![code(), Spec::json(200, json!({"user":replacement}))]).await;
        let http = LoginTransport::new().unwrap();
        let mut challenge = mock.begin(&http).await;
        assert_eq!(
            TEST_ORIGIN
                .scope(mock.origin.clone(), challenge.poll(&http))
                .await
                .unwrap_err(),
            LoginError::InvalidResponse
        );
        assert!(challenge.poll(&http).await.is_err());
        assert_eq!(mock.count(), 2);
        mock.stop().await;
    }
}

#[tokio::test]
async fn explicit_bad_device_is_rejected_before_network_and_http_errors_are_not_retried() {
    let http = LoginTransport::new().unwrap();
    let mock = Mock::new(vec![]).await;
    for device in ["", "with space", "private\nheader"] {
        assert_eq!(
            TEST_ORIGIN
                .scope(mock.origin.clone(), LoginChallenge::start(&http, device))
                .await
                .unwrap_err(),
            LoginError::InvalidInput
        );
    }
    assert_eq!(mock.count(), 0);
    mock.stop().await;
    for status in [400, 401, 403, 408, 429, 500, 502, 503, 504] {
        let mock = Mock::new(vec![Spec::json(
            status,
            json!({"error":"synthetic-private"}),
        )])
        .await;
        let error = TEST_ORIGIN
            .scope(mock.origin.clone(), LoginChallenge::start(&http, "device"))
            .await
            .unwrap_err();
        assert_eq!(error, LoginError::Rejected);
        assert!(!error.to_string().contains("synthetic-private"));
        assert_eq!(mock.count(), 1);
        mock.stop().await;
    }
}
