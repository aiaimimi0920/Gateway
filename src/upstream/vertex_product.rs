//! Shared REST algorithms receive an explicit product identity; no credentials or quota live here.
use crate::protocol::vertex_turn::VertexTurnSource;
#[derive(Clone, Copy)]
pub(crate) struct Contract {
    pub label: &'static str,
    pub profile: &'static str,
    pub code_prefix: &'static str,
    pub source: VertexTurnSource,
}
pub(crate) const EXPRESS: Contract = Contract {
    label: "Vertex Express",
    profile: "vertex_express_api",
    code_prefix: "vertex_express",
    source: VertexTurnSource::Express,
};
pub(crate) const OAUTH: Contract = Contract {
    label: "Full Vertex",
    profile: "vertex_user_oauth",
    code_prefix: "vertex_oauth",
    source: VertexTurnSource::UserOAuth,
};
pub(crate) const DIM: Contract = Contract {
    label: "DIM Gemini",
    profile: "dim_user_oauth_gemini",
    code_prefix: "dim_oauth",
    source: VertexTurnSource::DimUserOAuth,
};
/// Share only Gemini wire algorithms; authorization and the final sender remain product-owned.
pub(crate) fn for_payload(
    payload: &crate::routing::candidate::ProviderAccountPayload,
) -> Option<Contract> {
    if crate::protocol::vertex_express::owns_payload(payload) {
        Some(EXPRESS)
    } else if crate::protocol::vertex_oauth::business::owns_payload(payload) {
        Some(OAUTH)
    } else if crate::protocol::dim_oauth::business::owns_payload(payload)
        && payload.canonical_adapter() == "gemini_api_compatible"
    {
        Some(DIM)
    } else {
        None
    }
}
