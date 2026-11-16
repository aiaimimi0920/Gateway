//! Basic Converse generation parameters use their actual nested wire shape.
use neuro_gateway::{
    protocol::{anthropic, bedrock_converse, canonical::CanonicalRelayRequest, openai, responses},
    routing::candidate::ProviderAccountPayload,
    upstream::client::UpstreamClient,
};
use serde_json::{json, Value};

fn payload() -> ProviderAccountPayload {
    serde_json::from_value(json!({"adapter":"bedrock_converse_compatible",
        "base_url":"https://bedrock-runtime.us-east-1.amazonaws.com","api_key":"synthetic-only",
        "auth_mode":"bedrock_bearer","extra_body":{"awsRegion":"us-east-1"}}))
    .unwrap()
}
fn chat(parameters: Value) -> CanonicalRelayRequest {
    let mut body = json!({"model":"fixture","messages":[{"role":"user","content":"hi"}]});
    body.as_object_mut()
        .unwrap()
        .extend(parameters.as_object().unwrap().clone());
    openai::normalize_chat_completions(body).unwrap()
}
fn native(config: Value) -> Result<CanonicalRelayRequest, neuro_gateway::error::GatewayError> {
    bedrock_converse::normalize_converse(
        json!({"messages":[{"role":"user","content":[{"text":"hi"}]}],
        "inferenceConfig":config}),
        Some("fixture".into()),
        false,
    )
}
fn plan(
    req: &CanonicalRelayRequest,
    stream: bool,
) -> Result<Value, neuro_gateway::error::GatewayError> {
    UpstreamClient::build_request_plan(&payload(), req, "fixture", stream)
        .map(|plan| plan.body.unwrap())
}

#[test]
fn chat_responses_and_messages_map_basic_fields_into_inference_config() {
    let requests = [
        chat(json!({"max_completion_tokens":17,"temperature":0,"top_p":0.75,"stop":"终"})),
        responses::normalize_responses(json!({"model":"fixture","input":"hi",
            "max_output_tokens":17,"temperature":0,"top_p":0.75}))
        .unwrap(),
        anthropic::normalize_messages(
            json!({"model":"fixture","messages":[{"role":"user","content":"hi"}],
            "max_tokens":17,"temperature":0,"top_p":0.75,"stop_sequences":["终"]}),
        )
        .unwrap(),
    ];
    for (index, req) in requests.iter().enumerate() {
        for stream in [false, true] {
            let body = plan(req, stream).unwrap();
            assert_eq!(body["inferenceConfig"]["maxTokens"], 17);
            assert_eq!(body["inferenceConfig"]["temperature"], 0);
            assert_eq!(body["inferenceConfig"]["topP"], 0.75);
            if index != 1 {
                assert_eq!(body["inferenceConfig"]["stopSequences"], json!(["终"]));
            }
            for key in [
                "max_tokens",
                "max_completion_tokens",
                "max_output_tokens",
                "temperature",
                "top_p",
                "stop",
                "stop_sequences",
            ] {
                assert!(body.get(key).is_none(), "Wrong top-level wire field: {key}");
            }
        }
    }
}

#[test]
fn native_roundtrip_preserves_extensions_and_existing_reverse_bridge() {
    let config =
        json!({"maxTokens":17,"temperature":0,"topP":0.75,"stopSequences":["终"],"future":{"x":1}});
    let req = native(config.clone()).unwrap();
    for stream in [false, true] {
        assert_eq!(plan(&req, stream).unwrap()["inferenceConfig"], config);
        let chat = openai::pack_openai(&req, "fixture", stream);
        assert_eq!(chat["max_tokens"], 17);
        assert_eq!(chat["temperature"], 0);
        assert_eq!(chat["top_p"], 0.75);
        assert_eq!(chat["stop"], json!(["终"]));
        let messages = anthropic::pack_anthropic(&req, "fixture", stream);
        assert_eq!(messages["max_tokens"], 17);
        assert_eq!(messages["stop_sequences"], json!(["终"]));
    }
}

#[test]
fn conflicting_aliases_or_native_values_are_rejected_not_hash_order_selected() {
    for parameters in [
        json!({"max_tokens":17,"max_completion_tokens":18}),
        json!({"max_tokens":17,"max_output_tokens":18}),
        json!({"stop":"a","stop_sequences":["b"]}),
        json!({"inferenceConfig":{"maxTokens":18},"max_tokens":17}),
        json!({"inferenceConfig":{"temperature":0.5},"temperature":0}),
    ] {
        let req = chat(parameters);
        for stream in [false, true] {
            let error = plan(&req, stream)
                .expect_err("Conflicting limits must not be arbitrarily selected");
            assert_eq!(error.code.as_deref(), Some("bedrock_generation_parameters"));
        }
    }
}

