//! Authenticated encryption binds opaque state to its seed and storage namespace.
use ring::{
    aead,
    rand::{SecureRandom, SystemRandom},
};
use serde::{Deserialize, Serialize};

use super::{error, Config, MAX_BYTES};
use crate::{error::GatewayError, upstream::gemini::web_reverse::session_cookies::CookieSnapshot};

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Record {
    version: u8,
    pub revision: String,
    pub blocked: bool,
    pub state: Option<CookieSnapshot>,
}

fn key(config: &Config) -> Result<aead::LessSafeKey, GatewayError> {
    aead::UnboundKey::new(&aead::AES_256_GCM, &config.key)
        .map(aead::LessSafeKey::new)
        .map_err(|_| error("crypto_failed"))
}

pub(super) fn encrypt(
    config: &Config,
    binding: &str,
    state: Option<CookieSnapshot>,
) -> Result<(String, Vec<u8>), GatewayError> {
    let revision = uuid::Uuid::new_v4().to_string();
    let record = Record {
        version: 1,
        revision: revision.clone(),
        blocked: state.is_none(),
        state,
    };
    let mut bytes = serde_json::to_vec(&record).map_err(|_| error("invalid_record"))?;
    if bytes.len() > MAX_BYTES - 33 {
        return Err(error("too_large"));
    }
    let mut nonce = [0u8; 12];
    SystemRandom::new()
        .fill(&mut nonce)
        .map_err(|_| error("crypto_failed"))?;
    key(config)?
        .seal_in_place_append_tag(
            aead::Nonce::assume_unique_for_key(nonce),
            aead::Aad::from(binding.as_bytes()),
            &mut bytes,
        )
        .map_err(|_| error("crypto_failed"))?;
    let mut output = Vec::with_capacity(bytes.len() + 17);
    output.extend_from_slice(b"NGHC1");
    output.extend_from_slice(&nonce);
    output.extend_from_slice(&bytes);
    Ok((revision, output))
}

pub(super) fn decrypt(
    config: &Config,
    binding: &str,
    mut bytes: Vec<u8>,
) -> Result<Record, GatewayError> {
    let invalid = || error("invalid_record");
    if bytes.len() < 34 || bytes.len() > MAX_BYTES || &bytes[..5] != b"NGHC1" {
        return Err(invalid());
    }
    let nonce: [u8; 12] = bytes[5..17].try_into().map_err(|_| invalid())?;
    let plain = key(config)?
        .open_in_place(
            aead::Nonce::assume_unique_for_key(nonce),
            aead::Aad::from(binding.as_bytes()),
            &mut bytes[17..],
        )
        .map_err(|_| invalid())?;
    let record: Record = serde_json::from_slice(plain).map_err(|_| invalid())?;
    if record.version != 1
        || uuid::Uuid::parse_str(&record.revision).is_err()
        || record.blocked != record.state.is_none()
    {
        return Err(invalid());
    }
    Ok(record)
}
