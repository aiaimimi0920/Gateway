//! Management-authenticated OAuth orchestration; callback state never grants console access.
use super::{console_request_context, json_with_no_store, required_console_management_token};
use crate::{
    console::chatgpt_oauth, error::GatewayError, http::extractors::OptionalBearerToken,
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

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Create {
    provider_id: String,
    group: String,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Action {
    #[serde(default)]
    callback_url: String,
}

fn authorize(
    state: &AppState,
    headers: &HeaderMap,
    bearer: Option<&str>,
    info: Option<&ConnectInfo<SocketAddr>>,
    secret: bool,
) -> Result<(String, bool), GatewayError> {
    let context = console_request_context(headers, info);
    let actor = state.console_auth.authenticate_management_token(
        &context,
        required_console_management_token(bearer, headers)?,
    )?;
    if secret {
        state.console_auth.verify_secret_grant(
            &context,
            &actor,
            headers
                .get("x-secret-grant")
                .and_then(|v| v.to_str().ok())
                .unwrap_or_default(),
        )?;
    }
    let owner = serde_json::to_string(&(
        actor.token_fingerprint(),
        session_host(headers),
        context.client_ip(),
    ))
    .expect("owner serialization");
    Ok((owner, context.is_loopback()))
}

// Browser GETs can omit both Origin and Referer (the console uses no-referrer).
// Bind sessions to the authenticated operator, client IP and destination host.
// Sensitive mutations still require the existing origin-bound secret grant.
fn session_host(headers: &HeaderMap) -> &str {
    headers
        .get("host")
        .and_then(|value| value.to_str().ok())
        .unwrap_or("local")
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn browser_get_and_post_share_session_host_without_referrer() {
        let mut post = HeaderMap::new();
        post.insert("host", "127.0.0.1:56627".parse().unwrap());
        post.insert("origin", "http://127.0.0.1:56627".parse().unwrap());
        let mut get = HeaderMap::new();
        get.insert("host", "127.0.0.1:56627".parse().unwrap());
        assert_eq!(session_host(&post), session_host(&get));
        get.insert("host", "example.com".parse().unwrap());
        assert_ne!(session_host(&post), session_host(&get));
    }
}

pub async fn create_chatgpt_oauth(
    State(state): State<Arc<AppState>>,
    info: Option<ConnectInfo<SocketAddr>>,
    OptionalBearerToken(token): OptionalBearerToken,
    headers: HeaderMap,
    Json(body): Json<Create>,
) -> Result<Response, GatewayError> {
    let (owner, local) = authorize(&state, &headers, token.as_deref(), info.as_ref(), true)?;
    let session = chatgpt_oauth::create(&state, owner, body.provider_id, body.group, local).await?;
    Ok(json_with_no_store(
        serde_json::json!({"session":session.view()}),
    ))
}

pub async fn get_chatgpt_oauth(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    info: Option<ConnectInfo<SocketAddr>>,
    OptionalBearerToken(token): OptionalBearerToken,
    headers: HeaderMap,
) -> Result<Response, GatewayError> {
    let (owner, _) = authorize(&state, &headers, token.as_deref(), info.as_ref(), false)?;
    Ok(json_with_no_store(
        serde_json::json!({"session":chatgpt_oauth::get(&id,&owner)?.view()}),
    ))
}

pub async fn act_chatgpt_oauth(
    State(state): State<Arc<AppState>>,
    Path((id, action)): Path<(String, String)>,
    info: Option<ConnectInfo<SocketAddr>>,
    OptionalBearerToken(token): OptionalBearerToken,
    headers: HeaderMap,
    Json(body): Json<Action>,
) -> Result<Response, GatewayError> {
    let (owner, local) = authorize(
        &state,
        &headers,
        token.as_deref(),
        info.as_ref(),
        action == "import",
    )?;
    let session = chatgpt_oauth::get(&id, &owner)?;
    match action.as_str() {
        "cancel" => session.cancel(),
        "complete" => {
            let (code, nonce) = chatgpt_oauth::parse_callback(&body.callback_url)?;
            session
                .complete(state.upstream_client.client(), &code, &nonce)
                .await?;
        }
        "import" => {
            session.import(&state).await?;
        }
        "open-browser" if local => session.open_browser().await?,
        _ => {
            return Err(GatewayError::bad_request(
                "Unsupported ChatGPT login action",
            ))
        }
    }
    Ok(json_with_no_store(
        serde_json::json!({"session":session.view()}),
    ))
}
