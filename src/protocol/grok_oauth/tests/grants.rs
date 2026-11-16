//! Grant acceptance is separate from end-user JWT assurance and consent pre-selection.
use super::*;

fn access(body: Value) -> String {
    format!(
        "{}.{}.opaque",
        URL_SAFE_NO_PAD.encode(r#"{"alg":"ES256","typ":"JWT"}"#),
        URL_SAFE_NO_PAD.encode(body.to_string())
    )
}
fn callback(auth: &Authorization) -> String {
    format!(
        "{}?code=synthetic-code&state={}",
        auth.redirect_uri(),
        auth.state
    )
}
#[tokio::test]
async fn team_code_grant_keeps_endpoint_only_assurance_without_inventing_user_identity() {
    let fixture = wire::spawn_fixture(axum::http::StatusCode::OK, json!({"access_token":access(json!({"principal_type":"Team","principal_id":"private-team"})),"refresh_token":"private-refresh","expires_in":60})).await;
    let auth = identity_support::authorization(&fixture.client);
    let receipt = fixture
        .client
        .complete_code(&wire::http(), &auth, &callback(&auth), None)
        .await
        .unwrap();
    assert!(receipt.identity().is_none());
    assert_eq!(receipt.principal_hint().unwrap().id(), Some("private-team"));
    assert_eq!(receipt.assurance(), GrantAssurance::TokenEndpointOnly);
    assert!(!format!("{receipt:?}").contains("private-"));
    assert_eq!(fixture.calls.lock().unwrap().len(), 1);
}

#[tokio::test]
async fn consent_selection_is_not_a_pin_but_explicit_constraints_fail_before_jwks() {
    for constraint in [
        None,
        Some(PrincipalConstraint::for_id("different-team").unwrap()),
    ] {
        let fixture=wire::spawn_fixture(axum::http::StatusCode::OK,json!({"access_token":access(json!({"principalType":"Team","principalId":"actual-team"}))})).await;
        let client = fixture
            .client
            .clone()
            .with_principal(Principal::new("User", "preselected-user").unwrap());
        let auth = identity_support::authorization(&client);
        let result = client
            .complete_code(&wire::http(), &auth, &callback(&auth), constraint.as_ref())
            .await;
        if constraint.is_some() {
            assert!(matches!(result, Err(OAuthError::PrincipalMismatch)));
        } else {
            assert_eq!(
                result.unwrap().principal_hint().unwrap().id(),
                Some("actual-team")
            );
        }
        assert_eq!(fixture.calls.lock().unwrap().len(), 1);
    }
}

#[tokio::test]
async fn organization_incomplete_team_and_case_variants_do_not_skip_code_identity() {
    for principal in [
        json!({"principal_type":"Organization","principal_id":"org"}),
        json!({"principal_type":"team","principal_id":"team"}),
        json!({"principal_type":"Team"}),
        json!({"principal_id":"team"}),
    ] {
        let fixture = wire::spawn_fixture(
            axum::http::StatusCode::OK,
            json!({"access_token":access(principal)}),
        )
        .await;
        let auth = identity_support::authorization(&fixture.client);
        assert!(matches!(
            fixture
                .client
                .complete_code(&wire::http(), &auth, &callback(&auth), None)
                .await,
            Err(OAuthError::InvalidIdentity)
        ));
        assert_eq!(fixture.calls.lock().unwrap().len(), 1);
    }
}

fn challenge(client: &OAuthClient) -> DeviceChallenge {
    DeviceChallenge::parse(json!({"device_code":"synthetic-device","user_code":"TEST-CODE","verification_uri":"https://accounts.x.ai/device","expires_in":60,"interval":1}),client).unwrap()
}
#[tokio::test]
async fn opaque_dotted_access_token_is_not_required_to_be_a_jwt() {
    let fixture = wire::spawn_fixture(
        axum::http::StatusCode::OK,
        json!({"access_token":"opaque.session.token"}),
    )
    .await;
    let mut challenge = challenge(&fixture.client);
    let result = fixture
        .client
        .poll_device_receipt(&wire::http(), &mut challenge, None)
        .await;
    assert!(
        matches!(&result,Ok(GrantPoll::Authorized(receipt)) if receipt.principal_hint().is_none() && receipt.assurance()==GrantAssurance::TokenEndpointOnly),
        "opaque token must remain usable: {result:?}"
    );
    assert_eq!(fixture.calls.lock().unwrap().len(), 1);
}
#[tokio::test]
async fn device_receipts_allow_opaque_team_and_organization_without_forging_identity() {
    for token in [
        "opaque-access".into(),
        access(json!({"principal_type":"Team","principal_id":"team"})),
        access(json!({"principal_type":"Organization","principal_id":"org"})),
    ] {
        let fixture = wire::spawn_fixture(
            axum::http::StatusCode::OK,
            json!({"access_token":token,"id_token":"unverified-display-hint"}),
        )
        .await;
        let mut challenge = challenge(&fixture.client);
        let result = fixture
            .client
            .poll_device_receipt(&wire::http(), &mut challenge, None)
            .await
            .unwrap();
        let GrantPoll::Authorized(receipt) = result else {
            panic!("expected receipt");
        };
        assert_eq!(receipt.assurance(), GrantAssurance::TokenEndpointOnly);
        assert_eq!(receipt.flow(), GrantFlow::Device);
        assert!(receipt.identity().is_none());
        assert!(matches!(
            fixture
                .client
                .poll_device_receipt(&wire::http(), &mut challenge, None)
                .await,
            Err(OAuthError::Expired)
        ));
        assert_eq!(fixture.calls.lock().unwrap().len(), 1);
    }
}

#[tokio::test]
async fn device_id_only_constraint_is_distinct_from_type_and_missing_hint_fails_closed() {
    for (token, constraint, accepted) in [
        (
            access(json!({"principalId":"required"})),
            PrincipalConstraint::for_id("required").unwrap(),
            true,
        ),
        (
            access(json!({"principalId":"required"})),
            PrincipalConstraint::for_principal(Principal::new("Team", "required").unwrap()),
            false,
        ),
        (
            "opaque-access".into(),
            PrincipalConstraint::for_id("required").unwrap(),
            false,
        ),
    ] {
        let fixture =
            wire::spawn_fixture(axum::http::StatusCode::OK, json!({"access_token":token})).await;
        let mut challenge = challenge(&fixture.client);
        let result = fixture
            .client
            .poll_device_receipt(&wire::http(), &mut challenge, Some(&constraint))
            .await;
        if accepted {
            assert!(matches!(result, Ok(GrantPoll::Authorized(_))));
        } else {
            assert!(matches!(result, Err(OAuthError::PrincipalMismatch)));
        }
        assert_eq!(fixture.calls.lock().unwrap().len(), 1);
    }
}

#[test]
fn principal_hints_reject_ambiguous_aliases_and_never_claim_signature_verification() {
    for body in [
        json!({"principal_id":"one","principalId":"two"}),
        json!({"principal_type":"Team","principalType":"Team"}),
        json!({"principal_id":null}),
        json!({"principal_type":""}),
    ] {
        assert!(PrincipalHint::read(&access(body)).is_err());
    }
    assert!(PrincipalHint::read("opaque-token").unwrap().is_none());
    let hint = PrincipalHint::read(&access(json!({"principalId":"private-hint"})))
        .unwrap()
        .unwrap();
    assert_eq!(hint.kind(), None);
    assert_eq!(hint.id(), Some("private-hint"));
    assert!(!format!("{hint:?}").contains("private-hint"));
    assert!(PrincipalConstraint::for_id("").is_err());
}

#[test]
fn opaque_shapes_are_not_principal_hints_and_jwt_headers_remain_unambiguous() {
    for token in [
        "opaque.session.token".to_owned(),
        ".".repeat(32_768),
        format!(
            "{}.{}.signature",
            URL_SAFE_NO_PAD.encode(r#"{"kid":"opaque"}"#),
            URL_SAFE_NO_PAD.encode(r#"{"principal_type":"Team","principal_id":"team"}"#)
        ),
    ] {
        assert!(PrincipalHint::read(&token).unwrap().is_none());
    }
    assert!(PrincipalHint::read(&".".repeat(32_769)).is_err());
    for header in [r#"{"alg":""}"#, r#"{"alg":"RS256","alg":"ES256"}"#] {
        let token = format!("{}.e30.signature", URL_SAFE_NO_PAD.encode(header));
        assert!(PrincipalHint::read(&token).is_err());
    }
}

#[tokio::test]
async fn ambiguous_principals_fail_on_both_public_acceptance_paths() {
    for principal in [
        json!({"principal_id":"one","principalId":"two"}),
        json!({"principal_type":"Team","principalType":"Team"}),
        json!({"principal_id":null}),
        json!({"principal_type":""}),
    ] {
        for device in [false, true] {
            let fixture = wire::spawn_fixture(
                axum::http::StatusCode::OK,
                json!({"access_token":access(principal.clone())}),
            )
            .await;
            if device {
                let mut challenge = challenge(&fixture.client);
                assert!(matches!(
                    fixture
                        .client
                        .poll_device_receipt(&wire::http(), &mut challenge, None)
                        .await,
                    Err(OAuthError::InvalidResponse)
                ));
            } else {
                let auth = identity_support::authorization(&fixture.client);
                assert!(matches!(
                    fixture
                        .client
                        .complete_code(&wire::http(), &auth, &callback(&auth), None)
                        .await,
                    Err(OAuthError::InvalidResponse)
                ));
            }
            assert_eq!(fixture.calls.lock().unwrap().len(), 1);
        }
    }
}

#[tokio::test]
async fn ordinary_code_receipt_requires_real_jwks_verification_and_retains_client_context() {
    let (result, client, now, calls) = signed_code_receipt(300, std::time::Duration::ZERO).await;
    let receipt = result.unwrap();
    assert_eq!(receipt.client(), &client);
    assert_eq!(receipt.assurance(), GrantAssurance::SignedEndUser);
    assert_eq!(receipt.flow(), GrantFlow::Code);
    assert_eq!(
        receipt.identity().unwrap().subject(),
        "private-synthetic-subject"
    );
    assert!(receipt.principal_hint().is_none());
    let after = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs();
    assert!((now + 300..=after + 300).contains(&receipt.expires_at_unix_secs().unwrap()));
    assert_eq!(calls, 1);
}

#[tokio::test]
async fn receipt_expiring_during_successful_identity_verification_is_rejected() {
    let (result, _, _, calls) =
        signed_code_receipt(1, std::time::Duration::from_millis(1100)).await;
    assert!(matches!(result, Err(OAuthError::Expired)));
    assert_eq!(calls, 1);
}

async fn signed_code_receipt(
    expires: u64,
    jwks_delay: std::time::Duration,
) -> (Result<GrantReceipt, OAuthError>, OAuthClient, u64, usize) {
    use axum::{
        routing::{get, post},
        Json, Router,
    };
    use std::sync::{
        atomic::{AtomicUsize, Ordering},
        Arc,
    };
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let client = client().test_issuer(&format!("http://{}", listener.local_addr().unwrap()));
    let auth = identity_support::authorization(&client);
    let signer = identity_support::Signer::new();
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs();
    let body = json!({"access_token":"opaque.session.token","id_token":signer.sign(&identity_support::claims(&client,&auth,now)),"expires_in":expires});
    let keys: Value = serde_json::from_slice(&signer.jwks()).unwrap();
    let calls = Arc::new(AtomicUsize::new(0));
    let count = calls.clone();
    let app = Router::new()
        .route("/oauth2/token", post(move || async move { Json(body) }))
        .route(
            "/.well-known/jwks.json",
            get(move || {
                count.fetch_add(1, Ordering::SeqCst);
                let keys = keys.clone();
                async move {
                    tokio::time::sleep(jwks_delay).await;
                    Json(keys)
                }
            }),
        );
    let server = tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    let result = client
        .complete_code(&wire::http(), &auth, &callback(&auth), None)
        .await;
    server.abort();
    assert!(server.await.unwrap_err().is_cancelled());
    (result, client, now, calls.load(Ordering::SeqCst))
}
