//! Select the accounting contract for product-owned native Responses streams.
use crate::{
    protocol::{
        canonical::{EndpointKind, TokenUsage},
        commandcode, responses, sap_aicore, sensenova,
    },
    routing::candidate::ProviderAccountPayload,
};

type Observer = fn(&serde_json::Value) -> (Option<TokenUsage>, &'static str);

pub(super) fn observer(
    payload: &ProviderAccountPayload,
    endpoint: EndpointKind,
) -> Option<Observer> {
    if sensenova::native_responses::selected(payload, endpoint)
        || sap_aicore::native_responses::selected(payload, endpoint)
    {
        Some(responses::native_admission::observe)
    } else if endpoint == EndpointKind::Responses
        && commandcode::owns_payload(payload)
        && payload.responses_path.as_deref() == Some("/responses")
    {
        Some(commandcode::native_json::observe_responses)
    } else {
        None
    }
}
