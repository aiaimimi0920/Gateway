//! A hosted audit may only read from the exact admitted database credential and key.
use super::*;
use crate::protocol::luma_agents as luma;
use hmac::{Hmac, Mac};
use sha2::Sha256;

type MaterialMac = Hmac<Sha256>;

#[derive(Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(super) struct Binding {
    provider_id: String,
    credential_id: String,
    material_tag: String,
}

impl Binding {
    pub(super) fn capture(
        state: &AppState,
        audit: &str,
        provider: &str,
        credential: &str,
        payload: &ProviderAccountPayload,
    ) -> Option<Self> {
        let mut binding = Self {
            provider_id: provider.into(),
            credential_id: credential.into(),
            material_tag: String::new(),
        };
        binding.material_tag =
            hex::encode(binding.mac(state, audit, payload)?.finalize().into_bytes());
        Some(binding)
    }

    pub(super) async fn target(
        &self,
        state: &AppState,
        audit: &str,
    ) -> Result<Option<ProviderAccountPayload>, GatewayError> {
        let Some(pool) = state.pg_pool.as_ref() else {
            return Ok(None);
        };
        let mut tag = [0u8; 32];
        if hex::decode_to_slice(&self.material_tag, &mut tag).is_err() {
            return Ok(None);
        }
        let payload = crate::db::routing::read_hosted_luma_payload(
            pool,
            &self.provider_id,
            &self.credential_id,
        )
        .await?;
        Ok(payload.filter(|payload| {
            self.mac(state, audit, payload)
                .is_some_and(|mac| mac.verify_slice(&tag).is_ok())
        }))
    }

    fn mac(
        &self,
        state: &AppState,
        audit: &str,
        payload: &ProviderAccountPayload,
    ) -> Option<MaterialMac> {
        if payload.adapter != luma::ADAPTER || !luma::valid_key(&payload.api_key) {
            return None;
        }
        luma::validate(payload, luma::ADAPTER).ok()?;
        let secret = state
            .config
            .gateway_api_key_secret
            .as_deref()
            .filter(|s| !s.is_empty())?;
        // Separate this key from public Key signatures; don't expose a reusable hash of Luma material.
        let mut derive = MaterialMac::new_from_slice(secret.as_bytes()).ok()?;
        derive.update(b"neuro-gateway/luma-hosted-reconciliation/v2\0");
        let mut mac = MaterialMac::new_from_slice(&derive.finalize().into_bytes()).ok()?;
        for field in [
            audit,
            &self.provider_id,
            &self.credential_id,
            luma::BASE,
            &payload.api_key,
        ] {
            mac.update(&(field.len() as u64).to_be_bytes());
            mac.update(field.as_bytes());
        }
        Some(mac)
    }
}
