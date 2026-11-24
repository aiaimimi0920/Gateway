//! Bounded HTTP JSON transport for AWS IAM Identity Center OIDC and QuickWork user-info.
//! Enforces no-redirect, no-retry, 15s request budget, and 64 KiB response size cap.

use super::{
    atom, context::AuthorizationContext, valid_token, OAuthError, OIDC_CLIENT_NAME, OIDC_SCOPE,
};
use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine as _};
use futures::StreamExt;
use serde::Deserialize;
use serde_json::{json, Value};
use std::time::Duration;

const HTTP_BUDGET: Duration = Duration::from_secs(15);
const MAX_BODY_BYTES: usize = 64 * 1024;

#[derive(Clone)]
pub(crate) struct RegisteredClient {
    pub client_id: String,
    pub client_secret: String,
    pub client_secret_expires_at: Option<i64>,
}

pub(crate) struct ExchangedTokens {
    pub access_token: String,
    pub refresh_token: String,
    pub id_token: Option<String>,
    pub expires_in_secs: u64,
}

pub(crate) struct RefreshedTokens {
    pub bearer_token: String,
    pub refresh_token: Option<String>,
    pub expires_in_secs: u64,
}

#[derive(Default)]
pub(crate) struct UserProfile {
    pub actor_arn: Option<String>,
    pub aws_account_id: Option<String>,
    pub email: Option<String>,
    pub display_name: Option<String>,
    pub account_name: Option<String>,
}

/// One-shot refresh grant pinned by the durable `token_refresh` scheduler.
pub(crate) struct RefreshGrant {
    pub(crate) token_endpoint: String,
    pub(crate) client_id: String,
    pub(crate) client_secret: String,
    pub(crate) refresh_token: String,
}

#[derive(Deserialize)]
struct RegisterReply {
    #[serde(alias = "clientId", alias = "client_id")]
    client_id: String,
    #[serde(alias = "clientSecret", alias = "client_secret")]
    client_secret: String,
    #[serde(
        default,
        alias = "clientSecretExpiresAt",
        alias = "client_secret_expires_at"
    )]
    client_secret_expires_at: Option<i64>,
}

#[derive(Deserialize)]
struct TokenReply {
    #[serde(alias = "accessToken", alias = "access_token")]
    access_token: String,
    #[serde(default, alias = "refreshToken", alias = "refresh_token")]
    refresh_token: Option<String>,
    #[serde(default, alias = "idToken", alias = "id_token")]
    id_token: Option<String>,
    #[serde(default, alias = "expiresIn", alias = "expires_in")]
    expires_in: Option<i64>,
}

#[derive(Deserialize)]
struct OidcErrorReply {
    #[serde(default)]
    error: String,
}

pub(crate) fn build_http_client() -> Result<rquest::Client, OAuthError> {
    rquest::Client::builder()
        .redirect(rquest::redirect::Policy::none())
        .retry(rquest::retry::Policy::never())
        .timeout(HTTP_BUDGET)
        .build()
        .map_err(|_| OAuthError::Transport)
}

pub(crate) fn handshake_token(id_token: Option<&str>, access_token: &str) -> String {
    id_token
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .unwrap_or(access_token)
        .to_owned()
}

pub(crate) async fn register_oidc_client(
    http: &rquest::Client,
    idc_region: &str,
    issuer_url: &str,
    redirect_uri: &str,
) -> Result<RegisteredClient, OAuthError> {
    let origin = AuthorizationContext::oidc_origin_for_region(idc_region)?;
    let url = format!("{origin}/client/register");
    let body = json!({
        "clientName": OIDC_CLIENT_NAME,
        "clientType": "public",
        "scopes": [OIDC_SCOPE],
        "grantTypes": ["authorization_code", "refresh_token"],
        "redirectUris": [redirect_uri],
        "issuerUrl": issuer_url,
    });
    let (status, bytes) = post_json(http, &url, &body, None).await?;
    if !(200..300).contains(&status) {
        return Err(classify_oidc_failure(status, &bytes));
    }
    let reply: RegisterReply =
        serde_json::from_slice(&bytes).map_err(|_| OAuthError::InvalidResponse)?;
    if !atom(&reply.client_id, 4096) || !atom(&reply.client_secret, 16_384) {
        return Err(OAuthError::InvalidResponse);
    }
    Ok(RegisteredClient {
        client_id: reply.client_id,
        client_secret: reply.client_secret,
        client_secret_expires_at: reply.client_secret_expires_at.filter(|v| *v > 0),
    })
}

