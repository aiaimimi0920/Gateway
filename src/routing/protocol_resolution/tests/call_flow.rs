//! Hosted-shaped candidates cannot bypass flow preparation at protocol admission.
use super::*;

#[test]
fn call_flow_protocol_admission_rejects_invalid_product_executor() {
    let mut candidate = make_candidate(OPENAI_CHAT_FAMILY);
    candidate.adapter = "freebuff_compatible".into();
    candidate.payload.adapter = candidate.adapter.clone();
    candidate.supported_protocol_families = vec![OPENAI_CHAT_FAMILY.into()];
    candidate.payload.call_flow = Some(
        serde_json::from_value(json!({
            "call_adapter":"anthropic","special_adapter":"none"
        }))
        .unwrap(),
    );
    let req = make_request(ProtocolFamily::OpenAi, EndpointKind::Responses);
    assert!(finalize_candidate_protocol_family(&mut candidate, &req).is_none());
}

#[test]
fn call_flow_protocol_admission_synchronizes_payload_without_expanding_policy() {
    let mut candidate = make_candidate(OPENAI_RESPONSES_FAMILY);
    candidate.payload.call_flow = Some(
        serde_json::from_value(json!({
            "call_adapter":"anthropic","fingerprint_adapter":"claude_cli","special_adapter":"none"
        }))
        .unwrap(),
    );
    candidate.supported_protocol_families = vec![ANTHROPIC_MESSAGES_FAMILY.into()];
    let req = make_request(ProtocolFamily::OpenAi, EndpointKind::Responses);
    assert!(finalize_candidate_protocol_family(&mut candidate, &req).is_some());
    assert_eq!(candidate.adapter, "anthropic_compatible");
    assert_eq!(candidate.payload.adapter, candidate.adapter);
    assert_eq!(candidate.protocol_family, ANTHROPIC_MESSAGES_FAMILY);
    candidate.supported_protocol_families = vec![OPENAI_RESPONSES_FAMILY.into()];
    assert!(finalize_candidate_protocol_family(&mut candidate, &req).is_none());
}
