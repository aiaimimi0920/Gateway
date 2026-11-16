//! Fresh synthetic signing keys only; never loads a user token or public issuer key.
use super::*;
use ring::signature::{EcdsaKeyPair, KeyPair, ECDSA_P256_SHA256_FIXED_SIGNING};

pub(super) const NOW: u64 = 1_800_000_000;
pub(super) struct Signer(EcdsaKeyPair);
impl Signer {
    pub(super) fn new() -> Self {
        let random = ring::rand::SystemRandom::new();
        let pkcs8 =
            EcdsaKeyPair::generate_pkcs8(&ECDSA_P256_SHA256_FIXED_SIGNING, &random).unwrap();
        Self(
            EcdsaKeyPair::from_pkcs8(&ECDSA_P256_SHA256_FIXED_SIGNING, pkcs8.as_ref(), &random)
                .unwrap(),
        )
    }
    pub(super) fn jwk(&self) -> Value {
        let key = self.0.public_key().as_ref();
        json!({"kty":"EC","crv":"P-256","alg":"ES256","use":"sig","kid":"synthetic-key","x":URL_SAFE_NO_PAD.encode(&key[1..33]),"y":URL_SAFE_NO_PAD.encode(&key[33..65])})
    }
    pub(super) fn jwks(&self) -> Vec<u8> {
        serde_json::to_vec(&json!({"keys":[self.jwk()]})).unwrap()
    }
    pub(super) fn sign(&self, claims: &Value) -> String {
        self.sign_raw(
            r#"{"alg":"ES256","kid":"synthetic-key","typ":"JWT"}"#,
            &claims.to_string(),
        )
    }
    pub(super) fn sign_raw(&self, header: &str, claims: &str) -> String {
        let input = format!(
            "{}.{}",
            URL_SAFE_NO_PAD.encode(header),
            URL_SAFE_NO_PAD.encode(claims)
        );
        let sig = self
            .0
            .sign(&ring::rand::SystemRandom::new(), input.as_bytes())
            .unwrap();
        format!("{input}.{}", URL_SAFE_NO_PAD.encode(sig.as_ref()))
    }
}
pub(super) fn claims(client: &OAuthClient, auth: &Authorization, now: u64) -> Value {
    json!({"iss":client.issuer,"sub":"private-synthetic-subject","aud":client.client_id,"exp":now+300,"iat":now-30,"nonce":auth.nonce()})
}
pub(super) fn authorization(client: &OAuthClient) -> Authorization {
    Authorization::new(client, "http://127.0.0.1:34567/callback").unwrap()
}
pub(super) fn tokens(id_token: Option<String>) -> UnverifiedTokens {
    UnverifiedTokens {
        access_token: "synthetic-access".into(),
        refresh_token: None,
        id_token,
        expires_in: Some(300),
        scope: None,
    }
}