pub(crate) async fn exchange_code(
    http: &rquest::Client,
    idc_region: &str,
    client: &RegisteredClient,
    code: &str,
    redirect_uri: &str,
    code_verifier: &str,
) -> Result<ExchangedTokens, OAuthError> {
    let origin = AuthorizationContext::oidc_origin_for_region(idc_region)?;
    let url = format!("{origin}/token");
    let body = json!({
        "clientId": client.client_id,
        "clientSecret": client.client_secret,
        "grantType": "authorization_code",
        "code": code,
        "redirectUri": redirect_uri,
        "codeVerifier": code_verifier,
    });
    let (status, bytes) = post_json(http, &url, &body, None).await?;
    if !(200..300).contains(&status) {
        return Err(classify_oidc_failure(status, &bytes));
    }
    let reply: TokenReply =
        serde_json::from_slice(&bytes).map_err(|_| OAuthError::InvalidResponse)?;
    let refresh_token = reply.refresh_token.ok_or(OAuthError::InvalidResponse)?;
    if !valid_token(&reply.access_token) || !valid_token(&refresh_token) {
        return Err(OAuthError::InvalidResponse);
    }
    let id_token = reply
        .id_token
        .map(|s| s.trim().to_owned())
        .filter(|s| !s.is_empty());
    if id_token.as_deref().is_some_and(|s| !valid_token(s)) {
        return Err(OAuthError::InvalidResponse);
    }
    let expires_in_secs = reply.expires_in.unwrap_or(3600).clamp(60, 86_400) as u64;
    Ok(ExchangedTokens {
        access_token: reply.access_token,
        refresh_token,
        id_token,
        expires_in_secs,
    })
}

impl RefreshGrant {
    pub(crate) async fn execute(
        self,
        http: &rquest::Client,
    ) -> Result<RefreshedTokens, OAuthError> {
        let body = json!({
            "clientId": self.client_id,
            "clientSecret": self.client_secret,
            "grantType": "refresh_token",
            "refreshToken": self.refresh_token,
        });
        let (status, bytes) = post_json(http, &self.token_endpoint, &body, None).await?;
        if !(200..300).contains(&status) {
            return Err(classify_oidc_failure(status, &bytes));
        }
        let reply: TokenReply =
            serde_json::from_slice(&bytes).map_err(|_| OAuthError::InvalidResponse)?;
        if !valid_token(&reply.access_token) {
            return Err(OAuthError::InvalidResponse);
        }
        let id_token = reply
            .id_token
            .map(|s| s.trim().to_owned())
            .filter(|s| !s.is_empty());
        if id_token.as_deref().is_some_and(|s| !valid_token(s)) {
            return Err(OAuthError::InvalidResponse);
        }
        let rotated = reply
            .refresh_token
            .map(|s| s.trim().to_owned())
            .filter(|s| !s.is_empty());
        if rotated.as_deref().is_some_and(|s| !valid_token(s)) {
            return Err(OAuthError::InvalidResponse);
        }
        let bearer_token = handshake_token(id_token.as_deref(), &reply.access_token);
        let expires_in_secs = reply.expires_in.unwrap_or(3600).clamp(60, 86_400) as u64;
        Ok(RefreshedTokens {
            bearer_token,
            refresh_token: rotated,
            expires_in_secs,
        })
    }
}

