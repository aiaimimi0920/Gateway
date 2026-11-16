//! Decision routing is product-scoped and must not inherit generic OpenAI or fallback capabilities.
use super::*;
use crate::protocol::{commandcode as product, registry::SYSTEMONE_FAMILY};

#[test]
fn commandcode_systemone_routing_requires_product_catalog_and_allowed_family() {
    let request = product::systemone::normalize(json!({
        "model":"gpt-4o","state":"fixture","questions":{"q":{"type":"noul"}}
    }))
    .unwrap();
    assert_eq!(
        requested_wire_protocol_family(&request).as_deref(),
        Some(SYSTEMONE_FAMILY)
    );
    assert_eq!(
        serde_json::to_value(request.endpoint_kind).unwrap(),
        "systemone"
    );
    assert_eq!(
        serde_json::to_value(request.protocol_family).unwrap(),
        "systemone"
    );
    for owns in [false, true] {
        for declared in [false, true] {
            for allowed in [OPENAI_CHAT_FAMILY, SYSTEMONE_FAMILY] {
                for forced in [None, Some(OPENAI_CHAT_FAMILY), Some(SYSTEMONE_FAMILY)] {
                    let mut candidate = make_candidate(SYSTEMONE_FAMILY);
                    candidate.supported_protocol_families = vec![allowed.into()];
                    if owns {
                        candidate.payload.auth_mode = Some(product::AUTH_MODE.into());
                        candidate.payload.base_url = product::BASE.into();
                        if declared {
                            let catalog = serde_json::from_value(json!({
                                "sourceUrl":product::SOURCE,"checkedAt":"2026-11-14T00:00:00Z",
                                "models":["gpt-4o"],"supportedEndpoints":{"gpt-4o":["/systemone"]}
                            }))
                            .unwrap();
                            product::catalog::configure(&mut candidate.payload, &catalog);
                        }
                    }
                    let result = finalize_candidate_protocol_family_with_override(
                        &mut candidate,
                        &request,
                        forced,
                    );
                    let expected = owns
                        && declared
                        && allowed == SYSTEMONE_FAMILY
                        && forced.is_none_or(|value| value == SYSTEMONE_FAMILY);
                    assert_eq!(
                        result,
                        expected.then_some(true),
                        "{owns} {declared} {allowed} {forced:?}"
                    );
                }
            }
        }
    }
}
