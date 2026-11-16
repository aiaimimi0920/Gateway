//! Directory declarations cannot override a candidate's allowed or forced wire family.
use super::*;
use crate::protocol::commandcode as product;

#[test]
fn commandcode_native_selection_obeys_allowed_and_forced_families() {
    let catalog = serde_json::from_value(json!({
        "sourceUrl":product::SOURCE,"checkedAt":"2026-11-14T00:00:00Z",
        "models":["gpt-4o"],"supportedEndpoints":{"gpt-4o":["/messages","/responses"]}
    }))
    .unwrap();
    for (endpoint, family) in [
        (EndpointKind::Messages, ANTHROPIC_MESSAGES_FAMILY),
        (EndpointKind::Responses, OPENAI_RESPONSES_FAMILY),
    ] {
        let request = make_request(
            if endpoint == EndpointKind::Messages {
                ProtocolFamily::Anthropic
            } else {
                ProtocolFamily::OpenAi
            },
            endpoint,
        );
        for allowed in [
            OPENAI_CHAT_FAMILY,
            ANTHROPIC_MESSAGES_FAMILY,
            OPENAI_RESPONSES_FAMILY,
        ] {
            for forced in [None, Some(OPENAI_CHAT_FAMILY), Some(family)] {
                let mut candidate = make_candidate(OPENAI_CHAT_FAMILY);
                candidate.payload.auth_mode = Some(product::AUTH_MODE.into());
                candidate.payload.base_url = product::BASE.into();
                candidate.payload.chat_completions_path = Some("/chat/completions".into());
                candidate.supported_protocol_families = vec![allowed.into()];
                product::catalog::configure(&mut candidate.payload, &catalog);
                let selected = product::catalog::select(&mut candidate, &request, forced).unwrap();
                let expected = allowed == family && forced.is_none_or(|value| value == family);
                assert_eq!(selected, expected, "{endpoint:?} {allowed} {forced:?}");
                if expected {
                    assert_eq!(candidate.protocol_family, family);
                    assert!(candidate.payload.chat_completions_path.is_none());
                    product::validate(&candidate.payload, "openai_compatible").unwrap();
                } else {
                    assert_eq!(candidate.protocol_family, OPENAI_CHAT_FAMILY);
                    assert!(
                        candidate.payload.messages_path.is_none()
                            && candidate.payload.responses_path.is_none()
                    );
                }
            }
        }
    }
}
