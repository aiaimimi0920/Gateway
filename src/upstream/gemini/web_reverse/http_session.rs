//! One request owns loaded state; commits never alter an accepted result or replay business I/O.
use super::{http_checkpoint::Checkpoint, session_cookies::SessionCookies};
use crate::{
    error::{ErrorKind, GatewayError},
    routing::candidate::ProviderAccountPayload,
};
use rquest::header::HeaderMap;
use std::ops::{Deref, DerefMut};

pub(super) struct HttpSession {
    cookies: SessionCookies,
    checkpoint: Option<Checkpoint>,
}

impl std::fmt::Debug for HttpSession {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("HttpSession([redacted])")
    }
}

impl HttpSession {
    pub(super) async fn start(
        base: &str,
        payload: &ProviderAccountPayload,
        headers: &HeaderMap,
    ) -> Result<Self, GatewayError> {
        let mut cookies = SessionCookies::new(base, headers)?;
        let checkpoint = match Checkpoint::load(base, payload, headers).await? {
            None => None,
            Some((checkpoint, state)) => {
                if let Some(state) = state {
                    cookies = SessionCookies::restore(base, state)?;
                }
                Some(checkpoint)
            }
        };
        Ok(Self {
            cookies,
            checkpoint,
        })
    }

    pub(super) async fn commit(&mut self) {
        let Some(checkpoint) = self.checkpoint.as_mut() else {
            return;
        };
        if let Err(error) = checkpoint.save(Some(self.cookies.snapshot())).await {
            // Never log filesystem, SQL, codec details or the session itself.
            tracing::warn!(
                code = error
                    .code
                    .as_deref()
                    .unwrap_or("gemini_web_http_auth_checkpoint_failed"),
                "Gemini HTTP auth checkpoint not committed"
            );
        }
    }

    pub(super) async fn invalidate(&mut self, error: &GatewayError) {
        if error.kind != ErrorKind::Authentication {
            return;
        }
        let Some(checkpoint) = self.checkpoint.as_mut() else {
            return;
        };
        if let Err(error) = checkpoint.save(None).await {
            tracing::warn!(
                code = error
                    .code
                    .as_deref()
                    .unwrap_or("gemini_web_http_auth_checkpoint_failed"),
                "Gemini HTTP auth invalidation not committed"
            );
        }
    }
}

impl Deref for HttpSession {
    type Target = SessionCookies;
    fn deref(&self) -> &Self::Target {
        &self.cookies
    }
}
impl DerefMut for HttpSession {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.cookies
    }
}
