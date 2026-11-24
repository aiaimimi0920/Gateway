//! Match the effective SAP target, not a display name or a renewable OAuth token.
use super::{reference, Receipt, RoutePin};
use crate::routing::candidate::ProviderAccountPayload;
use hmac::{Hmac, Mac};
use sha2::Sha256;

impl RoutePin {
    fn binding_mac(
        &self,
        stable_secret: &[u8],
        provider: &str,
        credential: &str,
        payload: &ProviderAccountPayload,
    ) -> Option<Hmac<Sha256>> {
        if stable_secret.is_empty()
            || payload.api_key.is_empty()
            || !reference(provider)
            || !reference(credential)
            || payload.credential_id.as_deref() != Some(credential)
        {
            return None;
        }
        super::super::deployment(payload).ok()?;
        let mut mac = Hmac::<Sha256>::new_from_slice(stable_secret).ok()?;
        mac.update(b"neuro-gateway/sap-response-binding/v1\0");
        mac.update(&[u8::from(self.platform_access_ref.is_some())]);
        // The same SAP binding must deduplicate an upstream ID even across
        // callers or model aliases. Route authorization is compared separately.
        for field in [
            provider,
            credential,
            &payload.base_url,
            payload.headers.get("AI-Resource-Group")?,
            &payload.api_key,
        ] {
            mac.update(&(field.len() as u64).to_be_bytes());
            mac.update(field.as_bytes());
        }
        Some(mac)
    }

    pub(super) fn capture_binding(
        &self,
        stable_secret: &[u8],
        provider: &str,
        credential: &str,
        payload: &ProviderAccountPayload,
    ) -> Option<String> {
        Some(hex::encode(
            self.binding_mac(stable_secret, provider, credential, payload)?
                .finalize()
                .into_bytes(),
        ))
    }

    /// Call only with a freshly authorized selection. Equality of this tag is
    /// necessary, but does not by itself prove the principal still has access.
    pub(super) fn matches_binding(
        &self,
        current: &RoutePin,
        receipt: &Receipt,
        stable_secret: &[u8],
        provider: &str,
        credential: &str,
        payload: &ProviderAccountPayload,
    ) -> bool {
        if self != current
            || provider != receipt.provider_ref
            || credential != receipt.credential_ref
        {
            return false;
        }
        let mut tag = [0u8; 32];
        hex::decode_to_slice(&receipt.binding_tag, &mut tag).is_ok()
            && self
                .binding_mac(stable_secret, provider, credential, payload)
                .is_some_and(|mac| mac.verify_slice(&tag).is_ok())
    }
}

#[cfg(all(test, feature = "line-sap-aicore-chat"))]
#[test]
fn sap_aicore_response_receipts_route_identity_rejects_replacement() {
    let secret = b"only a synthetic signing secret";
    let mut payload = super::super::tests::payload();
    payload.credential_id = Some("credential".into());
    let pin = RoutePin {
        public_model: "public-model".into(),
        source_key_ref: "key".into(),
        platform_access_ref: Some("platform-access".into()),
    };
    let tag = pin
        .capture_binding(secret, "account", "credential", &payload)
        .unwrap();
    let receipt = Receipt {
        id: String::new(),
        owner_tag: "a".repeat(64),
        binding_tag: tag.clone(),
        provider_ref: "account".into(),
        credential_ref: "credential".into(),
        upstream_id: None,
        phase: "submitting".into(),
        revision: 0,
        input_tokens: None,
        output_tokens: None,
        settled: 0,
    };
    let matches =
        |current: &RoutePin, provider: &str, credential: &str, payload: &ProviderAccountPayload| {
            pin.matches_binding(current, &receipt, secret, provider, credential, payload)
        };
    assert!(matches(&pin, "account", "credential", &payload));
    let mut another = RoutePin {
        public_model: "different-alias".into(),
        source_key_ref: pin.source_key_ref.clone(),
        platform_access_ref: pin.platform_access_ref.clone(),
    };
    assert_eq!(
        another.capture_binding(secret, "account", "credential", &payload),
        Some(tag)
    );
    assert!(!matches(&another, "account", "credential", &payload));
    another.public_model = pin.public_model.clone();
    another.source_key_ref = "other-key".into();
    assert!(!matches(&another, "account", "credential", &payload));
    assert!(!matches(&pin, "other-account", "credential", &payload));
    assert!(!matches(&pin, "account", "other-credential", &payload));
    let mut changed = payload.clone();
    changed
        .headers
        .insert("AI-Resource-Group".into(), "other-group".into());
    assert!(!matches(&pin, "account", "credential", &changed));
    changed = payload.clone();
    changed.api_key = format!(" {}", changed.api_key);
    assert!(!matches(&pin, "account", "credential", &changed));
    changed = payload.clone();
    changed.base_url = changed.base_url.replace("deployment1", "deployment2");
    assert!(!matches(&pin, "account", "credential", &changed));
    assert!(pin
        .capture_binding(b"", "account", "credential", &payload)
        .is_none());
}
