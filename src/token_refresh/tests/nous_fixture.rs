//! Synthetic signed-looking JWTs are hints only; these fixtures never use an inference transport.
use super::*;
use crate::{
    console::{refresh_grants::*, RouteConfigRedisRevision},
    protocol::nous_portal::*,
};
use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
use serde_json::{json, Value};

pub(super) const EXPIRY: &str = "2100-01-01T00:00:00Z";
pub(super) fn access(subject: &str, expiry: u64) -> String {
    format!(
        "{}.{}.{}",
        URL_SAFE_NO_PAD.encode(br#"{"alg":"RS256"}"#),
        URL_SAFE_NO_PAD
            .encode(json!({"sub":subject,"exp":expiry,"scope":INFERENCE_SCOPE}).to_string()),
        URL_SAFE_NO_PAD.encode(b"synthetic-signature")
    )
}
pub(super) fn receipt() -> ReceivedTokens {
    ReceivedTokens::from_json(
        json!({"access_token":access("synthetic-subject",4102444900),
        "refresh_token":"nous-new-refresh","token_type":"Bearer"})
        .to_string()
        .as_bytes(),
    )
    .unwrap()
}
pub(super) fn old_tokens() -> AuthorizedTokens {
    ReceivedTokens::from_json(json!({"access_token":access("synthetic-subject",4102444800),
        "refresh_token":"nous-old-refresh","inference_base_url":InferenceTarget::Welcome.base_url()})
        .to_string().as_bytes()).unwrap().admit(&OAuthClient::new("operator-client").unwrap(),None).unwrap()
}
pub(super) fn context() -> Value {
    serde_json::to_value(old_tokens().context).unwrap()
}
pub(super) async fn fixture() -> LocalFixture {
    let f = LocalFixture::new(TOKEN_ENDPOINT).await;
    let snapshot = f.runtime.route_config().snapshot();
    let mut doc = snapshot.document().clone();
    let p = &mut doc.providers[0];
    p.base_url = InferenceTarget::Welcome.base_url().into();
    p.adapter = Some("openai_compatible".into());
    p.explicit_credentials_only = true;
    p.auth_mode = Some("nous_portal_oauth".into());
    let old = old_tokens();
    let c = &mut p.credentials[0];
    c.headers.clear();
    c.api_key = Some(old.access_token);
    c.refresh_token = Some(old.refresh_token);
    c.nous_portal = Some(old.context);
    c.refresh_endpoint = None;
    c.refresh_client_id = None;
    c.expires_at = Some(EXPIRY.into());
    f.runtime
        .commit_document(snapshot.revision().id(), doc, None)
        .await
        .unwrap();
    f
}
pub(super) fn request(f: &LocalFixture) -> GrantRequest {
    let snapshot = f.runtime.route_config().snapshot();
    let revision =
        RouteConfigRedisRevision::new(snapshot.revision().clone(), snapshot.document().clone())
            .unwrap();
    GrantRequest::from_revision(&revision, "oauth", "account").unwrap()
}
pub(super) fn persistence(f: &LocalFixture) -> RouteConfigPersistence {
    RouteConfigPersistence::new(&f.config.state_dir, &f.config.routes_file).unwrap()
}
pub(super) fn load(f: &LocalFixture, request: &GrantRequest) -> GrantRecord {
    let path = persistence(f)
        .state_root()
        .join("refresh-grants")
        .join(request.family_id())
        .join(format!("{}.json", request.material_id()));
    GrantRecord::from_json(&std::fs::read_to_string(path).unwrap()).unwrap()
}
pub(super) fn result(request: &GrantRequest, received: &ReceivedTokens) -> GrantResult {
    let token = received
        .interpret_successor(
            &OAuthClient::new("operator-client").unwrap(),
            &request.material.access,
            &request.material.refresh,
            request.material.nous_portal.as_ref(),
        )
        .unwrap();
    request
        .result_with_nous(
            &token.access_token,
            &token.refresh_token,
            token.expires_in,
            "2100-01-01T00:01:40Z",
            token.context,
        )
        .unwrap()
}
