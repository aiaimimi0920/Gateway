//! Bounded, operator-owned PKCE sessions with ephemeral token material and explicit import.
mod callback;
mod material;
mod persistence;
pub use callback::parse_callback;

use crate::{error::GatewayError, state::AppState};
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
type Sessions = Mutex<HashMap<String, Arc<Session>>>;
static SESSIONS: OnceLock<Sessions> = OnceLock::new();
fn sessions() -> &'static Sessions {
    SESSIONS.get_or_init(|| Mutex::new(HashMap::new()))
}

pub struct Session {
    id: String,
    owner: String,
    provider: String,
    group: String,
    created: Instant,
    nonce: String,
    verifier: String,
    auth_url: String,
    cancel: Arc<Notify>,
    inner: Mutex<SessionState>,
}

struct SessionState {
    status: &'static str,
    message: String,
    material: Option<material::Material>,
    credential_id: Option<String>,
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

pub fn get(id: &str, owner: &str) -> Result<Arc<Session>, GatewayError> {
    let map = sessions().lock();
    map.get(id)
        .filter(|s| s.owner == owner && s.created.elapsed() < TTL)
        .cloned()
        .ok_or_else(|| {
            GatewayError::bad_request("ChatGPT login session expired or unavailable")
                .with_code("chatgpt_auth_session_unavailable")
        })
}

fn random_secret() -> String {
    let mut bytes = [0u8; 32];
    OsRng.fill_bytes(&mut bytes);
    URL_SAFE_NO_PAD.encode(bytes)
}

fn authorization_url(nonce: &str, verifier: &str) -> String {
    let challenge = URL_SAFE_NO_PAD.encode(Sha256::digest(verifier.as_bytes()));
    let mut url =
        url::Url::parse("https://auth.openai.com/oauth/authorize").expect("fixed OAuth URL");
    url.query_pairs_mut().extend_pairs([
        (
            "client_id",
            crate::protocol::chatgpt::codex_client::CLIENT_ID,
        ),
        ("response_type", "code"),
        ("redirect_uri", material::REDIRECT_URI),
        ("scope", "openid email profile offline_access"),
        ("state", nonce),
        ("code_challenge", &challenge),
        ("code_challenge_method", "S256"),
        ("prompt", "login"),
        ("id_token_add_organizations", "true"),
        ("codex_cli_simplified_flow", "true"),
    ]);
    url.to_string()
}

pub async fn create(
    state: &Arc<AppState>,
    owner: String,
    provider: String,
    group: String,
    local: bool,
) -> Result<Arc<Session>, GatewayError> {
    if !material::GROUPS.contains(&group.as_str()) {
        return Err(GatewayError::bad_request(
            "Unknown ChatGPT subscription group",
        ));
    }
    persistence::validate_provider(state, &provider)?;
    let listener = if local {
        Some(
            tokio::net::TcpListener::bind("127.0.0.1:1455")
                .await
                .map_err(|_| {
                    GatewayError::bad_request(
                        "OAuth callback port 1455 is in use; finish the other login first",
                    )
                })?,
        )
    } else {
        None
    };
    let verifier = random_secret();
    let nonce = random_secret();
    let session = Arc::new(Session {
        id: uuid::Uuid::new_v4().to_string(),
        owner,
        provider,
        group,
        created: Instant::now(),
        auth_url: authorization_url(&nonce, &verifier),
        nonce,
        verifier,
        cancel: Arc::new(Notify::new()),
        inner: Mutex::new(SessionState {
            status: "waiting_user",
            message: "Complete ChatGPT sign-in in your browser.".into(),
            material: None,
            credential_id: None,
        }),
    });
    {
        let mut map = sessions().lock();
        map.retain(|_, s| s.created.elapsed() < TTL);
        if map.len() >= 16 {
            return Err(GatewayError::bad_request("Too many pending ChatGPT logins"));
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
        expiring.cancel();
        sessions().lock().remove(&expiring.id);
    });
    Ok(session)
}

impl Session {
    pub async fn open_browser(&self) -> Result<(), GatewayError> {
        if self.inner.lock().status != "waiting_user" {
            return Err(GatewayError::bad_request(
                "Login is no longer waiting for browser authorization",
            ));
        }
        #[cfg(target_os = "windows")]
        let mut command = {
            let mut c = tokio::process::Command::new("rundll32.exe");
            c.arg("url.dll,FileProtocolHandler");
            c
        };
        #[cfg(target_os = "macos")]
        let mut command = tokio::process::Command::new("open");
        #[cfg(not(any(target_os = "windows", target_os = "macos")))]
        let mut command = tokio::process::Command::new("xdg-open");
        let result = tokio::time::timeout(
            Duration::from_secs(10),
            command.arg(&self.auth_url).kill_on_drop(true).status(),
        )
        .await;
        match result {
            Ok(Ok(status)) if status.success() => Ok(()),
            _ => Err(GatewayError::bad_request(
                "Cannot open browser; copy the authorization link instead",
            )),
        }
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
        if inner.status != "succeeded" && inner.status != "importing" {
            inner.status = "cancelled";
            inner.material = None;
            inner.message = "ChatGPT login cancelled or expired.".into();
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
                "Invalid, expired or already consumed OAuth state",
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
        let result = material::exchange(client, code, &self.verifier).await;
        let mut inner = self.inner.lock();
        if inner.status != "exchanging" || self.created.elapsed() >= TTL {
            return Err(GatewayError::bad_request("OAuth session cancelled"));
        }
        match result {
            Ok(material) => {
                inner.material = Some(material);
                inner.status = "ready";
                inner.message = "ChatGPT authorization received; ready to save.".into();
            }
            Err(error) => {
                inner.status = "failed";
                inner.message = error.message.clone();
                self.cancel.notify_one();
                return Err(error);
            }
        }
        self.cancel.notify_one();
        Ok(())
    }
    pub async fn import(&self, state: &AppState) -> Result<String, GatewayError> {
        let material = {
            let mut inner = self.inner.lock();
            if inner.status == "succeeded" {
                return Ok(inner.credential_id.clone().unwrap_or_default());
            }
            if inner.status != "ready" || self.created.elapsed() >= TTL {
                return Err(GatewayError::bad_request(
                    "OAuth session is not ready to import",
                ));
            }
            let material = inner
                .material
                .take()
                .ok_or_else(|| GatewayError::bad_request("OAuth credential unavailable"))?;
            inner.status = "importing";
            material
        };
        let result = persistence::save(
            state,
            &self.provider,
            &self.group,
            &material,
            self.created + TTL,
        )
        .await;
        let mut inner = self.inner.lock();
        match result {
            Ok(id) => {
                inner.status = "succeeded";
                inner.message = "ChatGPT credential saved.".into();
                inner.credential_id = Some(id.clone());
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

#[cfg(test)]
mod lifecycle_tests;
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn pkce_matches_rfc7636_and_state_is_unpredictable() {
        let url = url::Url::parse(&authorization_url(
            "state",
            "dBjftJeZ4CVP-mB92K27uhbUJU1p1r_wW1gFWFOEjXk",
        ))
        .unwrap();
        let query: HashMap<_, _> = url.query_pairs().collect();
        assert_eq!(
            query["code_challenge"],
            "E9Melhoa2OwvFrEMTJguCHaoeK1t8URWbuGJSstw-cM"
        );
        assert_ne!(random_secret(), random_secret());
        assert_eq!(random_secret().len(), 43);
    }
}
