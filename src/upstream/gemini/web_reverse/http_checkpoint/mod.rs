//! Pure HTTP auth persistence has its own namespace; it never imports browser state.
use super::session_cookies::{CookieSnapshot, SessionCookies};
use crate::{
    error::{FallbackHint, GatewayError},
    routing::candidate::ProviderAccountPayload,
};
use rquest::header::{HeaderMap, COOKIE};
use sha2::{Digest, Sha256};
use std::path::PathBuf;
mod codec;
mod config;
mod local;
const MAX_BYTES: usize = 128 * 1024;

pub(super) struct Config {
    directory: PathBuf,
    key: [u8; 32],
}
pub(super) struct Checkpoint {
    config: Config,
    binding: String,
    token: Option<String>,
}

fn error(kind: &str) -> GatewayError {
    let message = format!(
        "Gemini Web HTTP auth checkpoint {}.",
        kind.replace('_', " ")
    );
    let mut error = if kind == "session_invalid" {
        GatewayError::unauthorized(message)
    } else {
        GatewayError::server_error(message)
    };
    error.code = Some(format!("gemini_web_http_auth_checkpoint_{kind}"));
    error.provider_name = Some(super::PROVIDER.into());
    error.retryable = false;
    error.fallback_hint = FallbackHint::Abort {
        reason: "HTTP authentication checkpoint requires operator attention; do not replay.".into(),
    };
    error
}

impl Checkpoint {
    pub(super) async fn load(
        base: &str,
        payload: &ProviderAccountPayload,
        headers: &HeaderMap,
    ) -> Result<Option<(Self, Option<CookieSnapshot>)>, GatewayError> {
        let Some(config) = Config::from_env()? else {
            return Ok(None);
        };
        Self::load_with(config, base, payload, headers)
            .await
            .map(Some)
    }

    async fn load_with(
        config: Config,
        base: &str,
        payload: &ProviderAccountPayload,
        headers: &HeaderMap,
    ) -> Result<(Self, Option<CookieSnapshot>), GatewayError> {
        let id = payload
            .credential_id
            .as_deref()
            .filter(|id| !id.trim().is_empty())
            .ok_or_else(|| error("identity_missing"))?;
        // Validate before disk I/O; binding includes exact selected seed, origin and key namespace.
        SessionCookies::new(base, headers)?;
        let origin = url::Url::parse(base)
            .map_err(|_| error("identity_missing"))?
            .origin()
            .ascii_serialization();
        let seed = headers
            .get(COOKIE)
            .and_then(|v| v.to_str().ok())
            .unwrap_or("");
        let bytes = serde_json::to_vec(&(
            "http-auth-checkpoint-v1",
            &config.directory,
            hex::encode(Sha256::digest(config.key)),
            &payload.adapter,
            id,
            &payload.runtime_state_object_key,
            origin,
            seed,
        ))
        .map_err(|_| error("identity_missing"))?;
        let binding = hex::encode(Sha256::digest(bytes));
        let stored = local::read(&config, &binding).await?;
        let (token, state) = match stored {
            None => (None, None),
            Some((token, bytes)) => {
                let record = codec::decrypt(&config, &binding, bytes)?;
                if record.revision != token {
                    return Err(error("invalid_record"));
                }
                if record.blocked {
                    return Err(error("session_invalid"));
                }
                (Some(token), record.state)
            }
        };
        Ok((
            Self {
                config,
                binding,
                token,
            },
            state,
        ))
    }

    pub(super) async fn save(&mut self, state: Option<CookieSnapshot>) -> Result<(), GatewayError> {
        let (revision, bytes) = codec::encrypt(&self.config, &self.binding, state)?;
        local::write(
            &self.config,
            &self.binding,
            self.token.as_deref(),
            &revision,
            &bytes,
        )
        .await?;
        self.token = Some(revision);
        Ok(())
    }
}

#[cfg(test)]
mod tests;
