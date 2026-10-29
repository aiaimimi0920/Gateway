//! Compile additive flow selections once, retaining the legacy payload contract.
use crate::{
    protocol::{
        call_flow::{prepare, FingerprintAdapter, ProviderCallFlow, SpecialAdapter},
        request_profile::RequestClientProfile,
    },
    routing::candidate::ProviderAccountPayload,
};

pub(super) fn configure(
    payload: &mut ProviderAccountPayload,
    flow: Option<ProviderCallFlow>,
    profile: Option<RequestClientProfile>,
    credential_profile: Option<RequestClientProfile>,
    basispoints: bool,
    basispoints_override: Option<bool>,
) -> anyhow::Result<()> {
    payload.request_client_profile = credential_profile.or(profile);
    payload.call_flow = flow.map(|mut flow| {
        if let Some(profile) = credential_profile {
            flow.fingerprint_adapter = profile.into();
        }
        if let Some(enabled) = basispoints_override {
            if enabled {
                flow.special_adapter = SpecialAdapter::Basispoints;
                flow.call_adapter = crate::protocol::call_flow::CallAdapter::Auto;
                flow.fingerprint_adapter = FingerprintAdapter::Session;
            } else if flow.special_adapter == SpecialAdapter::Basispoints {
                flow.special_adapter = SpecialAdapter::None;
                flow.fingerprint_adapter =
                    credential_profile.or(profile).unwrap_or_default().into();
            }
        }
        flow
    });
    crate::protocol::chatgpt::basispoints::configure(
        payload,
        basispoints_override.unwrap_or(
            basispoints || flow.is_some_and(|f| f.special_adapter == SpecialAdapter::Basispoints),
        ),
    )?;
    // No-flow payloads stay byte-for-byte legacy. New flow blocks remain in the
    // payload as typed policy, not hidden in headers or extra_body sent upstream.
    if payload.call_flow.is_some() {
        *payload = prepare(payload)?.payload.into_owned();
    }
    Ok(())
}
