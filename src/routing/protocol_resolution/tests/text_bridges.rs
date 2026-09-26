use super::*;

#[test]
fn openai_compatible_completions_falls_back_to_chat_family_without_legacy_path() {
    let payload = ProviderAccountPayload {
        chat_completions_path: Some("/v2/chat/completions".to_string()),
        ..make_candidate("openai").payload
    };
    let families =
        request_compatible_wire_protocol_families(&payload, EndpointKind::Completions, "openai");
    assert_eq!(families, vec![OPENAI_CHAT_FAMILY.to_string()]);
}

#[test]
fn openai_compatible_completions_uses_legacy_family_when_path_is_present() {
    let payload = ProviderAccountPayload {
        chat_completions_path: Some("/v2/chat/completions".to_string()),
        completions_path: Some("/v1/completions".to_string()),
        ..make_candidate("openai").payload
    };
    let families =
        request_compatible_wire_protocol_families(&payload, EndpointKind::Completions, "openai");
    assert_eq!(families, vec![OPENAI_LEGACY_COMPLETIONS_FAMILY.to_string()]);
}

#[test]
fn openai_compatible_responses_fall_back_to_chat_family_when_only_chat_path_is_present() {
    let payload = ProviderAccountPayload {
        chat_completions_path: Some("/v1/chat/completions".to_string()),
        ..make_candidate("openai").payload
    };
    let families =
        request_compatible_wire_protocol_families(&payload, EndpointKind::Responses, "openai");
    assert_eq!(families, vec![OPENAI_CHAT_FAMILY.to_string()]);
}

#[test]
fn anthropic_compatible_supports_cross_family_text_ingress() {
    let payload = ProviderAccountPayload {
        adapter: "anthropic_compatible".to_string(),
        ..make_candidate("anthropic").payload
    };
    for endpoint_kind in [
        EndpointKind::ChatCompletions,
        EndpointKind::Completions,
        EndpointKind::Messages,
        EndpointKind::Responses,
    ] {
        let families =
            request_compatible_wire_protocol_families(&payload, endpoint_kind, "anthropic");
        assert_eq!(families, vec![ANTHROPIC_MESSAGES_FAMILY.to_string()]);
    }
}

#[test]
fn freebuff_compatible_supports_cross_family_text_ingress() {
    let payload = ProviderAccountPayload {
        adapter: "freebuff_compatible".to_string(),
        ..make_candidate("freebuff").payload
    };
    for endpoint_kind in [
        EndpointKind::ChatCompletions,
        EndpointKind::Completions,
        EndpointKind::Messages,
        EndpointKind::Responses,
    ] {
        let families =
            request_compatible_wire_protocol_families(&payload, endpoint_kind, "freebuff");
        assert_eq!(families, vec![OPENAI_CHAT_FAMILY.to_string()]);
    }
}

#[test]
fn accio_compatible_supports_cross_family_text_ingress_via_responses_bridge() {
    let payload = ProviderAccountPayload {
        adapter: "accio_compatible".to_string(),
        ..make_candidate("openai_responses").payload
    };
    for endpoint_kind in [
        EndpointKind::ChatCompletions,
        EndpointKind::Completions,
        EndpointKind::Messages,
        EndpointKind::Responses,
    ] {
        let families = request_compatible_wire_protocol_families(
            &payload,
            endpoint_kind,
            OPENAI_RESPONSES_FAMILY,
        );
        assert_eq!(families, vec![OPENAI_RESPONSES_FAMILY.to_string()]);
    }
}

#[test]
fn chatgpt_web_reverse_supports_cross_family_text_ingress() {
    let payload = ProviderAccountPayload {
        adapter: "chatgpt_web_reverse_compatible".to_string(),
        ..make_candidate("openai_chat").payload
    };
    for endpoint_kind in [
        EndpointKind::ChatCompletions,
        EndpointKind::Completions,
        EndpointKind::Messages,
        EndpointKind::Responses,
    ] {
        let families =
            request_compatible_wire_protocol_families(&payload, endpoint_kind, OPENAI_CHAT_FAMILY);
        assert_eq!(families, vec![CHATGPT_WEB_CHAT_FAMILY.to_string()]);
    }
}
