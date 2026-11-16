//! Wire -> serialized context -> owned wire refresh, using only synthetic loopback peers.
//! This is not Console persistence, revision publication, scheduler or business-send acceptance.
use super::{
    authorization_tests::callback,
    material_tests::tokens,
    transport_fixture::{http, Mock, Reply},
    transport_test_support::test_endpoint,
    *,
};
use serde_json::json;

#[tokio::test]
async fn vertex_oauth_context_code_roundtrip_reconstructs_only_captured_client_for_refresh() {
    for secret in [None, Some("synthetic-client-secret")] {
        let mock = Mock::new(Reply {
            body: tokens().to_string(),
            ..Reply::default()
        })
        .await;
        let client = OAuthClient::new("captured-client", secret).unwrap();
        let target =
            VertexTarget::new("captured-project", "us-central1", Some("captured-billing")).unwrap();
        let attempt = AuthorizationAttempt::new(client.clone(), 43991).unwrap();
        let callback = callback(&attempt);
        let grant = attempt.finish(&callback).unwrap();
        let material = test_endpoint(mock.endpoint.clone(), grant.exchange(&http()))
            .await
            .unwrap();
        let original =
            AuthorizationContext::from_authorization(client, target.clone(), &material).unwrap();
        assert_eq!(mock.count(), 1);
        mock.stop().await;

        let stored = serde_json::to_vec(&original).unwrap();
        let restored: AuthorizationContext = serde_json::from_slice(&stored).unwrap();
        let mut reply = tokens();
        reply.as_object_mut().unwrap().remove("scope");
        reply["refresh_token_expires_in"] = json!(3600);
        reply["refresh_token"] = json!("new-synthetic-refresh");
        let mock = Mock::new(Reply {
            body: reply.to_string(),
            ..Reply::default()
        })
        .await;
        let grant = restored
            .refresh_grant(material.refresh_token().unwrap())
            .unwrap();
        let rotated = test_endpoint(mock.endpoint.clone(), grant.execute(&http()))
            .await
            .unwrap();
        let next = restored.rotated(&rotated).unwrap();
        assert!(restored.permits_rotation(&next));
        assert_eq!(next.target(), &target);
        assert!(next.scope_reported());
        assert!(next.refresh_expires_at().unwrap() < original.refresh_expires_at().unwrap());
        assert_eq!(rotated.refresh_token(), Some("new-synthetic-refresh"));
        {
            let calls = mock.calls.lock().unwrap();
            assert_eq!(calls.len(), 1);
            let form: std::collections::HashMap<_, _> =
                url::form_urlencoded::parse(calls[0].1.as_bytes()).collect();
            assert_eq!(form["client_id"], "captured-client");
            assert_eq!(form["refresh_token"], "refresh-private");
            assert_eq!(form.get("client_secret").map(|s| s.as_ref()), secret);
            for forbidden in [
                "project",
                "location",
                "quota_project",
                "scope",
                "access_token",
            ] {
                assert!(!form.contains_key(forbidden));
            }
            for forbidden in [
                "authorization",
                "cookie",
                "x-api-key",
                "x-goog-user-project",
            ] {
                assert!(!calls[0].0.contains_key(forbidden));
            }
        }
        mock.stop().await;
    }
}

#[tokio::test]
async fn vertex_oauth_context_roundtrip_refresh_never_resets_known_deadline_or_original_token() {
    let original = super::context_tests::material(Some(600), true);
    let context = AuthorizationContext::from_authorization(
        OAuthClient::new("explicit-client", None).unwrap(),
        VertexTarget::new("explicit-project", "global", None).unwrap(),
        &original,
    )
    .unwrap();
    let stored = serde_json::to_vec(&context).unwrap();
    for reported_deadline in [None, Some(7200)] {
        let restored: AuthorizationContext = serde_json::from_slice(&stored).unwrap();
        let mut reply = tokens();
        reply.as_object_mut().unwrap().remove("refresh_token");
        reply.as_object_mut().unwrap().remove("scope");
        reply
            .as_object_mut()
            .unwrap()
            .remove("refresh_token_expires_in");
        if let Some(seconds) = reported_deadline {
            reply["refresh_token_expires_in"] = json!(seconds);
        }
        let mock = Mock::new(Reply {
            body: reply.to_string(),
            ..Reply::default()
        })
        .await;
        let grant = restored.refresh_grant("synthetic-refresh").unwrap();
        let material = test_endpoint(mock.endpoint.clone(), grant.execute(&http()))
            .await
            .unwrap();
        let next = restored.rotated(&material).unwrap();
        assert_eq!(material.refresh_token(), Some("synthetic-refresh"));
        assert_eq!(
            next.refresh_expires_at().unwrap(),
            original.refresh_expires_at()
        );
        assert_eq!(next, context);
        assert!(context.permits_rotation(&next));
        assert_eq!(mock.count(), 1);
        mock.stop().await;
    }
}
