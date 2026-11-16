//! Management identity and secret consent are independent of callback state and OAuth identity.
use super::{chatgpt_oauth::authorize, json_with_no_store};
use crate::{
    console::grok_oauth::{self, Binding},
    error::GatewayError,
    http::extractors::OptionalBearerToken,
    protocol::grok_oauth::{OAuthClient, Principal, PrincipalConstraint},
    state::AppState,
};
use axum::{
    extract::{ConnectInfo, Path, State},
    http::HeaderMap,
    response::Response,
    Json,
};
use serde::Deserialize;
use std::{net::SocketAddr, sync::Arc};

#[derive(Default, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AuthorizationFlow {
    #[default]
    Code,
    Loopback,
    Device,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PrincipalInput {
    kind: String,
    id: String,
}
impl PrincipalInput {
    fn principal(self) -> Result<Principal, GatewayError> {
        Principal::new(&self.kind, &self.id).map_err(|e| grok_oauth::login_error(e.into()))
    }
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Create {
    provider_id: String,
    expected_revision: String,
    credential_id: Option<String>,
    client_id: String,
    scopes: Vec<String>,
    redirect_uri: Option<String>,
    #[serde(default)]
    authorization_flow: AuthorizationFlow,
    principal_selection: Option<PrincipalInput>,
    required_principal: Option<PrincipalInput>,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Action {
    expected_revision: String,
    callback_url: Option<String>,
}

pub async fn create_grok_oauth(
    State(state): State<Arc<AppState>>,
    info: Option<ConnectInfo<SocketAddr>>,
    OptionalBearerToken(token): OptionalBearerToken,
    headers: HeaderMap,
    Json(body): Json<Create>,
) -> Result<Response, GatewayError> {
    let (owner, _) = authorize(&state, &headers, token.as_deref(), info.as_ref(), true)?;
    let mut binding = Binding::new(&owner, &body.provider_id, &body.expected_revision)
        .map_err(grok_oauth::login_error)?;
    if let Some(id) = body.credential_id {
        binding = binding
            .with_credential(&id)
            .map_err(grok_oauth::login_error)?;
    }
    grok_oauth::validate_target(&state, &binding)?;
    let scopes: Vec<_> = body.scopes.iter().map(String::as_str).collect();
    let mut client = OAuthClient::new(&body.client_id, &scopes)
        .map_err(|e| grok_oauth::login_error(e.into()))?;
    #[cfg(test)]
    {
        client = device_tests::client_for_test(client);
    }
    if let Some(principal) = body.principal_selection {
        client = client.with_principal(principal.principal()?);
    }
    let constraint = body
        .required_principal
        .map(|p| p.principal().map(PrincipalConstraint::for_principal))
        .transpose()?;
    let session = match body.authorization_flow {
        AuthorizationFlow::Loopback => state.lifecycle.grok_oauth.create_loopback(
            &state,
            binding,
            client,
            body.redirect_uri.as_deref().ok_or_else(|| {
                GatewayError::bad_request("redirectUri is required for loopback flow")
            })?,
            constraint,
            oauth_http()?,
        ),
        AuthorizationFlow::Code => state.lifecycle.grok_oauth.create(
            binding,
            client,
            body.redirect_uri.as_deref().ok_or_else(|| {
                GatewayError::bad_request("redirectUri is required for code flow")
            })?,
            constraint,
        ),
        AuthorizationFlow::Device => {
            if body.redirect_uri.is_some() || client.principal().is_some() {
                return Err(GatewayError::bad_request(
                    "Device flow does not accept redirectUri or consent principal selection.",
                ));
            }
            let http = oauth_http()?;
            state
                .lifecycle
                .grok_oauth
                .create_device(binding, client, constraint, &http)
                .await
        }
    }
    .map_err(grok_oauth::login_error)?;
    Ok(json_with_no_store(
        serde_json::json!({"session":session.view()}),
    ))
}
pub async fn get_grok_oauth(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    info: Option<ConnectInfo<SocketAddr>>,
    OptionalBearerToken(token): OptionalBearerToken,
    headers: HeaderMap,
) -> Result<Response, GatewayError> {
    let (owner, _) = authorize(&state, &headers, token.as_deref(), info.as_ref(), false)?;
    let session = state
        .lifecycle
        .grok_oauth
        .get_owned(&id, &owner)
        .map_err(grok_oauth::login_error)?;
    Ok(json_with_no_store(
        serde_json::json!({"session":session.view()}),
    ))
}
pub async fn act_grok_oauth(
    State(state): State<Arc<AppState>>,
    Path((id, action)): Path<(String, String)>,
    info: Option<ConnectInfo<SocketAddr>>,
    OptionalBearerToken(token): OptionalBearerToken,
    headers: HeaderMap,
    Json(body): Json<Action>,
) -> Result<Response, GatewayError> {
    let (owner, _) = authorize(&state, &headers, token.as_deref(), info.as_ref(), true)?;
    let session = state
        .lifecycle
        .grok_oauth
        .get_owned(&id, &owner)
        .map_err(grok_oauth::login_error)?;
    let binding = session.binding();
    if body.expected_revision != binding.revision() {
        return Err(
            GatewayError::conflict("Grok login revision does not match the session")
                .with_code("console_revision_conflict"),
        );
    }
    match action.as_str() {
        "cancel" if body.callback_url.is_none() => session.cancel(),
        "complete" => {
            grok_oauth::validate_target(&state, binding)?;
            let callback = body
                .callback_url
                .ok_or_else(|| GatewayError::bad_request("callbackUrl is required"))?;
            // The protocol transport itself fixes issuer, redirects, body and deadline budgets.
            let http = oauth_http()?;
            session
                .complete(&http, &callback)
                .await
                .map_err(grok_oauth::login_error)?;
        }
        "poll" if body.callback_url.is_none() => {
            grok_oauth::validate_target(&state, binding)?;
            session
                .poll_device(&oauth_http()?)
                .await
                .map_err(grok_oauth::login_error)?;
        }
        "import" if body.callback_url.is_none() => {
            let view = state
                .lifecycle
                .grok_oauth
                .start_import(state.clone(), &id, binding)?;
            return Ok(json_with_no_store(serde_json::json!({"session":view})));
        }
        _ => return Err(GatewayError::bad_request("Unsupported Grok login action")),
    }
    Ok(json_with_no_store(
        serde_json::json!({"session":session.view()}),
    ))
}

fn oauth_http() -> Result<rquest::Client, GatewayError> {
    rquest::Client::builder()
        .build()
        .map_err(|_| GatewayError::service_unavailable("Grok OAuth transport unavailable"))
}

#[cfg(test)]
#[path = "grok_oauth/device_tests.rs"]
mod device_tests;
#[cfg(test)]
#[path = "grok_oauth/loopback_tests.rs"]
mod loopback_tests;
#[cfg(test)]
mod tests;
