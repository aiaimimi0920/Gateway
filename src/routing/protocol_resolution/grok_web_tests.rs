//! Only the typed current Web owner can bridge Responses ingress onto its Chat event wire.
use super::{
    request_compatible_wire_protocol_families, EndpointKind, ProviderAccountPayload,
    OPENAI_CHAT_FAMILY,
};
#[test]
fn grok_web_request_compatibility_does_not_widen_legacy_or_messages() {
    let mut payload: ProviderAccountPayload = serde_json::from_value(serde_json::json!({
        "adapter":"grok_compatible","base_url":"https://grok.com","api_key":"synthetic",
        "call_flow":{"special_adapter":"grok_web","fingerprint_adapter":"session"}
    }))
    .unwrap();
    for typed in [true, false] {
        if !typed {
            payload.call_flow = None;
        }
        for (endpoint, expected) in [
            (EndpointKind::ChatCompletions, true),
            (EndpointKind::Responses, typed),
            (EndpointKind::Messages, false),
            (EndpointKind::Completions, false),
        ] {
            let actual =
                request_compatible_wire_protocol_families(&payload, endpoint, OPENAI_CHAT_FAMILY);
            assert_eq!(
                actual,
                if expected {
                    vec![OPENAI_CHAT_FAMILY.to_owned()]
                } else {
                    vec![]
                },
                "typed={typed} endpoint={endpoint:?}"
            );
        }
    }
}
