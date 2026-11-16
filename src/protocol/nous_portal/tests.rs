//! Pure eligibility tests do not equate decoded JWT hints with issuer signature verification.
use super::*;
use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
use serde_json::{json, Value};
mod wire;

fn client() -> OAuthClient {
    OAuthClient::new("permitted-gateway-client").unwrap()
}
fn jwt(claims: Value) -> String {
    format!(
        "{}.{}.{}",
        URL_SAFE_NO_PAD.encode(br#"{"alg":"RS256"}"#),
        URL_SAFE_NO_PAD.encode(claims.to_string()),
        URL_SAFE_NO_PAD.encode(b"synthetic-signature")
    )
}
fn body(subject: Option<&str>) -> Value {
    let mut claims = json!({"exp":unix_now()+3600,"scope":INFERENCE_SCOPE});
    if let Some(subject) = subject {
        claims["sub"] = json!(subject);
    }
    json!({"access_token":jwt(claims),"refresh_token":"received-successor","expires_in":1800,
        "token_type":"Bearer","inference_base_url":InferenceTarget::Subscription.base_url()})
}
fn receipt(value: Value) -> ReceivedTokens {
    ReceivedTokens::from_json(value.to_string().as_bytes()).unwrap()
}

#[test]
fn nous_explicit_client_and_exact_inference_targets_have_no_borrowed_default() {
    for value in ["", "bad client", "bad\nclient"] {
        assert!(OAuthClient::new(value).is_err());
    }
    assert_eq!(client().client_id(), "permitted-gateway-client");
    for target in [InferenceTarget::Subscription, InferenceTarget::Welcome] {
        assert_eq!(
            InferenceTarget::from_base_url(target.base_url()).unwrap(),
            target
        );
    }
    for target in [
        "https://inference-api.nousresearch.com.attacker.test/v1",
        "http://inference-api.nousresearch.com/v1",
        "https://user@inference-api.nousresearch.com/v1",
        "https://inference-api.nousresearch.com/v1?x=1",
        "https://inference-api.nousresearch.com/v2",
        "https://inference-api.nousresearch.com:443/v1",
    ] {
        assert_eq!(
            InferenceTarget::from_base_url(target),
            Err(OAuthError::UnsafeInferenceTarget)
        );
    }
}

#[test]
fn nous_received_pair_survives_every_post_exchange_admission_failure() {
    let mut cases = Vec::new();
    for (field, replacement) in [
        ("access_token", json!("invalid-jwt")),
        ("expires_in", json!("bad")),
        ("token_type", json!("MAC")),
        ("inference_base_url", json!("https://attacker.test/v1")),
        ("scope", json!({"bad":"scope"})),
    ] {
        let mut value = body(Some("subject"));
        value[field] = replacement;
        cases.push(value);
    }
    let mut missing_access = body(None);
    missing_access
        .as_object_mut()
        .unwrap()
        .remove("access_token");
    cases.push(missing_access);
    for value in cases {
        let received = receipt(value);
        assert_eq!(received.refresh_token().unwrap(), "received-successor");
        assert!(received.admit(&client(), None).is_err());
        let restored: ReceivedTokens =
            serde_json::from_str(&serde_json::to_string(&received).unwrap()).unwrap();
        assert_eq!(received, restored);
        assert!(!format!("{received:?}").contains("received-successor"));
    }
}

#[test]
fn nous_expiry_is_bounded_by_both_real_expiries_and_never_rebased_on_replay() {
    let received = receipt(body(Some("subject")));
    let token = received.admit(&client(), None).unwrap();
    assert_eq!(token.expires_in, 1800);
    assert_eq!(token.expires_at_unix, received.received_at_unix() + 1800);
    assert_eq!(token.context.subject_hint(), Some("subject"));
    token.context.validate_access(&token.access_token).unwrap();
    assert!(token
        .context
        .validate_access(&jwt(json!({"exp":unix_now()+3600,"sub":"other"})))
        .is_err());
    let restored: ReceivedTokens =
        serde_json::from_str(&serde_json::to_string(&received).unwrap()).unwrap();
    assert_eq!(
        restored.admit(&client(), None).unwrap().expires_at_unix,
        token.expires_at_unix
    );
    for claims in [
        json!({"scope":INFERENCE_SCOPE}),
        json!({"scope":INFERENCE_SCOPE,"exp":0}),
        json!({"scope":INFERENCE_SCOPE,"exp":"bad"}),
        json!({"scope":INFERENCE_SCOPE,"exp":u64::MAX}),
        json!({"scope":INFERENCE_SCOPE,"exp":unix_now()+60,"nbf":unix_now()+100}),
    ] {
        let value = json!({"access_token":jwt(claims),"refresh_token":"new-refresh"});
        assert!(receipt(value).admit(&client(), None).is_err());
    }
}

#[test]
fn nous_subject_is_only_a_hint_and_unknown_subjects_are_not_account_equality() {
    let old = receipt(body(Some("original")))
        .admit(&client(), None)
        .unwrap();
    assert_eq!(
        receipt(body(Some("other")))
            .admit(&client(), Some(&old.context))
            .unwrap_err(),
        OAuthError::SubjectHintMismatch
    );
    assert_eq!(
        receipt(body(None))
            .admit(&client(), Some(&old.context))
            .unwrap_err(),
        OAuthError::SubjectHintMismatch
    );
    let unknown = receipt(body(None)).admit(&client(), None).unwrap();
    let other_unknown = receipt(body(None))
        .admit(&client(), Some(&unknown.context))
        .unwrap();
    assert!(!unknown
        .context
        .same_known_subject_hint(&other_unknown.context));
    assert!(!format!("{:?}", old.context).contains("original"));
    let different_client = OAuthClient::new("other-client").unwrap();
    assert!(receipt(body(Some("original")))
        .admit(&different_client, Some(&old.context))
        .is_err());
}

#[test]
fn nous_refresh_inherits_the_exact_previous_target_or_keeps_an_unresolved_receipt() {
    let mut old_body = body(Some("subject"));
    old_body["inference_base_url"] = json!(InferenceTarget::Welcome.base_url());
    old_body["refresh_token"] = json!("old-rt");
    let old = receipt(old_body).admit(&client(), None).unwrap();
    let mut next = body(Some("subject"));
    next.as_object_mut().unwrap().remove("inference_base_url");
    let next = receipt(next);
    let admitted = next
        .interpret_successor(&client(), &old.access_token, "old-rt", Some(&old.context))
        .unwrap();
    assert_eq!(
        admitted.context.inference_target(),
        InferenceTarget::Welcome
    );
    assert_eq!(
        next.interpret_successor(&client(), &old.access_token, "old-rt", None)
            .unwrap_err(),
        OAuthError::UnsafeInferenceTarget
    );
    assert_eq!(next.refresh_token().unwrap(), "received-successor");
    assert!(next
        .interpret_successor(
            &client(),
            "mismatched-old-access",
            "old-rt",
            Some(&old.context)
        )
        .is_err());
}

#[test]
fn nous_scope_can_be_explicit_response_or_jwt_but_is_never_fabricated() {
    for scopes in [json!(INFERENCE_SCOPE), json!(["other", INFERENCE_SCOPE])] {
        let value = json!({"access_token":jwt(json!({"exp":unix_now()+120,"scp":scopes})), "refresh_token":"next"});
        receipt(value).admit(&client(), None).unwrap();
    }
    let mut value = body(None);
    value["access_token"] = json!(jwt(json!({"exp":unix_now()+120})));
    assert_eq!(
        receipt(value.clone()).admit(&client(), None).unwrap_err(),
        OAuthError::ScopeMismatch
    );
    value["scope"] = json!(INFERENCE_SCOPE);
    receipt(value).admit(&client(), None).unwrap();
}

#[test]
fn nous_persisted_context_cannot_extend_its_jwt_expiry() {
    let mut value = body(Some("subject"));
    value["access_token"] = json!(jwt(
        json!({"scope":INFERENCE_SCOPE,"sub":"subject","exp":unix_now()+60})
    ));
    let token = receipt(value).admit(&client(), None).unwrap();
    let mut context = serde_json::to_value(&token.context).unwrap();
    context["expires_at_unix"] = json!(unix_now() + 7200);
    let context: AuthorizationContext = serde_json::from_value(context).unwrap();
    assert_eq!(
        context.validate_access(&token.access_token),
        Err(OAuthError::InvalidAccessJwt)
    );
}

#[test]
fn nous_catalog_preserves_slashes_and_distinguishes_successful_empty_from_failure() {
    let catalog =
        ModelCatalog::parse(json!({"data":[{"id":"org/model"},{"id":"nous/welcome"}]})).unwrap();
    assert_eq!(catalog.models[0].id, "org/model");
    assert!(ModelCatalog::parse(json!({"data":[]}))
        .unwrap()
        .models
        .is_empty());
    for value in [
        json!({"error":"private-detail"}),
        json!({}),
        json!({"data":[{"id":""}]}),
        json!({"data":[{"id":"same"},{"id":"same"}]}),
        json!({"data":vec![json!({"id":"a"});2049]}),
    ] {
        assert_eq!(
            ModelCatalog::parse(value).unwrap_err(),
            OAuthError::InvalidResponse
        );
    }
}

#[test]
fn nous_receipt_rejects_ambiguous_or_unbounded_secret_material() {
    for value in [
        br#"{"refresh_token":"a","refresh_token":"b"}"#.to_vec(),
        br#"{"refresh_token":""}"#.to_vec(),
        br#"[]"#.to_vec(),
        vec![b' '; 65_537],
    ] {
        assert!(ReceivedTokens::from_json(&value).is_err());
    }
    let mut value = body(None);
    value["refresh_token"] = json!("a\r\nb");
    assert!(ReceivedTokens::from_json(value.to_string().as_bytes()).is_err());
    let received = receipt(body(None));
    assert_eq!(
        received.validate_successor("received-successor"),
        Err(OAuthError::MissingSuccessor)
    );
}
