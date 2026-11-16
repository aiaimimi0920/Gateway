//! Packing and content conversion contracts independent of network/session state.
use super::tests::{make_payload, make_request};
use super::*;
use crate::protocol::canonical::EndpointKind;

#[test]
fn base_url_rewrite_requires_the_exact_codebuff_authority() {
    for url in [
        "https://codebuff.com.evil.test/api",
        "https://codebuff.com@evil.test/api",
        "https://codebuff.com:password@evil.test/api",
        "https://codebuff.com:/api",
        "https://codebuff.com:abc/api",
    ] {
        assert_eq!(normalize_base_url(url), url);
    }
    for (url, expected) in [
        (
            "https://codebuff.com:8443/api",
            "https://www.codebuff.com:8443/api",
        ),
        (
            "http://codebuff.com:80/api?x=1#frag",
            "http://www.codebuff.com:80/api?x=1#frag",
        ),
        (
            "https://codebuff.com?x=1#frag",
            "https://www.codebuff.com?x=1#frag",
        ),
        ("https://codebuff.com#frag", "https://www.codebuff.com#frag"),
    ] {
        assert_eq!(normalize_base_url(url), expected);
    }
}

#[test]
fn flattening_preserves_supported_parts_and_leaves_multimodal_messages_intact() {
    let parts = json!([
        {"type":"text","text":"a"}, {"type":"json","value":{"x":1}},
        {"type":"input_text","value":"b"}, {"type":"output_text","text":"c"},
        {"type":"custom","text":"d"}, {"type":"custom","value":false}
    ]);
    assert_eq!(
        flatten_freebuff_content_parts(parts.as_array().unwrap()).as_deref(),
        Some("a\n{\"x\":1}\nb\nc\nd\nfalse")
    );
    assert_eq!(flatten_freebuff_content_parts(&[]).as_deref(), Some(""));
    for unsupported in [
        json!({"type":"image_url","image_url":{"url":"https://example.test/image"}}),
        json!({"type":"input_image","image_url":"data:test"}),
        json!({"type":"text","text":123}),
        json!(false),
    ] {
        let mut body = json!({"messages":[{"role":"user","content":[{"type":"text","text":"keep"},unsupported]}]});
        let before = body.clone();
        sanitize_freebuff_messages(&mut body);
        assert_eq!(body, before);
    }
}

#[test]
fn packing_preserves_custom_metadata_and_overwrites_owned_runtime_fields() {
    let mut req = make_request();
    req.extra.insert(
        "codebuff_metadata".into(),
        json!({"custom":"keep","run_id":"old","cost_mode":"old","client_id":"old"}),
    );
    let body = build_chat_request_body(&req, "model", true, "run", " paid ", Some(" instance "));
    let metadata = &body["codebuff_metadata"];
    assert_eq!(metadata["custom"], "keep");
    assert_eq!(metadata["run_id"], "run");
    assert_eq!(metadata["cost_mode"], "paid");
    assert_eq!(metadata["freebuff_instance_id"], "instance");
    let client_id = metadata["client_id"].as_str().unwrap();
    assert_eq!(client_id.len(), 13);
    assert!(client_id
        .bytes()
        .all(|byte| byte.is_ascii_digit() || byte.is_ascii_lowercase()));
    req.extra
        .insert("codebuff_metadata".into(), json!("opaque"));
    let body = build_chat_request_body(&req, "model", false, "run", "free", None);
    assert_eq!(body["codebuff_metadata"]["run_id"], "run");
    assert_eq!(body["codebuff_metadata"]["cost_mode"], "free");
    assert!(body["codebuff_metadata"].is_object());
}

#[test]
fn cli_claim_metadata_keeps_multi_session_and_surface_with_the_original_instance() {
    let mut req = make_request();
    req.extra.insert(
        "codebuff_metadata".into(),
        json!({"freebuff_multi_session":"caller", "surface":"desktop", "freebuff_instance_id":"foreign"}),
    );
    let body = build_chat_request_body(
        &req,
        "fbm1.current",
        false,
        "real-run",
        "free",
        Some("cli:8759f8e9-6046-4350-9aef-bcd7309a02ea"),
    );
    let metadata = &body["codebuff_metadata"];
    assert_eq!(metadata["freebuff_multi_session"], "1");
    assert_eq!(metadata["surface"], "cli");
    assert_eq!(
        metadata["freebuff_instance_id"],
        "cli:8759f8e9-6046-4350-9aef-bcd7309a02ea"
    );
    let legacy = build_chat_request_body(
        &req,
        "legacy",
        false,
        "run",
        "free",
        Some("legacy-instance"),
    );
    assert!(legacy["codebuff_metadata"]
        .get("freebuff_multi_session")
        .is_none());
    assert!(legacy["codebuff_metadata"].get("surface").is_none());
}

#[test]
fn public_plan_preserves_text_endpoint_mapping_and_explicit_path() {
    let mut payload = make_payload();
    payload.base_url = " https://example.test/root/ ".into();
    payload.chat_completions_path = Some("/custom".into());
    for endpoint in [
        EndpointKind::ChatCompletions,
        EndpointKind::Messages,
        EndpointKind::Responses,
        EndpointKind::Completions,
    ] {
        let mut req = make_request();
        req.endpoint_kind = endpoint;
        let plan = build_request_plan(&payload, &req, "model", false).unwrap();
        assert_eq!(plan.method, Method::POST);
        assert_eq!(plan.url, "https://example.test/root/custom");
        assert!(plan.query.is_empty());
        assert_eq!(plan.response_kind, EndpointKind::ChatCompletions);
        assert_eq!(plan.body.unwrap()["stream"], false);
    }
}
