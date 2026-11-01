//! Operator-owned, bounded PKCE sessions. Ephemeral secrets never enter status views.
mod callback;
mod material;
mod persistence;
#[cfg(test)]
mod tests;
pub use callback::parse_callback;

use crate::{error::GatewayError, protocol::antigravity as config, state::AppState};
use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
use parking_lot::Mutex;
use rand::{rngs::OsRng, RngCore};
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::{
    collections::HashMap,
    sync::{Arc, OnceLock},
    time::{Duration, Instant},
};
use tokio::sync::Notify;
const TTL: Duration = Duration::from_secs(600);
const RECEIPT_TTL: Duration = Duration::from_secs(300);
static SESSIONS: OnceLock<Mutex<HashMap<String, Arc<Session>>>> = OnceLock::new();
fn sessions() -> &'static Mutex<HashMap<String, Arc<Session>>> {
    SESSIONS.get_or_init(|| Mutex::new(HashMap::new()))
}

pub struct Session {
    id: String,
    owner: String,
    provider: String,
    created: Instant,
    nonce: String,
    verifier: String,
    oauth_client: config::OAuthClient,
    auth_url: String,
    cancel: Arc<Notify>,
    inner: Mutex<SessionState>,
}
struct SessionState {
    status: &'static str,
    message: String,
    material: Option<material::Material>,
    credential_id: Option<String>,
    receipt_at: Option<Instant>,
}
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionView {
    id: String,
    status: &'static str,
    message: String,
    authorization_url: String,
    credential_id: Option<String>,
}

fn random_secret() -> String {
    let mut bytes = [0u8; 32];
    OsRng.fill_bytes(&mut bytes);
    URL_SAFE_NO_PAD.encode(bytes)
}
fn authorization_url(client: &config::OAuthClient, nonce: &str, verifier: &str) -> String {
    let challenge = URL_SAFE_NO_PAD.encode(Sha256::digest(verifier.as_bytes()));
    let mut url =
        url::Url::parse("https://accounts.google.com/o/oauth2/v2/auth").expect("fixed OAuth URL");
    url.query_pairs_mut().extend_pairs([
        ("client_id", client.client_id()),
        ("response_type", "code"),
        ("redirect_uri", config::REDIRECT_URI),
        ("scope", config::SCOPES),
        ("state", nonce),
        ("code_challenge", &challenge),
        ("code_challenge_method", "S256"),
        ("access_type", "offline"),
        ("prompt", "consent"),
    ]);
    url.to_string()
}

pub fn get(id: &str, owner: &str) -> Result<Arc<Session>, GatewayError> {
    sessions()
        .lock()
        .get(id)
        .filter(|s| s.owner == owner && s.retention_delay().is_some())
        .cloned()
        .ok_or_else(|| {
            GatewayError::bad_request("Antigravity login expired or unavailable")
                .with_code("antigravity_auth_session_unavailable")
        })
}

pub async fn create(
    state: &Arc<AppState>,
    owner: String,
    provider: String,
    local: bool,
) -> Result<Arc<Session>, GatewayError> {
    persistence::validate_provider(state, &provider)?;
    let oauth_client = config::OAuthClient::load().await.map_err(|code| {
        GatewayError::service_unavailable(
            "Correct or remove the explicit Antigravity OAuth client override before login",
        )
        .with_code(code)
    })?;
    let listener = if local {
        Some(
            tokio::net::TcpListener::bind("127.0.0.1:51121")
                .await
                .map_err(|_| {
                    GatewayError::bad_request(
                        "OAuth callback port 51121 is in use; finish the other login first",
                    )
                })?,
        )
    } else {
        None
    };
    let nonce = random_secret();
    let verifier = random_secret();
    let session = Arc::new(Session {
        id: uuid::Uuid::new_v4().to_string(),
        owner,
        provider,
        created: Instant::now(),
        auth_url: authorization_url(&oauth_client, &nonce, &verifier),
        oauth_client,
        nonce,
        verifier,
        cancel: Arc::new(Notify::new()),
        inner: Mutex::new(SessionState {
            status: "waiting_user",
            message: "Complete Antigravity Google authorization in your browser.".into(),
            material: None,
            credential_id: None,
            receipt_at: None,
        }),
    });
    {
        let mut map = sessions().lock();
        map.retain(|_, s| s.retention_delay().is_some());
        if map.len() >= 16 {
            return Err(GatewayError::bad_request(
                "Too many pending Antigravity logins",
            ));
        }
        map.insert(session.id.clone(), session.clone());
    }
    if let Some(listener) = listener {
        callback::serve(
            listener,
            session.clone(),
            state.upstream_client.client().clone(),
        );
    }
    let expiring = session.clone();
    tokio::spawn(async move {
        tokio::time::sleep(TTL).await;
        // An admitted commit may cross the authorization TTL. Keep its status readable;
        // only the non-secret completion receipt gets a fresh, bounded recovery period.
        while let Some(delay) = expiring.retention_delay() {
            tokio::time::sleep(delay).await;
        }
        expiring.cancel();
        sessions().lock().remove(&expiring.id);
    });
    Ok(session)
}

