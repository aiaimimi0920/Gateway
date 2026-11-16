//! Bounded model structure is not permission to use an extended product identity.
use crate::{credential_test_policy::exact_model, routing::candidate::ProviderAccountPayload};

pub(crate) const MAX_MODEL_BYTES: usize = 2048;

pub(crate) fn valid_structure(model: &str) -> bool {
    !model.is_empty()
        && model.len() <= MAX_MODEL_BYTES
        && model.trim() == model
        && !model.contains(['*', '?'])
        && !model.chars().any(char::is_control)
}

pub(crate) fn valid_for_payload(model: &str, payload: &ProviderAccountPayload) -> bool {
    if crate::protocol::nous_portal::business::owns_payload(payload) {
        return crate::protocol::nous_portal::atom(model, 512) && !model.contains(['*', '?']);
    }
    // Express uses a narrower URI resource segment even for short model identities.
    if crate::protocol::vertex_express::owns_payload(payload) {
        return crate::protocol::vertex_express::valid_model(model);
    }
    exact_model(model)
        || (payload.canonical_adapter() == "bedrock_converse_compatible"
            && crate::upstream::bedrock_auth::validate_model_id(model).is_ok())
}