#[test]
fn invalid_native_or_bridged_parameter_values_fail_without_echoing_values() {
    for (native_key, alias, values) in [
        (
            "maxTokens",
            "max_tokens",
            vec![
                json!(0),
                json!(-1),
                json!(1.5),
                json!("private-marker"),
                Value::Null,
            ],
        ),
        (
            "temperature",
            "temperature",
            vec![json!(-0.1), json!(1.1), json!(true), Value::Null],
        ),
        (
            "topP",
            "top_p",
            vec![json!(-0.1), json!(1.1), json!({"private-marker":1})],
        ),
        (
            "stopSequences",
            "stop",
            vec![json!([""]), json!([1]), json!(true), Value::Null],
        ),
    ] {
        for value in values {
            let mut config = json!({});
            config[native_key] = value.clone();
            let error = native(config)
                .expect_err("Native inference config must be validated before bridging");
            assert_eq!(error.code.as_deref(), Some("bedrock_generation_parameters"));
            assert!(!error.to_string().contains("private-marker"));
            let mut parameters = json!({});
            parameters[alias] = value;
            for stream in [false, true] {
                let error = plan(&chat(parameters.clone()), stream).unwrap_err();
                assert_eq!(error.code.as_deref(), Some("bedrock_generation_parameters"));
                assert!(!error.to_string().contains("private-marker"));
            }
        }
    }
    for config in [Value::Null, json!([]), json!("private-marker")] {
        assert!(native(config).is_err());
    }
}

#[test]
fn equal_aliases_and_zero_sampling_values_preserve_exact_meaning() {
    let config = json!({"maxTokens":17,"temperature":0.0,"topP":0.0,"stopSequences":["终"]});
    let req = chat(
        json!({"inferenceConfig":config,"max_tokens":17,"temperature":0,"top_p":0,"stop":"终"}),
    );
    assert_eq!(plan(&req, false).unwrap()["inferenceConfig"], config);
    let req = chat(
        json!({"max_tokens":17,"max_output_tokens":17,"max_completion_tokens":17,
        "temperature":0,"top_p":0,"stop":"终","stop_sequences":["终"]}),
    );
    assert_eq!(
        plan(&req, false).unwrap()["inferenceConfig"],
        json!({"maxTokens":17,"temperature":0,"topP":0,"stopSequences":["终"]})
    );
    assert_eq!(
        plan(&chat(json!({"stop":[]})), false).unwrap()["inferenceConfig"]["stopSequences"],
        json!([])
    );
    let exact = vec![json!("x"); 2500];
    assert!(native(json!({"stopSequences":exact})).is_ok());
    assert!(native(json!({"stopSequences":vec![json!("x");2501]})).is_err());
}

#[test]
fn provider_defaults_are_checked_after_merge_and_cannot_replace_caller_limit() {
    let mut provider = payload();
    provider
        .extra_body
        .as_mut()
        .unwrap()
        .insert("inferenceConfig".into(), json!({"maxTokens":99}));
    for stream in [false, true] {
        let planned = UpstreamClient::build_request_plan(
            &provider,
            &chat(json!({"max_tokens":17})),
            "fixture",
            stream,
        )
        .unwrap();
        assert_eq!(planned.body.unwrap()["inferenceConfig"]["maxTokens"], 17);
        let planned = UpstreamClient::build_request_plan(
            &provider,
            &chat(json!({"temperature":0})),
            "fixture",
            stream,
        )
        .unwrap();
        // Preserve the framework's top-level defaults merge, not a new recursive merge policy.
        assert!(planned.body.unwrap()["inferenceConfig"]
            .get("maxTokens")
            .is_none());
    }
    for extra in [
        json!({"inferenceConfig":{"maxTokens":0}}),
        json!({"temperature":0.5}),
        json!({"inferenceConfig":"private-marker"}),
    ] {
        provider.extra_body = Some(serde_json::from_value(extra).unwrap());
        let error =
            UpstreamClient::build_request_plan(&provider, &chat(json!({})), "fixture", false)
                .unwrap_err();
        assert_eq!(error.code.as_deref(), Some("bedrock_generation_parameters"));
        assert!(!error.to_string().contains("private-marker"));
    }
}
