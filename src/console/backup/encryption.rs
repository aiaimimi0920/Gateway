//! Versioned Argon2id + AES-256-GCM envelope with fixed, bounded KDF parameters.
use argon2::{Algorithm, Argon2, Params, Version};
use ring::{
    aead,
    rand::{SecureRandom, SystemRandom},
};

pub const MAX_BACKUP_BYTES: usize = 32 * 1024 * 1024;
const MAGIC: &[u8; 8] = b"NGBK\0\0\0\x01";
const HEADER_LEN: usize = 8 + 16 + 12;
const TAG_LEN: usize = 16;

fn key(password: &str, salt: &[u8]) -> Result<aead::LessSafeKey, &'static str> {
    if !(8..=1024).contains(&password.len()) {
        return Err("Backup password must contain 8 to 1024 UTF-8 bytes");
    }
    let params = Params::new(65536, 3, 1, Some(32))
        .map_err(|_| "Backup key derivation configuration failed")?;
    let mut bytes = [0u8; 32];
    let result = Argon2::new(Algorithm::Argon2id, Version::V0x13, params).hash_password_into(
        password.as_bytes(),
        salt,
        &mut bytes,
    );
    let key = result
        .map_err(|_| "Backup key derivation failed")
        .and_then(|_| {
            aead::UnboundKey::new(&aead::AES_256_GCM, &bytes)
                .map(aead::LessSafeKey::new)
                .map_err(|_| "Backup encryption initialization failed")
        });
    bytes.fill(0);
    key
}

pub fn encrypt(plaintext: &[u8], password: &str) -> Result<Vec<u8>, &'static str> {
    if plaintext.len() > MAX_BACKUP_BYTES {
        return Err("Backup exceeds size limit");
    }
    let mut header = [0u8; HEADER_LEN];
    header[..8].copy_from_slice(MAGIC);
    SystemRandom::new()
        .fill(&mut header[8..])
        .map_err(|_| "Backup randomness unavailable")?;
    let key = key(password, &header[8..24])?;
    let nonce = aead::Nonce::try_assume_unique_for_key(&header[24..])
        .map_err(|_| "Backup nonce initialization failed")?;
    let mut body = plaintext.to_vec();
    key.seal_in_place_append_tag(nonce, aead::Aad::from(&header), &mut body)
        .map_err(|_| "Backup encryption failed")?;
    let mut output = Vec::with_capacity(HEADER_LEN + body.len());
    output.extend_from_slice(&header);
    output.extend_from_slice(&body);
    Ok(output)
}

pub fn decrypt(envelope: &[u8], password: &str) -> Result<Vec<u8>, &'static str> {
    // Validate the fixed version and lengths before doing expensive password work.
    if envelope.len() < HEADER_LEN + TAG_LEN
        || envelope.len() > MAX_BACKUP_BYTES + HEADER_LEN + TAG_LEN
        || &envelope[..8] != MAGIC
    {
        return Err("Invalid or unsupported Gateway backup");
    }
    let (header, ciphertext) = envelope.split_at(HEADER_LEN);
    let key = key(password, &header[8..24])?;
    let nonce = aead::Nonce::try_assume_unique_for_key(&header[24..])
        .map_err(|_| "Invalid Gateway backup nonce")?;
    let mut plaintext = ciphertext.to_vec();
    let len = match key.open_in_place(nonce, aead::Aad::from(header), &mut plaintext) {
        Ok(bytes) => bytes.len(),
        Err(_) => {
            plaintext.fill(0);
            return Err("Incorrect password or damaged backup");
        }
    };
    plaintext.truncate(len);
    Ok(plaintext)
}
