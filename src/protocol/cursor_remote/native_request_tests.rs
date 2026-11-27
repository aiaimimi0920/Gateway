use super::*;
use crate::protocol::{canonical::ProtocolFamily, cohere, gemini_api};
use serde_json::{json, Value};

fn gemini(body: Value) -> Result<ingress::Execution, crate::error::GatewayError> {
    ingress::parse(&gemini_api::normalize_generate_content(
        body,
        Some("cursor-personal".into()),
        false,
    )?)
}
fn tool_body(parameters: Value) -> Value {
    json!({"systemInstruction":{"parts":[{"text":"system "},{"text":"retained"}]},
        "contents":[{"role":"user","parts":[{"text":"hello "},{"text":"world"}]}],
        "tools":[{"functionDeclarations":[{"name":"echo","description":"fixture","parameters":parameters}]}]})
}
fn schema() -> Value {
    json!({"type":"OBJECT","properties":{
        "type":{"type":"STRING","enum":["OBJECT","STRING"]},
        "values":{"type":"ARRAY","items":{"type":"INTEGER"}}},"required":["type"]})
}

#[test]
fn cursor_remote_gemini_projects_only_schema_nodes_and_retains_instructions() {
    let ingress::Execution::Tools(request) = gemini(tool_body(schema())).unwrap() else {
        panic!()
    };
    assert_eq!(request.family, ProtocolFamily::GeminiGenerateContent);
    assert!(request.prompt.contains("system retained"));
    assert!(request.prompt.contains("hello world"));
    let params = &request.tools[0].wire["parameters"];
    assert_eq!(params["properties"]["type"]["type"], "string");
    assert_eq!(
        params["properties"]["type"]["enum"],
        json!(["OBJECT", "STRING"])
    );
    assert_eq!(params["properties"]["values"]["items"]["type"], "integer");
    assert_eq!(params["required"], json!(["type"]));
    let mut zero = tool_body(schema());
    zero["tools"][0]["functionDeclarations"][0]
        .as_object_mut()
        .unwrap()
        .remove("parameters");
    assert!(gemini(zero).is_ok());
}

#[test]
fn cursor_remote_gemini_schema_and_unmapped_fields_fail_closed() {
    for extra in [
        json!({"nullable":true}),
        json!({"propertyOrdering":["type"]}),
        json!({"unknown":true}),
        json!({"type":"object"}),
        json!({"type":"TYPE_UNSPECIFIED"}),
        json!({"required":[1]}),
    ] {
        let mut s = schema();
        s.as_object_mut()
            .unwrap()
            .extend(extra.as_object().unwrap().clone());
        assert!(gemini(tool_body(s)).is_err());
    }
    for nested in [
        json!({"items":[{"propertyOrdering":[]}]}),
        json!({"additionalItems":{"propertyOrdering":[]}}),
        json!({"dependencies":{"x":{"propertyOrdering":[]}}}),
        json!({"$defs":{"x":{"propertyOrdering":[]}}}),
    ] {
        let mut body = tool_body(schema());
        let declaration = &mut body["tools"][0]["functionDeclarations"][0];
        declaration.as_object_mut().unwrap().remove("parameters");
        let mut s = json!({"type":"object"});
        s.as_object_mut()
            .unwrap()
            .extend(nested.as_object().unwrap().clone());
        declaration["parametersJsonSchema"] = s;
        assert!(gemini(body).is_err());
    }
    for part in [
        json!({"text":"hello","thought":true}),
        json!({"text":"hello","thoughtSignature":"fake"}),
        json!({"fileData":{"mimeType":"image/png","fileUri":"https://example.test/image.png"}}),
        json!({"inlineData":{"mimeType":"image/gif","data":"AA=="}}),
    ] {
        let mut body = tool_body(schema());
        body["contents"][0]["parts"] = json!([part]);
        assert!(gemini(body).is_err());
    }
}

