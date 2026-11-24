//! Two-step Amazon Quick social PKCE login supporting both automatic loopback 302->200
//! redirection and manual callback URL paste completion.

use super::{
    atom,
    context::{commercial_region, resolve_data_plane, valid_https_url, AuthorizationContext},
    transport::{
        exchange_code, fetch_user_info, handshake_token, register_oidc_client, RegisteredClient,
    },
    OAuthError, APP_VERSION, OIDC_SCOPE, PORTAL_BASE,
};
use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine as _};
use percent_encoding::{utf8_percent_encode, NON_ALPHANUMERIC};
use rand::RngCore;
use sha2::{Digest, Sha256};
use std::{
    collections::HashMap,
    sync::Arc,
    time::{Duration, Instant},
};
use subtle::ConstantTimeEq;
use time::{format_description::well_known::Rfc3339, OffsetDateTime};
use tokio::{
    io::AsyncWriteExt,
    net::{TcpListener, TcpStream},
    sync::Mutex,
};

pub const LOGIN_TTL: Duration = Duration::from_secs(600);

#[derive(Clone, Debug)]
pub struct AuthorizedSession {
    pub bearer_token: String,
    pub refresh_token: String,
    pub expires_in_secs: u64,
    pub expires_at: String,
    pub context: AuthorizationContext,
}

#[derive(Clone, Debug)]
pub enum LoginStepOutcome {
    AuthorizeRedirect { authorize_url: String },
    Completed(AuthorizedSession),
}

struct DiscoveredRegistration {
    idc_region: String,
    issuer_url: String,
    tenant_url: Option<String>,
    data_plane_base: String,
    ws_host: String,
    client: RegisteredClient,
    authorize_url: String,
}

struct FlowState {
    nonce: String,
    state: String,
    code_verifier: String,
    code_challenge: String,
    discovered: Option<DiscoveredRegistration>,
    completed: bool,
}

#[derive(Clone)]
pub struct PendingLogin {
    port: u16,
    redirect_uri: String,
    discovery_url: String,
    deadline: Instant,
    http: rquest::Client,
    flow: Arc<Mutex<FlowState>>,
}

impl PendingLogin {
    pub async fn start(http: rquest::Client) -> Result<(Self, TcpListener), OAuthError> {
        let nonce = random_b64url_32()?;
        let state = random_b64url_32()?;
        let code_verifier = random_b64url_32()?;
        let code_challenge = URL_SAFE_NO_PAD.encode(Sha256::digest(code_verifier.as_bytes()));
        let listener = TcpListener::bind(("127.0.0.1", 0))
            .await
            .map_err(|_| OAuthError::Listener)?;
        let port = listener
            .local_addr()
            .map_err(|_| OAuthError::Listener)?
            .port();
        let redirect_uri = format!("http://127.0.0.1:{port}");
        let surface = format!("desktop:{nonce}:{port}:{APP_VERSION}");
        let encoded_surface = utf8_percent_encode(&surface, NON_ALPHANUMERIC);
        let portal = discovery_origin();
        let discovery_url = format!("{portal}/sn?qwSurface={encoded_surface}");
        let login = Self {
            port,
            redirect_uri,
            discovery_url,
            deadline: Instant::now() + LOGIN_TTL,
            http,
            flow: Arc::new(Mutex::new(FlowState {
                nonce,
                state,
                code_verifier,
                code_challenge,
                discovered: None,
                completed: false,
            })),
        };
        Ok((login, listener))
    }

    pub fn discovery_url(&self) -> &str {
        &self.discovery_url
    }

    pub fn redirect_uri(&self) -> &str {
        &self.redirect_uri
    }

    pub fn is_expired(&self) -> bool {
        Instant::now() >= self.deadline
    }

