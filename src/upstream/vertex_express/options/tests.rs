use super::*;
use crate::protocol::{gemini::api::pack_request, openai::normalize_chat_completions};

#[test]
fn vertex_express_preview_options_omitted_never_enables_or_rewrites_existing_body() {
    for config in [
        None,
        Some(json!(null)),
        Some(json!({"functionCallingConfig":null})),
        Some(json!({"functionCallingConfig":{"mode":"NONE"}})),
    ] {
        let mut raw =
            json!({"messages":[{"role":"user","content":"test"}],"tool_choice":"required"});
        if let Some(config) = config {
            raw["toolConfig"] = config;
        }
        let req = normalize_chat_completions(raw).unwrap();
        let mut body = pack_request(&req, "model", true);
        let before = body.clone();
        apply(crate::upstream::vertex_product::EXPRESS, &req, &mut body).unwrap();
        assert_eq!(body, before);
        assert!(body
            .pointer("/toolConfig/functionCallingConfig/streamFunctionCallArguments")
            .is_none());
    }
}

#[test]
fn vertex_express_preview_options_keep_canonical_choice_and_explicit_false() {
    for (choice, expected) in [
        (json!("auto"), json!({"mode":"AUTO"})),
        (json!("none"), json!({"mode":"NONE"})),
        (json!("required"), json!({"mode":"ANY"})),
        (
            json!({"type":"function","function":{"name":"exact"}}),
            json!({"mode":"ANY","allowedFunctionNames":["exact"]}),
        ),
    ] {
        for enabled in [false, true] {
            let raw = json!({"messages":[{"role":"user","content":"test"}],"tool_choice":choice,"toolConfig":{"functionCallingConfig":{"mode":"ANY","allowedFunctionNames":["stale"],"streamFunctionCallArguments":enabled}}});
            let req = normalize_chat_completions(raw).unwrap();
            let mut body = pack_request(&req, "model", true);
            apply(crate::upstream::vertex_product::EXPRESS, &req, &mut body).unwrap();
            let mut expected = expected.clone();
            expected["streamFunctionCallArguments"] = json!(enabled);
            assert_eq!(body["toolConfig"]["functionCallingConfig"], expected);
        }
    }
}

#[test]
fn vertex_express_preview_options_read_lossless_raw_without_reintroducing_other_fields() {
    let mut req=normalize_chat_completions(json!({"messages":[{"role":"user","content":"test"}],"tool_choice":"auto","toolConfig":{"unowned":"not-forwarded","functionCallingConfig":{"mode":"NONE","streamFunctionCallArguments":true}}})).unwrap();
    req.extra.clear();
    let mut body = pack_request(&req, "model", true);
    apply(crate::upstream::vertex_product::EXPRESS, &req, &mut body).unwrap();
    assert_eq!(
        body["toolConfig"],
        json!({"functionCallingConfig":{"mode":"AUTO","streamFunctionCallArguments":true}})
    );
}