#[test]
fn cursor_remote_gemini_argument_objects_have_stable_echo_and_exact_result() {
    let mut body = tool_body(schema());
    body["contents"].as_array_mut().unwrap().extend([
        json!({"role":"model","parts":[{"text":"prelude"},
            {"functionCall":{"id":"one","name":"echo","args":{"z":1,"a":{"z":2,"a":3}}}}]}),
        json!({"role":"user","parts":[{"functionResponse":{"id":"one","name":"echo",
            "response":{"result":" \r\n中文🦀","error":false}}}]}),
    ]);
    let ingress::Execution::Tools(request) = gemini(body.clone()).unwrap() else {
        panic!()
    };
    assert_eq!(
        request.history[2]["arguments"],
        r#"{"a":{"a":3,"z":2},"z":1}"#
    );
    assert_eq!(
        request.output.unwrap().text,
        "{\"error\":false,\"result\":\" \\r\\n中文🦀\"}"
    );
    for (field, value) in [
        ("id", json!("other")),
        ("name", json!("other")),
        ("response", json!("string")),
        ("willContinue", json!(true)),
        ("scheduling", json!("SILENT")),
    ] {
        let mut changed = body.clone();
        changed["contents"][2]["parts"][0]["functionResponse"][field] = value;
        assert!(gemini(changed).is_err(), "{field}");
    }
    let mut missing_id = body.clone();
    missing_id["contents"][1]["parts"][1]["functionCall"]
        .as_object_mut()
        .unwrap()
        .remove("id");
    assert!(gemini(missing_id).is_err());
}

fn cohere(body: Value) -> Result<ingress::Execution, crate::error::GatewayError> {
    ingress::parse(&cohere::normalize_chat_v2(body)?)
}
fn cohere_body() -> Value {
    json!({"model":"cursor-personal","messages":[{"role":"user","content":"hello"},
        {"role":"assistant","tool_calls":[{"id":"one","type":"function","function":{"name":"echo","arguments":"{}"}}]},
        {"role":"tool","tool_call_id":"one","content":"{ \"z\":1, \"a\":\"中文\" }"}],
        "tools":[{"type":"function","function":{"name":"echo","parameters":{"type":"object"}}}]})
}

#[test]
fn cursor_remote_cohere_preserves_json_string_result_and_text_blocks() {
    let ingress::Execution::Tools(request) = cohere(cohere_body()).unwrap() else {
        panic!()
    };
    assert_eq!(request.family, ProtocolFamily::CohereChat);
    assert_eq!(request.output.unwrap().text, "{ \"z\":1, \"a\":\"中文\" }");
    let mut body = cohere_body();
    body["messages"][2]["content"] =
        json!([{"type":"text","text":" \r\n"},{"type":"text","text":"中文🦀"}]);
    let ingress::Execution::Tools(request) = cohere(body).unwrap() else {
        panic!()
    };
    assert_eq!(request.output.unwrap().text, " \r\n中文🦀");
    for result in [
        json!("not JSON"),
        json!("null"),
        json!("[]"),
        json!([{"type":"document","document":{"data":"x"}}]),
    ] {
        let mut body = cohere_body();
        body["messages"][2]["content"] = result;
        assert!(cohere(body).is_err());
    }
    for field in ["tool_plan", "reasoning_content"] {
        let mut body = cohere_body();
        body["messages"][1][field] = json!("unpublished");
        assert!(cohere(body).is_err());
    }
}

#[test]
fn cursor_remote_native_scope_does_not_enable_live_or_change_other_providers() {
    let mut req = gemini_api::normalize_generate_content(
        json!({"contents":[{"parts":[{"text":"hello"}]}]}),
        Some("cursor-personal".into()),
        false,
    )
    .unwrap();
    assert_eq!(ingress::scope(&req).unwrap(), "gemini.generateContent");
    req.protocol_family = ProtocolFamily::GeminiLive;
    assert!(ingress::parse(&req).is_err());
    assert!(ingress::auth_path(&req).is_none());
    req.protocol_family = ProtocolFamily::GeminiGenerateContent;
    req.requested_model = Some("ordinary-gemini".into());
    assert!(ingress::auth_path(&req).is_none());
}