    pub async fn submit_callback_url(&self, raw_url: &str) -> Result<LoginStepOutcome, OAuthError> {
        if self.is_expired() {
            return Err(OAuthError::Expired);
        }
        let params = parse_callback_query(raw_url, self.port)?;
        if let Some(err) = params.get("error") {
            return Err(if err == "access_denied" {
                OAuthError::AccessDenied
            } else {
                OAuthError::InvalidCallback
            });
        }
        let mut guard = self.flow.lock().await;
        if guard.completed {
            return Err(OAuthError::InvalidCallback);
        }
        if let Some(code) = params.get("code") {
            let state = params.get("state").ok_or(OAuthError::InvalidCallback)?;
            if !bool::from(state.as_bytes().ct_eq(guard.state.as_bytes())) || !atom(code, 4096) {
                return Err(OAuthError::StateMismatch);
            }
            let reg = guard
                .discovered
                .as_ref()
                .ok_or(OAuthError::InvalidCallback)?;
            let tokens = exchange_code(
                &self.http,
                &reg.idc_region,
                &reg.client,
                code,
                &self.redirect_uri,
                &guard.code_verifier,
            )
            .await?;
            let profile = fetch_user_info(
                &self.http,
                &reg.data_plane_base,
                &tokens.access_token,
                tokens.id_token.as_deref(),
            )
            .await;
            let bearer_token = handshake_token(tokens.id_token.as_deref(), &tokens.access_token);
            let expires_at = (OffsetDateTime::now_utc()
                + time::Duration::seconds(tokens.expires_in_secs as i64))
            .format(&Rfc3339)
            .map_err(|_| OAuthError::InvalidResponse)?;
            let context = AuthorizationContext {
                idc_region: reg.idc_region.clone(),
                issuer_url: reg.issuer_url.clone(),
                tenant_url: reg.tenant_url.clone(),
                ws_host: reg.ws_host.clone(),
                client_id: reg.client.client_id.clone(),
                client_secret: Some(reg.client.client_secret.clone()),
                client_secret_expires_at: reg.client.client_secret_expires_at,
                actor_arn: profile.actor_arn,
                aws_account_id: profile.aws_account_id,
                email: profile.email,
                display_name: profile.display_name,
                account_name: profile.account_name,
            };
            context.validate()?;
            guard.completed = true;
            return Ok(LoginStepOutcome::Completed(AuthorizedSession {
                bearer_token,
                refresh_token: tokens.refresh_token,
                expires_in_secs: tokens.expires_in_secs,
                expires_at,
                context,
            }));
        }

        let nonce = params.get("nonce").ok_or(OAuthError::InvalidCallback)?;
        if !bool::from(nonce.as_bytes().ct_eq(guard.nonce.as_bytes())) {
            return Err(OAuthError::StateMismatch);
        }
        if let Some(existing) = &guard.discovered {
            return Ok(LoginStepOutcome::AuthorizeRedirect {
                authorize_url: existing.authorize_url.clone(),
            });
        }
        let issuer_url = params
            .get("issuerUrl")
            .or_else(|| params.get("issuer_url"))
            .or_else(|| params.get("issuer"))
            .map(|s| s.trim().to_owned())
            .filter(|s| valid_https_url(s))
            .ok_or(OAuthError::InvalidCallback)?;
        let tenant_url = params
            .get("tenantUrl")
            .or_else(|| params.get("tenant_url"))
            .map(|s| s.trim().to_owned())
            .filter(|s| !s.is_empty());
        let idc_region = params
            .get("idcRegion")
            .or_else(|| params.get("idc_region"))
            .map(|s| s.trim())
            .filter(|s| !s.is_empty())
            .unwrap_or("us-east-1")
            .to_owned();
        if !commercial_region(&idc_region) {
            return Err(OAuthError::InvalidCallback);
        }
        let (data_plane_base, ws_host) = resolve_data_plane(tenant_url.as_deref())?;
        let client =
            register_oidc_client(&self.http, &idc_region, &issuer_url, &self.redirect_uri).await?;
        let oidc_origin = AuthorizationContext::oidc_origin_for_region(&idc_region)?;
        let authorize_url = build_authorize_url(
            &oidc_origin,
            &client.client_id,
            &self.redirect_uri,
            &guard.state,
            &guard.code_challenge,
        );
        guard.discovered = Some(DiscoveredRegistration {
            idc_region,
            issuer_url,
            tenant_url,
            data_plane_base,
            ws_host,
            client,
            authorize_url: authorize_url.clone(),
        });
        Ok(LoginStepOutcome::AuthorizeRedirect { authorize_url })
    }

