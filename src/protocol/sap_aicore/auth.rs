//! Client credentials are repeatable grants. Cache is process-local, bounded and owned by UpstreamClient.
use super::*;
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeMap,
    sync::{Arc, OnceLock},
    time::{Duration, Instant, SystemTime},
};
use tokio::sync::Mutex;
pub(super) struct Token {
    pub(super) value: String,
    expires: Instant,
    wall_expires: SystemTime,
}
type Slot = Arc<Mutex<Option<Token>>>;
#[derive(Default)]
pub struct TokenCache {
    slots: parking_lot::Mutex<BTreeMap<[u8; 32], Slot>>,
    pub(super) fixture: Option<fixture::Transport>,
    stored_http: OnceLock<Result<rquest::Client, ()>>,
}
impl TokenCache {
    pub(crate) fn for_runtime(config: &crate::config::Config) -> anyhow::Result<Self> {
        Ok(Self {
            fixture: fixture::Transport::load(config)?,
            ..Self::default()
        })
    }

    pub(super) fn http<'a>(&'a self, default: &'a rquest::Client) -> &'a rquest::Client {
        self.fixture
            .as_ref()
            .map_or(default, |fixture| &fixture.http)
    }

    pub(super) fn stored_http(&self) -> Result<&rquest::Client, GatewayError> {
        if let Some(fixture) = self.fixture.as_ref() {
            return Ok(&fixture.http);
        }
        self.stored_http
            .get_or_init(|| {
                #[cfg(not(test))]
                let builder = crate::http_client::builder();
                #[cfg(test)]
                let builder = rquest::Client::builder().no_proxy();
                builder
                    .retry(rquest::retry::Policy::never())
                    // A reused connection has a separate internal retry path.
                    .pool_max_idle_per_host(0)
                    .redirect(rquest::redirect::Policy::none())
                    .build()
                    .map_err(|_| ())
            })
            .as_ref()
            .map_err(|_| failure("sap_aicore_stored_transport_unavailable"))
    }

    fn slot(&self, id: [u8; 32]) -> Result<Slot, GatewayError> {
        let mut slots = self.slots.lock();
        if !slots.contains_key(&id) && slots.len() >= 256 {
            let idle = slots
                .iter()
                .find(|(_, slot)| Arc::strong_count(slot) == 1)
                .map(|(id, _)| *id);
            if let Some(idle) = idle {
                slots.remove(&idle);
            } else {
                return Err(failure("sap_aicore_auth_capacity"));
            }
        }
        Ok(slots.entry(id).or_default().clone())
    }

    pub async fn get(
        &self,
        http: &rquest::Client,
        payload: &ProviderAccountPayload,
    ) -> Result<String, GatewayError> {
        validate(payload, payload.canonical_adapter())?;
        let endpoint = Endpoint::parse(&payload.base_url).ok_or_else(invalid)?;
        let key = ServiceKey::parse(&payload.api_key, &endpoint)?;
        if self.fixture.is_some() {
            fixture::admit(&key, &endpoint.origin)?;
        }
        // Target/group and the exact secret revision scope reuse; no token is borrowed after replacement.
        let mut digest = Sha256::new();
        for part in [
            &payload.api_key,
            &payload.base_url,
            &payload.headers["AI-Resource-Group"],
        ] {
            digest.update((part.len() as u64).to_be_bytes());
            digest.update(part.as_bytes());
        }
        let id: [u8; 32] = digest.finalize().into();
        let slot = self.slot(id)?;
        // The waiting future owns its cancellation. No detached fetch or secret-bearing background task.
        tokio::time::timeout(Duration::from_secs(15), async {
            let mut cached = slot.lock().await;
            if let Some(token) = cached
                .as_ref()
                .filter(|t| t.expires > Instant::now() && t.wall_expires > SystemTime::now())
            {
                return Ok(token.value.clone());
            }
            *cached = None;
            let token = exchange_token(self.http(http), &key, self.fixture.as_ref()).await?;
            let value = token.value.clone();
            *cached = Some(token);
            Ok(value)
        })
        .await
        .map_err(|_| failure("sap_aicore_token_timeout"))?
    }
}