pub(crate) async fn fetch_user_info(
    http: &rquest::Client,
    data_plane_base: &str,
    access_token: &str,
    id_token: Option<&str>,
) -> UserProfile {
    let url = format!(
        "{}/integration/quick-work/user-info",
        data_plane_base.trim_end_matches('/')
    );
    let mut profile = UserProfile::default();
    if let Ok((status, bytes)) = post_json(http, &url, &json!({}), Some(access_token)).await {
        if (200..300).contains(&status) {
            if let Ok(root) = serde_json::from_slice::<Value>(&bytes) {
                let node = root.get("userInfo").unwrap_or(&root);
                profile.actor_arn = pick_str(node, &["userArn", "user_arn", "arn"]);
                profile.aws_account_id = pick_str(node, &["awsAccountId", "aws_account_id"]);
                profile.email = pick_str(node, &["email", "emailAddress", "email_address"]);
                profile.display_name = pick_str(
                    node,
                    &["displayName", "display_name", "userName", "user_name"],
                );
                profile.account_name = pick_str(node, &["accountName", "account_name"]);
            }
        }
    }
    if profile.actor_arn.is_none() {
        profile.actor_arn = id_token.and_then(jwt_sub_arn);
    }
    profile
}

async fn post_json(
    http: &rquest::Client,
    url: &str,
    body: &Value,
    bearer: Option<&str>,
) -> Result<(u16, Vec<u8>), OAuthError> {
    let mut req = http
        .post(url)
        .default_headers(false)
        .redirect(rquest::redirect::Policy::none())
        .timeout(HTTP_BUDGET)
        .header("content-type", "application/json")
        .header("accept", "application/json")
        .json(body);
    if let Some(token) = bearer {
        req = req.bearer_auth(token);
    }
    let resp = tokio::time::timeout(HTTP_BUDGET, req.send())
        .await
        .map_err(|_| OAuthError::Timeout)?
        .map_err(|e| {
            if e.is_timeout() {
                OAuthError::Timeout
            } else {
                OAuthError::Transport
            }
        })?;
    if resp.status().is_redirection() {
        return Err(OAuthError::RedirectRejected);
    }
    let status = resp.status().as_u16();
    let mut stream = resp.bytes_stream();
    let mut buf = Vec::new();
    while let Some(chunk) = tokio::time::timeout(HTTP_BUDGET, stream.next())
        .await
        .map_err(|_| OAuthError::Timeout)?
    {
        let chunk = chunk.map_err(|_| OAuthError::Transport)?;
        if buf.len() + chunk.len() > MAX_BODY_BYTES {
            return Err(OAuthError::ResponseTooLarge);
        }
        buf.extend_from_slice(&chunk);
    }
    Ok((status, buf))
}

fn classify_oidc_failure(status: u16, bytes: &[u8]) -> OAuthError {
    let code = serde_json::from_slice::<OidcErrorReply>(bytes)
        .map(|r| r.error)
        .unwrap_or_default();
    match code.as_str() {
        "access_denied" => OAuthError::AccessDenied,
        "invalid_grant" | "invalid_client" | "unauthorized_client" => {
            OAuthError::ReauthorizationRequired
        }
        _ if matches!(status, 401 | 403) => OAuthError::ReauthorizationRequired,
        _ => OAuthError::Transport,
    }
}

fn pick_str(node: &Value, keys: &[&str]) -> Option<String> {
    keys.iter().find_map(|key| {
        node.get(*key)
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|s| !s.is_empty() && s.len() <= 512 && !s.chars().any(char::is_control))
            .map(str::to_owned)
    })
}

fn jwt_sub_arn(jwt: &str) -> Option<String> {
    let payload_b64 = jwt.split('.').nth(1)?;
    let bytes = URL_SAFE_NO_PAD.decode(payload_b64).ok()?;
    let claims: Value = serde_json::from_slice(&bytes).ok()?;
    claims
        .get("sub")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|s| s.starts_with("arn:") && s.len() <= 512 && !s.chars().any(char::is_control))
        .map(str::to_owned)
}
