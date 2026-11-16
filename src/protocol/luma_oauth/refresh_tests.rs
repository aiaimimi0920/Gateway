use super::*;
use crate::protocol::luma_oauth::{transport_fixture::*, ISSUER, RESOURCE};
use serde_json::json;

fn context() -> AuthorizationContext {
    serde_json::from_value(json!({"issuer":ISSUER,"resource":RESOURCE,
        "token_endpoint":TOKEN_ENDPOINT,"client_id":"refresh-client","offline_access":true,
        "redirect_uri":"http://127.0.0.1:34567/callback","expires_at":"2020-01-01T00:00:00Z"}))
    .unwrap()
}
fn grant() -> RefreshGrant {
    RefreshGrant::new(context(), "old-refresh".into()).unwrap()
}
fn reply(body: &str) -> Reply {
    Reply {
        body: body.into(),
        ..Default::default()
    }
}

#[tokio::test]
async fn luma_refresh_rotates_material_without_requiring_unexpired_access() {
    let mock = Mock::new(reply(r#"{"access_token":"new-access","token_type":"Bearer","expires_in":3600,"refresh_token":"new-refresh"}"#)).await;
    let outcome = grant()
        .exchange_at(&http(), &mock.endpoint, DEADLINE)
        .await
        .unwrap();
    assert!(!format!("{outcome:?}").contains("new-refresh"));
    assert!(!format!("{outcome:?}").contains("new-access"));
    let (material, refresh) = outcome.into_parts();
    assert_eq!(material.access_token(), "new-access");
    assert!(material
        .expires_at()
        .is_some_and(|at| at > OffsetDateTime::now_utc()));
    assert_eq!(material.client_id(), "refresh-client");
    assert_eq!(material.redirect_uri(), "http://127.0.0.1:34567/callback");
    assert_eq!(refresh, "new-refresh");
    assert!(material.offline_access());
    assert!(AuthorizationContext::from_material(&material)
        .unwrap()
        .offline_access());
    assert_eq!(mock.count(), 1);
    mock.stop().await;
}

#[tokio::test]
async fn luma_refresh_exact_public_form_retains_token_only_when_omitted() {
    let original = "opaque refresh+&=/%:token";
    let mock = Mock::new(reply(
        r#"{"access_token":"new-access","token_type":"bearer","id_token":"unverified"}"#,
    ))
    .await;
    let outcome = RefreshGrant::new(context(), original.into())
        .unwrap()
        .exchange_at(&http(), &mock.endpoint, DEADLINE)
        .await
        .unwrap();
    assert!(!format!("{outcome:?}").contains(original));
    let (material, refresh) = outcome.into_parts();
    assert_eq!(refresh, original);
    assert!(material.expires_at().is_none());
    {
        let calls = mock.calls.lock().unwrap();
        assert_eq!(calls.len(), 1);
        for header in ["authorization", "cookie", "x-api-key"] {
            assert!(!calls[0].0.contains_key(header));
        }
        let form: std::collections::HashMap<String, String> =
            url::form_urlencoded::parse(calls[0].1.as_bytes())
                .into_owned()
                .collect();
        assert_eq!(form.len(), 4);
        assert_eq!(form["grant_type"], "refresh_token");
        assert_eq!(form["refresh_token"], original);
        assert_eq!(form["client_id"], "refresh-client");
        assert_eq!(form["resource"], RESOURCE);
    }
    mock.stop().await;
}

#[test]
fn luma_refresh_rejects_invalid_input_without_secret_echo() {
    for token in [
        String::new(),
        "secret\n".into(),
        "secret\0".into(),
        "秘密".into(),
        "x".repeat(32_769),
    ] {
        let error = RefreshGrant::new(context(), token).err().unwrap();
        assert_eq!(error, OAuthError::InvalidInput);
        assert!(!error.to_string().contains("secret"));
    }
    assert!(RefreshGrant::new(context(), "x".repeat(32_768)).is_ok());
}

#[tokio::test]
async fn luma_refresh_rejects_malformed_rotation_without_falling_back() {
    for body in [
        r#"{"access_token":"new","token_type":"Bearer","refresh_token":null}"#.to_owned(),
        r#"{"access_token":"new","token_type":"Bearer","refresh_token":42}"#.to_owned(),
        r#"{"access_token":"new","token_type":"Bearer","refresh_token":""}"#.to_owned(),
        r#"{"access_token":"new","token_type":"Bearer","refresh_token":"secret\n"}"#.to_owned(),
        r#"{"access_token":"new","token_type":"Bearer","refresh_token":"a","refresh_token":"b"}"#
            .to_owned(),
        r#"{"access_token":"new","access_token":"other","token_type":"Bearer"}"#.to_owned(),
        r#"{"access_token":"new","token_type":"Basic","refresh_token":"secret"}"#.to_owned(),
        r#"{"access_token":"new","token_type":"Bearer","expires_in":0}"#.to_owned(),
        r#"{"access_token":"new","token_type":"Bearer","error":"private-error"}"#.to_owned(),
        json!({"access_token":"new","token_type":"Bearer","refresh_token":"x".repeat(32_769)})
            .to_string(),
    ] {
        let mock = Mock::new(reply(&body)).await;
        let error = grant()
            .exchange_at(&http(), &mock.endpoint, DEADLINE)
            .await
            .unwrap_err();
        assert_eq!(error, OAuthError::InvalidResponse);
        assert!(!error.to_string().contains("private-error"));
        assert_eq!(mock.count(), 1);
        mock.stop().await;
    }
}

#[tokio::test]
async fn luma_refresh_never_follows_redirects_or_retries_rejections() {
    let target = Mock::new(reply("unused")).await;
    for status in [301, 302, 303, 307, 308, 400, 401, 429, 503] {
        let mock = Mock::new(Reply {
            status,
            location: Some(target.endpoint.clone()),
            ..reply("private-error")
        })
        .await;
        let error = grant()
            .exchange_at(&http(), &mock.endpoint, DEADLINE)
            .await
            .unwrap_err();
        assert_eq!(
            error,
            if status < 400 {
                OAuthError::RedirectRejected
            } else {
                OAuthError::Rejected
            }
        );
        assert_eq!(mock.count(), 1);
        assert_eq!(target.count(), 0);
        mock.stop().await;
    }
    target.stop().await;
}

#[tokio::test]
async fn luma_refresh_bounds_decoded_body_and_zero_budget_never_sends() {
    let mock = Mock::new(reply(&"x".repeat(super::super::token_response::LIMIT + 1))).await;
    assert_eq!(
        grant()
            .exchange_at(&http(), &mock.endpoint, DEADLINE)
            .await
            .unwrap_err(),
        OAuthError::ResponseTooLarge
    );
    assert_eq!(
        grant()
            .exchange_at(&http(), &mock.endpoint, Duration::ZERO)
            .await
            .unwrap_err(),
        OAuthError::Timeout
    );
    assert_eq!(mock.count(), 1);
    mock.stop().await;
}

#[tokio::test]
async fn luma_refresh_deadline_covers_headers_and_body() {
    for (header_delay, body_delay) in [
        (Duration::from_millis(400), Duration::ZERO),
        (Duration::ZERO, Duration::from_millis(400)),
    ] {
        let mock = Mock::new(Reply {
            header_delay,
            body_delay,
            ..reply(r#"{"access_token":"new","token_type":"Bearer"}"#)
        })
        .await;
        let error = grant()
            .exchange_at(&http(), &mock.endpoint, Duration::from_millis(100))
            .await
            .unwrap_err();
        assert!(matches!(error, OAuthError::Timeout | OAuthError::Transport));
        assert_eq!(mock.count(), 1);
        mock.stop().await;
    }
}

#[tokio::test]
async fn luma_refresh_cancellation_releases_body_without_background_retry() {
    let mock = Mock::new(Reply {
        body_delay: Duration::from_secs(300),
        ..reply("unused")
    })
    .await;
    let endpoint = mock.endpoint.clone();
    let task = tokio::spawn(async move { grant().exchange_at(&http(), &endpoint, DEADLINE).await });
    mock.wait_called().await;
    tokio::time::sleep(Duration::from_millis(50)).await;
    task.abort();
    assert!(task.await.unwrap_err().is_cancelled());
    mock.wait_body_dropped().await;
    assert_eq!(mock.count(), 1);
    mock.stop().await;
}