pub(super) async fn exchange_token(
    http: &rquest::Client,
    key: &ServiceKey,
    fixture: Option<&fixture::Transport>,
) -> Result<Token, GatewayError> {
    let started = Instant::now();
    let wall_started = SystemTime::now();
    use super::service_key::Credential;
    let bytes = match &key.credential {
        Credential::Secret { url, secret } => {
            exchange_secret(http, url, &key.clientid, secret).await?
        }
        Credential::Certificate {
            url,
            certificate,
            key: private_key,
        } => mtls::exchange(url, &key.clientid, certificate, private_key, fixture).await?,
    };
    #[derive(serde::Deserialize)]
    struct Reply {
        access_token: String,
        token_type: String,
        expires_in: u64,
    }
    let reply: Reply =
        serde_json::from_slice(&bytes).map_err(|_| failure("sap_aicore_token_failed"))?;
    if !reply.token_type.eq_ignore_ascii_case("bearer")
        || reply.access_token.is_empty()
        || reply.access_token.len() > 32_768
        || !reply.access_token.bytes().all(|b| b.is_ascii_graphic())
        || reply.expires_in == 0
    {
        return Err(failure("sap_aicore_token_failed"));
    }
    let ttl = Duration::from_secs(
        reply
            .expires_in
            .saturating_sub((reply.expires_in / 10).min(30)),
    );
    let expires = started
        .checked_add(ttl)
        .ok_or_else(|| failure("sap_aicore_token_failed"))?;
    let wall_expires = wall_started
        .checked_add(ttl)
        .ok_or_else(|| failure("sap_aicore_token_failed"))?;
    if expires <= Instant::now() || wall_expires <= SystemTime::now() {
        return Err(failure("sap_aicore_token_failed"));
    }
    Ok(Token {
        value: reply.access_token,
        expires,
        wall_expires,
    })
}

async fn exchange_secret(
    http: &rquest::Client,
    url: &str,
    clientid: &str,
    secret: &str,
) -> Result<Vec<u8>, GatewayError> {
    let url = format!("{}/oauth/token", url);
    #[cfg(test)]
    let url = super::test_support::url(&url);
    let response = http
        .post(&url)
        .default_headers(false)
        .redirect(rquest::redirect::Policy::none())
        .basic_auth(clientid, Some(secret))
        .form(&[("grant_type", "client_credentials")])
        .send()
        .await
        .map_err(|_| failure("sap_aicore_token_failed"))?;
    if !response.status().is_success() {
        return Err(errors::http_rejected(&response, true));
    }
    crate::protocol::upstream_body::collect_bounded_upstream_stream_with_limit_and_provider(
        response.bytes_stream(),
        "SAP OAuth",
        mtls::MAX_TOKEN_BYTES,
        None,
    )
    .await
    .map_err(|_| failure("sap_aicore_token_failed"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sap_aicore_token_capacity_preserves_active_slots_and_reuses_existing_key() {
        let cache = TokenCache::default();
        let mut active: Vec<_> = (0..=u8::MAX)
            .map(|index| cache.slot([index; 32]).unwrap())
            .collect();
        let mut new_key = [0; 32];
        new_key[0] = 7;
        assert!(Arc::ptr_eq(&active[0], &cache.slot([0; 32]).unwrap()));
        let error = cache.slot(new_key).err().expect("all slots are active");
        assert!(!error.retryable);
        assert_eq!(error.code.as_deref(), Some("sap_aicore_auth_capacity"));
        assert_eq!(cache.slots.lock().len(), 256);

        drop(active.pop().unwrap());
        let new_slot = cache.slot(new_key).unwrap();
        let slots = cache.slots.lock();
        assert_eq!(slots.len(), 256);
        assert!(!slots.contains_key(&[u8::MAX; 32]));
        assert!(Arc::ptr_eq(&slots[&new_key], &new_slot));
        assert!(active
            .iter()
            .enumerate()
            .all(|(idx, slot)| Arc::ptr_eq(&slots[&[idx as u8; 32]], slot)));
    }
}