impl Session {
    fn retention_delay(&self) -> Option<Duration> {
        let inner = self.inner.lock();
        if inner.status == "importing" {
            return Some(Duration::from_secs(1));
        }
        if inner.status == "succeeded" {
            return inner
                .receipt_at
                .and_then(|at| RECEIPT_TTL.checked_sub(at.elapsed()));
        }
        TTL.checked_sub(self.created.elapsed())
    }
    pub fn view(&self) -> SessionView {
        let inner = self.inner.lock();
        SessionView {
            id: self.id.clone(),
            status: inner.status,
            message: inner.message.clone(),
            authorization_url: self.auth_url.clone(),
            credential_id: inner.credential_id.clone(),
        }
    }
    pub fn cancel(&self) {
        let mut inner = self.inner.lock();
        if !matches!(inner.status, "succeeded" | "importing") {
            inner.status = "cancelled";
            inner.material = None;
            inner.message = "Antigravity authorization cancelled or expired.".into();
        }
        self.cancel.notify_one();
    }
    fn begin_exchange(&self, nonce: &str) -> Result<(), GatewayError> {
        use subtle::ConstantTimeEq;
        let mut inner = self.inner.lock();
        if self.created.elapsed() >= TTL
            || inner.status != "waiting_user"
            || !bool::from(self.nonce.as_bytes().ct_eq(nonce.as_bytes()))
        {
            return Err(GatewayError::bad_request(
                "Invalid, expired or consumed OAuth state",
            ));
        }
        inner.status = "exchanging";
        Ok(())
    }
    pub async fn complete(
        &self,
        client: &rquest::Client,
        code: &str,
        nonce: &str,
    ) -> Result<(), GatewayError> {
        if code.is_empty() || code.len() > 8192 {
            return Err(GatewayError::bad_request(
                "Invalid OAuth authorization code",
            ));
        }
        self.begin_exchange(nonce)?;
        let result = material::exchange(client, code, &self.verifier, &self.oauth_client).await;
        let mut inner = self.inner.lock();
        if inner.status != "exchanging" || self.created.elapsed() >= TTL {
            return Err(GatewayError::bad_request("OAuth session cancelled"));
        }
        self.cancel.notify_one();
        match result {
            Ok(material) => {
                inner.material = Some(material);
                inner.status = "ready";
                inner.message = "Antigravity authorization received; ready to save.".into();
                Ok(())
            }
            Err(error) => {
                inner.status = "failed";
                inner.message = error.message.clone();
                Err(error)
            }
        }
    }
    pub async fn import(&self, state: &AppState) -> Result<String, GatewayError> {
        let material = {
            let mut inner = self.inner.lock();
            if inner.status == "succeeded" {
                return Ok(inner.credential_id.clone().unwrap_or_default());
            }
            if inner.status != "ready" || self.created.elapsed() >= TTL {
                return Err(GatewayError::bad_request(
                    "Antigravity OAuth session is not ready to import",
                ));
            }
            let material = inner
                .material
                .take()
                .ok_or_else(|| GatewayError::bad_request("OAuth credential unavailable"))?;
            inner.status = "importing";
            material
        };
        let result = persistence::save(state, &self.provider, &material, self.created + TTL).await;
        let mut inner = self.inner.lock();
        match result {
            Ok(id) => {
                inner.status = "succeeded";
                inner.message = "Antigravity credential saved.".into();
                inner.credential_id = Some(id.clone());
                inner.receipt_at = Some(Instant::now());
                Ok(id)
            }
            Err(error) => {
                if self.created.elapsed() < TTL {
                    inner.status = "ready";
                    inner.material = Some(material);
                } else {
                    inner.status = "cancelled";
                }
                inner.message = error.message.clone();
                Err(error)
            }
        }
    }
}