    pub async fn run_loopback<F>(
        &self,
        listener: TcpListener,
        mut on_authorize: F,
    ) -> Result<AuthorizedSession, OAuthError>
    where
        F: FnMut(String),
    {
        loop {
            let remaining = self.deadline.saturating_duration_since(Instant::now());
            if remaining.is_zero() {
                return Err(OAuthError::Expired);
            }
            let (mut stream, _) = tokio::time::timeout(remaining, listener.accept())
                .await
                .map_err(|_| OAuthError::Expired)?
                .map_err(|_| OAuthError::Listener)?;
            let Ok(head) = tokio::time::timeout(
                Duration::from_secs(5),
                crate::protocol::loopback_callback::read_head(&mut stream),
            )
            .await
            else {
                continue;
            };
            let Ok(head) = head else { continue };
            let Some(url) = extract_loopback_url(&head, self.port) else {
                let _ = write_http(&mut stream, "404 Not Found", None, "Not found.").await;
                continue;
            };
            match self.submit_callback_url(&url).await {
                Ok(LoginStepOutcome::AuthorizeRedirect { authorize_url }) => {
                    on_authorize(authorize_url.clone());
                    let _ = write_http(&mut stream, "302 Found", Some(&authorize_url), "").await;
                }
                Ok(LoginStepOutcome::Completed(session)) => {
                    let _ = write_http(
                        &mut stream,
                        "200 OK",
                        None,
                        "Amazon Quick authorization received. Return to Neuro Gateway to save.",
                    )
                    .await;
                    return Ok(session);
                }
                Err(err) => {
                    let _ = write_http(
                        &mut stream,
                        "400 Bad Request",
                        None,
                        "Amazon Quick authorization failed. Return to Neuro Gateway.",
                    )
                    .await;
                    return Err(err);
                }
            }
        }
    }
}

fn discovery_origin() -> String {
    #[cfg(test)]
    if let Ok(origin) = super::TEST_DISCOVERY_ORIGIN.try_with(|v| v.clone()) {
        return origin.trim_end_matches('/').to_owned();
    }
    PORTAL_BASE.to_owned()
}

fn random_b64url_32() -> Result<String, OAuthError> {
    let mut bytes = [0u8; 32];
    rand::rngs::OsRng
        .try_fill_bytes(&mut bytes)
        .map_err(|_| OAuthError::Entropy)?;
    Ok(URL_SAFE_NO_PAD.encode(bytes))
}

fn build_authorize_url(
    oidc_origin: &str,
    client_id: &str,
    redirect_uri: &str,
    state: &str,
    challenge: &str,
) -> String {
    let enc = |v: &str| utf8_percent_encode(v, NON_ALPHANUMERIC).to_string();
    format!(
        "{oidc_origin}/authorize?response_type=code&client_id={}&redirect_uri={}&scope={}&state={}&code_challenge={}&code_challenge_method=S256",
        enc(client_id),
        enc(redirect_uri),
        enc(OIDC_SCOPE),
        enc(state),
        enc(challenge),
    )
}

fn parse_callback_query(
    raw: &str,
    expected_port: u16,
) -> Result<HashMap<String, String>, OAuthError> {
    let trimmed = raw.trim();
    if trimmed.is_empty() || trimmed.len() > 16_384 {
        return Err(OAuthError::InvalidCallback);
    }
    let url = url::Url::parse(trimmed).map_err(|_| OAuthError::InvalidCallback)?;
    if url.scheme() != "http"
        || !matches!(url.host_str(), Some("127.0.0.1" | "localhost"))
        || url.port() != Some(expected_port)
        || !matches!(url.path(), "/callback" | "/" | "/index.html")
        || url.fragment().is_some()
    {
        return Err(OAuthError::InvalidCallback);
    }
    Ok(url.query_pairs().into_owned().collect())
}

fn extract_loopback_url(head: &[u8], port: u16) -> Option<String> {
    let text = std::str::from_utf8(head).ok()?.strip_suffix("\r\n\r\n")?;
    let mut lines = text.split("\r\n");
    let parts: Vec<_> = lines.next()?.split(' ').collect();
    let ["GET", uri, "HTTP/1.1"] = parts.as_slice() else {
        return None;
    };
    if uri.len() > 16_384 || !uri.contains('?') || uri.contains('#') {
        return None;
    }
    let path = uri.split('?').next()?;
    if !matches!(path, "/callback" | "/" | "/index.html") {
        return None;
    }
    Some(format!("http://127.0.0.1:{port}{uri}"))
}

async fn write_http(
    stream: &mut TcpStream,
    status: &str,
    location: Option<&str>,
    body: &str,
) -> std::io::Result<()> {
    let loc_header = location
        .map(|l| format!("Location: {l}\r\n"))
        .unwrap_or_default();
    let reply = format!(
        "HTTP/1.1 {status}\r\n{loc_header}Content-Type: text/plain; charset=utf-8\r\nCache-Control: no-store\r\nConnection: close\r\nContent-Length: {}\r\n\r\n{body}",
        body.len()
    );
    stream.write_all(reply.as_bytes()).await?;
    stream.shutdown().await
}
