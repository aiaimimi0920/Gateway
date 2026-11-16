use super::*;

#[test]
fn pack_response_openai_chat_completions() {
    let req = make_req(ProtocolFamily::OpenAi, EndpointKind::ChatCompletions);
    let resp = make_resp();
    let json = pack_response(&req, &resp);
    assert_eq!(json["object"], "chat.completion");
    assert_eq!(json["choices"][0]["message"]["content"], "Hello!");
    assert_eq!(json["model"], "gpt-4o");
}

#[test]
fn pack_response_anthropic_messages() {
    let req = make_req(ProtocolFamily::Anthropic, EndpointKind::Messages);
    let resp = make_resp();
    let json = pack_response(&req, &resp);
    assert_eq!(json["type"], "message");
    assert_eq!(json["content"][0]["text"], "Hello!");
}

#[test]
fn pack_response_legacy_completions() {
    let req = make_req(ProtocolFamily::OpenAi, EndpointKind::Completions);
    let resp = make_resp();
    let json = pack_response(&req, &resp);
    assert_eq!(json["object"], "text_completion");
    assert_eq!(json["choices"][0]["text"], "Hello!");
    assert_eq!(json["model"], "gpt-4o");
}

#[test]
fn pack_response_prefers_requested_model_over_upstream_model() {
    let req = make_req(ProtocolFamily::Anthropic, EndpointKind::Messages);
    let mut resp = make_resp();
    resp.model = "astron-code-latest".to_string();
    let json = pack_response(&req, &resp);
    assert_eq!(json["model"], "gpt-4o");
}

#[test]
fn response_sources_buffered_pack_and_unpack_preserve_list_and_text() {
    let mut value = serde_json::to_value(make_resp()).unwrap();
    let sources =
        serde_json::json!([{"name":"标题", "url":"https://example.org/a", "snippet":"摘要"}]);
    value["sources"] = sources.clone();
    value["usage"] = serde_json::Value::Null;
    let resp: CanonicalRelayResponse = serde_json::from_value(value).unwrap();
    for endpoint in [EndpointKind::ChatCompletions, EndpointKind::Responses] {
        let wire = pack_response(&make_req(ProtocolFamily::OpenAi, endpoint), &resp);
        assert_eq!(wire["gateway_sources"], sources);
        assert!(wire.get("usage").is_none());
        let canonical = if endpoint == EndpointKind::Responses {
            crate::protocol::responses::unpack_responses_response(&wire)
        } else {
            crate::protocol::openai::unpack_openai_response(&wire)
        }
        .unwrap();
        assert_eq!(canonical.text, "Hello!");
        assert_eq!(serde_json::to_value(canonical).unwrap()["sources"], sources);
    }
}

#[test]
fn response_sources_empty_pack_leaves_both_old_formats_unchanged() {
    for endpoint in [EndpointKind::ChatCompletions, EndpointKind::Responses] {
        let wire = pack_response(&make_req(ProtocolFamily::OpenAi, endpoint), &make_resp());
        assert!(wire.get("gateway_sources").is_none());
        assert!(wire.to_string().contains("Hello!"));
    }
}
