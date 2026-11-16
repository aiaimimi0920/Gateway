//! Client credentials are repeatable grants. Cache is process-local, bounded and owned by UpstreamClient.
use super::*;
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeMap,
    sync::Arc,
    time::{Duration, Instant, SystemTime},
};
use tokio::sync::Mutex;
struct Token {
    value: String,
    expires: Instant,
    wall_expires: SystemTime,
}
type Slot = Arc<Mutex<Option<Token>>>;
#[derive(Default)]
pub struct TokenCache {
    slots: parking_lot::Mutex<BTreeMap<[u8; 32], Slot>>,
}
impl TokenCache {
    pub async fn get(
        &self,
        http: &rquest::Client,
        payload: &ProviderAccountPayload,
    ) -> Result<String, GatewayError> {
        validate(payload, payload.canonical_adapter())?;
        let endpoint = Endpoint::parse(&payload.base_url).ok_or_else(invalid)?;
        let key = ServiceKey::parse(&payload.api_key, &endpoint)?;
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
        let slot = {
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
            slots.entry(id).or_default().clone()
        };
        // The waiting future owns its cancellation. No detached fetch or secret-bearing background task.
        tokio::time::timeout(Duration::from_secs(15), async {
            let mut cached = slot.lock().await;
            if let Some(token) = cached.as_ref().filter(|t| t.expires > Instant::now() && t.wall_expires > SystemTime::now()) {
                return Ok(token.value.clone());
            }
            *cached = None;
            let started = Instant::now();
            let wall_started = SystemTime::now();
            let url = format!("{}/oauth/token", key.url.trim_end_matches('/'));
            #[cfg(test)]
            let url = super::test_support::url(&url);
            let response = http.post(&url).default_headers(false).redirect(rquest::redirect::Policy::none())
                .basic_auth(&key.clientid, Some(&key.clientsecret))
                .form(&[("grant_type", "client_credentials")]).send().await
                .map_err(|_| failure("sap_aicore_token_failed"))?;
            if !response.status().is_success() { return Err(failure("sap_aicore_token_failed")); }
            let bytes = crate::protocol::upstream_body::collect_bounded_upstream_stream_with_limit_and_provider(response.bytes_stream(), "SAP OAuth", 65_536, None).await
                .map_err(|_| failure("sap_aicore_token_failed"))?;
            #[derive(serde::Deserialize)]
            struct Reply { access_token: String, token_type: String, expires_in: u64 }
            let reply: Reply = serde_json::from_slice(&bytes).map_err(|_| failure("sap_aicore_token_failed"))?;
            if !reply.token_type.eq_ignore_ascii_case("bearer") || reply.access_token.is_empty()
                || reply.access_token.len() > 32_768 || !reply.access_token.bytes().all(|b| b.is_ascii_graphic())
                || reply.expires_in == 0 {
                return Err(failure("sap_aicore_token_failed"));
            }
            let ttl = Duration::from_secs(reply.expires_in.saturating_sub((reply.expires_in / 10).min(30)));
            let expires = started.checked_add(ttl).ok_or_else(|| failure("sap_aicore_token_failed"))?;
            let wall_expires = wall_started.checked_add(ttl).ok_or_else(|| failure("sap_aicore_token_failed"))?;
            if expires <= Instant::now() || wall_expires <= SystemTime::now() { return Err(failure("sap_aicore_token_failed")); }
            *cached = Some(Token { value: reply.access_token.clone(), expires, wall_expires });
            Ok(reply.access_token)
        }).await.map_err(|_| failure("sap_aicore_token_timeout"))?
    }
}
