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
