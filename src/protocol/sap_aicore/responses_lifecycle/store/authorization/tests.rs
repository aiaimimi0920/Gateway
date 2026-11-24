use super::*;
use crate::{
    auth::{adapter::AuthRequest, authenticate_request},
    protocol::sap_aicore::{
        test_support::{BASE, KEY},
        wire_tests::{fixture::Fixture, peer::Peer},
    },
};
use serde_json::json;

#[tokio::test]
async fn sap_aicore_response_receipts_local_authorization_never_charges_or_substitutes() {
    Box::pin(exercise_local()).await;
}

async fn exercise_local() {
    let peer = Peer::new("responses", false, false, 200).await;
    let fixture = Box::pin(Fixture::configured(
        &peer,
        true,
        BASE,
        KEY,
        json!({"sap_native_responses":true,"sap_stored_responses":true,"sap_api_version":"2024-10-21"}),
    ))
    .await;
    let state = &fixture.state;
    let session = authenticate_request(
        state,
        &AuthRequest {
            authorization: Some(format!("Bearer {}", fixture.token)),
            api_key: None,
            path: "/v1/responses".into(),
            method: "POST".into(),
        },
        None,
    )
    .await
    .unwrap();
    let key_id = session.access_key_id.as_deref().unwrap();
    let secret = key(state).unwrap();
    let pin = RoutePin {
        public_model: "public-sap_aicore".into(),
        source_key_ref: key_id.into(),
        platform_access_ref: None,
    };
    let mut route = Receipt {
        id: String::new(),
        owner_tag: String::new(),
        binding_tag: String::new(),
        provider_ref: "sap_aicore-public".into(),
        credential_ref: "exact".into(),
        upstream_id: None,
        phase: "submitting".into(),
        revision: 0,
        input_tokens: None,
        output_tokens: None,
        settled: 0,
    };
    let (payload, model) = read_target(
        state,
        &session,
        &pin,
        &route.provider_ref,
        &route.credential_ref,
        None,
    )
    .await
    .unwrap()
    .unwrap();
    assert_eq!(model, "deployment1");
    route.binding_tag = pin
        .capture_binding(
            &secret,
            &route.provider_ref,
            &route.credential_ref,
            &payload,
        )
        .unwrap();
    let local = state.local_runtime.as_ref().unwrap();
    let store = Store::Local(&local.pool);
    let owner = owner_tag(&secret, &session).unwrap();
    let first = store
        .begin(
            &owner,
            &route.binding_tag,
            &route.provider_ref,
            &route.credential_ref,
            &pin,
        )
        .await
        .unwrap();
    let second = store
        .begin(
            &owner,
            &route.binding_tag,
            &route.provider_ref,
            &route.credential_ref,
            &pin,
        )
        .await
        .unwrap();
    assert!(store.reserve_balance(&first, key_id, 1).await.unwrap());
    assert!(store.reserve_balance(&second, key_id, 1).await.unwrap());
    assert_eq!(fixture.remaining().await, 0);
    assert!(read_authorized(state, &session, &first.id, None)
        .await
        .unwrap()
        .is_some());
    assert_eq!(fixture.remaining().await, 0);
    let mut other = session.clone();
    other.project_id.push_str("-wrong");
    assert!(read_authorized(state, &other, &first.id, None)
        .await
        .unwrap()
        .is_none());
    route.credential_ref = "disabled-or-missing".into();
    assert!(read_target(
        state,
        &session,
        &pin,
        &route.provider_ref,
        &route.credential_ref,
        None
    )
    .await
    .unwrap()
    .is_none());
    assert!(
        read_authorized(state, &session, &first.id, Some("missing-group"))
            .await
            .unwrap()
            .is_none()
    );
    local.set_access_key_enabled(key_id, false).await.unwrap();
    assert!(read_authorized(state, &session, &first.id, None)
        .await
        .is_err());
    assert!(peer.oauth.lock().unwrap().is_empty());
    assert!(peer.calls.lock().unwrap().is_empty());
    fixture.finish().await;
    peer.stop().await;
}
